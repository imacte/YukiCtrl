/*
 * Copyright (C) 2026 yuki
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! 热插拔 (Hotplug) 主循环 — 独立于 scheduler 主循环的 200ms tick.
//!
//! 架构 (D1-D8 决策):
//! - 独立线程 (hotplug_loop), 200ms tick (D3)
//! - 数据源: monitor::cpu_monitor::idle_snapshot_now()
//! - 判定器: threshold::ThresholdDecider (无 IO, 纯算法)
//! - FAS panic 旁路: fas::is_fas_panic() (D5)
//! - WebUI 通信: 文件 IPC (无 daemon HTTP server, 与现有 WebUI 风格一致)
//!     - 读 toggle/config: <module_root>/hotplug/config.yaml
//!     - 写 state: <module_root>/hotplug/state.yaml
//! - D7 KSU allowlist: 写 sysfs 失败 → log warn + skip, 不 panic

pub mod disable_policy;
pub mod threshold;
pub mod touch_signal;

use std::thread;
use std::time::{Duration, Instant};
use std::fs;
use std::path::{Path, PathBuf};
use std::io::Write;
use std::sync::OnceLock;
use std::collections::HashMap;

use anyhow::Result;
use log::{info, warn, debug};

use crate::monitor::cpu_monitor::{CpuIdleSnapshot, idle_snapshot_now};
use crate::monitor::app_detect;
use crate::monitor::sense_snapshot::screen_on_now;
use crate::scheduler::fas;
use crate::scheduler::app_rule::{AppRuleBias, AppRuleEngine};
use crate::utils::{find_cpu_temp_path, read_f64_from_file, try_write_file};
use crate::common;

use threshold::{
    ThresholdDecider, HotplugThresholds, HotplugToggles, CpuLoad,
    DISABLE_DEBOUNCE_TICKS,
};
use disable_policy::{
    decide_disable, DisableOutcome,
};

/// 热插拔 tick 周期 (D3: 200ms = 5Hz)
pub const HOTPLUG_TICK_MS: u64 = 200;

const HOTPLUG_DIR: &str = "hotplug";
const CONFIG_FILE: &str = "config.yaml";
const STATE_FILE: &str = "state.yaml";

// ════════════════════════════════════════════════════════════════
//  Phase 2 / ticket-07: App 规则引擎全局共享句柄
// ════════════════════════════════════════════════════════════════
//
// hotplug 是独立线程 (200ms tick), 不持有 Config 句柄.
// scheduler 主循环在启动时构造 AppRuleEngine 并通过 set_global_app_rule_engine
// 注入. 默认空 (OnceLock 未 init 时 query 返回 None → 不施加偏置).

static GLOBAL_APP_RULE_ENGINE: OnceLock<AppRuleEngine> = OnceLock::new();

/// 由 scheduler 主循环调用, 注入全局 AppRuleEngine.
/// 在 hotplug_loop 启动前调用一次即可; 之后若需热更新可重复调用.
pub fn set_global_app_rule_engine(engine: AppRuleEngine) {
    // OnceLock 不支持覆盖; 这里允许"覆盖式"调用, 通过 get_or_init 兜底
    // (生产环境应在启动期单次调用, 不存在覆盖场景)
    let _ = GLOBAL_APP_RULE_ENGINE.get_or_init(|| engine);
}

fn cpu_online_path(cpu_id: u32) -> String {
    format!("/sys/devices/system/cpu/cpu{}/online", cpu_id)
}

/// 解析内核 /sys/devices/system/cpu/online 的压缩区间格式: "0-7" / "0-3,5-7" / "0,2,4-6"
fn parse_online_list(s: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for part in s.trim().split(',') {
        let part = part.trim();
        if part.is_empty() { continue; }
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                if a <= b && b <= threshold::MAX_CPU_ID {
                    out.extend(a..=b);
                }
            }
        } else if let Ok(c) = part.parse::<u32>() {
            if c <= threshold::MAX_CPU_ID {
                out.push(c);
            }
        }
    }
    out
}

/// 读 sysfs 实际在线核列表. 读失败返回 None (跳过本次对账, 不阻塞决策).
fn read_sysfs_online() -> Option<Vec<u32>> {
    let s = fs::read_to_string("/sys/devices/system/cpu/online").ok()?;
    Some(parse_online_list(&s))
}

/// 节流日志 (10s): 温度软阈值预警等周期性事件用, 防刷屏
fn warn_throttle(msg: &str) {
    use std::sync::atomic::{AtomicI64, Ordering};
    static LAST_WARN_MS: AtomicI64 = AtomicI64::new(0);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let last = LAST_WARN_MS.load(Ordering::Relaxed);
    if now_ms.saturating_sub(last) >= 10_000 {
        LAST_WARN_MS.store(now_ms, Ordering::Relaxed);
        log::warn!("{}", msg);
    }
}

fn hotplug_dir() -> PathBuf { common::get_module_root().join(HOTPLUG_DIR) }
fn config_path() -> PathBuf { hotplug_dir().join(CONFIG_FILE) }
fn state_path() -> PathBuf { hotplug_dir().join(STATE_FILE) }

/// Phase 2 / ticket-07: 取当前前台包名 (app_detect 暴露).
/// 返回 None 表示 app_detect 尚未跑起来 / 还没有前台 App.
fn app_rule_bias_pkg_now() -> Option<String> {
    let pkg = app_detect::current_package();
    if pkg.is_empty() { None } else { Some(pkg) }
}

/// Phase 2 / ticket-07: 查询当前 App 规则偏置.
/// 若全局引擎未注入或包名无匹配, 返回 None (调用方不施加偏置).
fn query_app_rule_bias(pkg: &str) -> Option<AppRuleBias> {
    let engine = GLOBAL_APP_RULE_ENGINE.get()?;
    let rule = engine.match_rule(pkg)?;
    Some(AppRuleBias::from_rule(Some(rule)))
}

/// 启动 hotplug 主循环线程
pub fn start_hotplug_thread() -> Result<()> {
    let _ = fs::create_dir_all(hotplug_dir());

    thread::Builder::new()
        .name("hotplug_loop".to_string())
        .spawn(|| {
            info!("hotplug loop started");
            let mut loop_state = HotplugLoopState::default();
            // 任务 #6 reliability: hotplug 200ms tick 也是稳定心跳源.
            crate::watchdog::heartbeat_tick();
            loop {
                crate::watchdog::heartbeat_tick();
                let start = Instant::now();
                if let Err(e) = run_one_tick(&mut loop_state) {
                    debug!("hotplug tick error: {}", e);
                }
                let elapsed = start.elapsed();
                if elapsed < Duration::from_millis(HOTPLUG_TICK_MS) {
                    thread::sleep(Duration::from_millis(HOTPLUG_TICK_MS) - elapsed);
                }
            }
        })?;
    Ok(())
}

#[derive(Default)]
struct HotplugLoopState {
    decider: Option<ThresholdDecider>,
    last_state_write: Option<Instant>,
    /// Bug 3: 上次清 FAS panic 的 unix ms. 用于判断是否还在持续丢帧
    last_fas_panic_clear_ms: Option<i64>,
    /// Ticket merge: per-cpu last enable unix ms (0 = never).
    /// Used by hyperos-style min-offline-duration guard (cpu7=5s, cpu6=8s).
    last_enable_unix_ms: HashMap<u32, i64>,
    /// 任务 A: 上次 tick 的屏幕状态 (None = 首次), 用于切换日志与强制恢复保护核
    last_screen_on: Option<bool>,
}

/// 任务 A: keep_cores 用户配置 (亮屏/息屏分开; hotplug/config.yaml 由 WebUI 写入)
#[derive(Debug, Clone)]
struct KeepCoresConfig {
    /// 亮屏时永不关闭的核心 (默认 0-5)
    screen_on: Vec<u32>,
    /// 息屏时永不关闭的核心 (默认 0-1)
    screen_off: Vec<u32>,
}

/// 需求: 温度保护亮/息屏双套 (软阈值=预警, 硬阈值=强制全核).
/// hard_c <= 0 表示未配置 → 回落旧字段 thermal_force_all_on_c (兼容).
#[derive(Debug, Clone, Copy, Default)]
struct TempScreenCfg {
    soft_c: f32,
    hard_c: f32,
}

impl TempScreenCfg {
    fn pick(on: &TempScreenCfg, off: &TempScreenCfg, screen_on: bool) -> TempScreenCfg {
        if screen_on { *on } else { *off }
    }
}

impl Default for KeepCoresConfig {
    fn default() -> Self {
        Self {
            screen_on: vec![0, 1, 2, 3, 4, 5],
            screen_off: vec![0, 1],
        }
    }
}

/// 解析行内数组格式: "[0,1,2]" / "0,1,2" → Vec<u32>; 越界 (>7) 值丢弃
fn parse_cores(v: &str) -> Vec<u32> {
    v.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .filter(|&c| c <= threshold::MAX_CPU_ID)
        .collect()
}

fn load_config() -> (HotplugToggles, HotplugThresholds, KeepCoresConfig, TempScreenCfg, TempScreenCfg) {
    let path = config_path();
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            debug!("hotplug config not loaded ({}), using defaults", e);
            return (HotplugToggles::default(), HotplugThresholds::default(), KeepCoresConfig::default(),
                    TempScreenCfg::default(), TempScreenCfg::default());
        }
    };

    let mut toggles = HotplugToggles::default();
    let mut thresholds = HotplugThresholds::default();
    let mut keep = KeepCoresConfig::default();
    // 需求: 温度双套 (temp_on_soft_c / temp_on_hard_c / temp_off_soft_c / temp_off_hard_c)
    let mut temp_on = TempScreenCfg::default();
    let mut temp_off = TempScreenCfg::default();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            let v = v.trim();
            match k {
                "lockscreen_onoff" => toggles.lockscreen_onoff = parse_bool(v),
                "screens_onoff" => toggles.screens_onoff = parse_bool(v),
                "off_threshold_idle_pct" => {
                    if let Ok(x) = v.parse::<f32>() { thresholds.off_threshold_idle_pct = x.clamp(50.0, 100.0); }
                }
                "on_threshold_util_pct" => {
                    if let Ok(x) = v.parse::<f32>() { thresholds.on_threshold_util_pct = x.clamp(5.0, 95.0); }
                }
                "min_online_cores" => {
                    // 安全下限 2 与 WebUI 一致; 上限 8 (全保护等效停用动态关核)
                    if let Ok(x) = v.parse::<u32>() { thresholds.min_online_cores = x.clamp(2, 8); }
                }
                "thermal_force_all_on_c" => {
                    if let Ok(x) = v.parse::<f32>() { thresholds.thermal_force_all_on_c = x.clamp(45.0, 95.0); }
                }
                // 需求: 温度双套 (亮/息屏 soft/hard)
                "temp_on_soft_c" => { if let Ok(x) = v.parse::<f32>() { temp_on.soft_c = x.clamp(35.0, 95.0); } }
                "temp_on_hard_c" => { if let Ok(x) = v.parse::<f32>() { temp_on.hard_c = x.clamp(45.0, 100.0); } }
                "temp_off_soft_c" => { if let Ok(x) = v.parse::<f32>() { temp_off.soft_c = x.clamp(35.0, 95.0); } }
                "temp_off_hard_c" => { if let Ok(x) = v.parse::<f32>() { temp_off.hard_c = x.clamp(45.0, 100.0); } }
                "screen_on_keep_cores" => {
                    let cores = parse_cores(v);
                    if !cores.is_empty() { keep.screen_on = cores; }
                }
                "screen_off_keep_cores" => {
                    let cores = parse_cores(v);
                    if !cores.is_empty() { keep.screen_off = cores; }
                }
                _ => {}
            }
        }
    }
    (toggles, thresholds, keep, temp_on, temp_off)
}

fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

fn write_state_yaml(
    online_mask: u64,
    thermal_c: f32,
    toggles: HotplugToggles,
    thresholds: HotplugThresholds,
    screen_on: bool,
    active_keep_cores: &[u32],
) -> Result<()> {
    let path = state_path();
    let keep_str = active_keep_cores
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let body = format!(
        "# Auto-generated by core-pilot hotplug loop. Do not edit.\n\
         online_mask: {:#x}\n\
         thermal_c: {:.1}\n\
         lockscreen_onoff: {}\n\
         screens_onoff: {}\n\
         off_threshold_idle_pct: {:.1}\n\
         on_threshold_util_pct: {:.1}\n\
         min_online_cores: {}\n\
         thermal_force_all_on_c: {:.1}\n\
         disable_debounce_ticks: {}\n\
         screen_on: {}\n\
         active_keep_cores: \"{}\"\n\
         updated_at_unix_ms: {}\n",
        online_mask, thermal_c,
        toggles.lockscreen_onoff, toggles.screens_onoff,
        thresholds.off_threshold_idle_pct, thresholds.on_threshold_util_pct,
        thresholds.min_online_cores, thresholds.thermal_force_all_on_c,
        DISABLE_DEBOUNCE_TICKS,
        screen_on, keep_str,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
    );
    try_write_file(&path, body).map_err(|e| anyhow::anyhow!("write state.yaml: {}", e))
}

fn snapshot_to_loads(snap: &CpuIdleSnapshot) -> Vec<CpuLoad> {
    snap.cpus.iter().map(|e| CpuLoad {
        cpu_id: e.cpu_id, idle_pct: e.idle_pct, util_pct: e.util_pct,
    }).collect()
}

fn read_thermal_c() -> f32 {
    let path = match find_cpu_temp_path() {
        Ok(p) if !p.is_empty() => p,
        _ => return 0.0,
    };
    match read_f64_from_file(&path) {
        Ok(raw) => (raw / 1000.0) as f32,
        Err(_) => 0.0,
    }
}

/// Bug 1: 返回写入是否成功
fn apply_enable(cpu_id: u32) -> bool {
    let p = cpu_online_path(cpu_id);
    debug!("hotplug: enable cpu{}", cpu_id);
    match try_write_file(&p, b"1") {
        Ok(_) => true,
        Err(e) => {
            warn!("hotplug: write {} failed ({}), likely D7 SELinux/KSU deny; skip", p, e);
            false
        }
    }
}

/// Bug 1: 返回写入是否成功
fn apply_disable(cpu_id: u32) -> bool {
    let p = cpu_online_path(cpu_id);
    debug!("hotplug: disable cpu{}", cpu_id);
    match try_write_file(&p, b"0") {
        Ok(_) => true,
        Err(e) => {
            warn!("hotplug: write {} failed ({}), likely D7 SELinux/KSU deny; skip", p, e);
            false
        }
    }
}

// Ticket: hyperos-style freq-floor fallback. When online write is refused by
// kernel-level hotplug lock (D7), drop scaling_max_freq to cpuinfo_min_freq
// so the core cannot reach useful frequency (pseudo-offline).
fn apply_freq_floor_disable(cpu_id: u32) -> bool {
    let min_path = format!("/sys/devices/system/cpu/cpu{}/cpufreq/cpuinfo_min_freq", cpu_id);
    let max_path = format!("/sys/devices/system/cpu/cpu{}/cpufreq/scaling_max_freq", cpu_id);
    let min_raw = match fs::read_to_string(&min_path) {
        Ok(s) => s,
        Err(e) => {
            warn!("hotplug: read {} failed ({})", min_path, e);
            return false;
        }
    };
    let min_val = match min_raw.trim().parse::<u64>() {
        Ok(v) => v,
        Err(e) => {
            warn!("hotplug: parse {}={} failed ({})", min_path, min_raw.trim(), e);
            return false;
        }
    };
    if let Err(e) = try_write_file(&max_path, min_val.to_string().as_bytes()) {
        warn!("hotplug: freq-floor write {}={} failed ({})", max_path, min_val, e);
        return false;
    }
    debug!("hotplug: cpu{} freq-floor applied: scaling_max_freq={}", cpu_id, min_val);
    true
}

// Ticket: combines min-offline-duration guard + apply_disable + freq-floor fallback.
fn apply_disable_with_fallback(
    cpu_id: u32,
    last_enable_unix_ms: i64,
    now_unix_ms: i64,
) -> DisableOutcome {
    let online_ok = apply_disable(cpu_id);
    let outcome = decide_disable(cpu_id, last_enable_unix_ms, now_unix_ms, online_ok);
    if outcome == DisableOutcome::FreqFloorFallback {
        if apply_freq_floor_disable(cpu_id) {
            debug!("hotplug: cpu{} disabled via freq-floor fallback", cpu_id);
        }
    }
    outcome
}

/// Bug 2 修复: sleep 改 15ms (从 50ms 减, 经验值上 sysfs 已稳定可读)
/// 关键: 这个函数由 mod.rs 只对 to_disable 的最后一个核调用 (减少 tick 周期阻塞)
fn verify_online(cpu_id: u32, expected: bool) -> bool {
    std::thread::sleep(std::time::Duration::from_millis(15));
    let p = cpu_online_path(cpu_id);
    let raw = match std::fs::read_to_string(&p) {
        Ok(s) => s,
        Err(_) => return expected, // 读不到, 默认认为我们写的成功
    };
    let actual = match raw.trim().parse::<u8>() {
        Ok(0) => false,
        Ok(_) => true,
        Err(_) => expected,
    };
    if actual != expected {
        warn!(
            "hotplug: cpu{} write expected={} but actual={}, external module override",
            cpu_id, expected, actual
        );
    }
    actual
}

fn run_one_tick(state: &mut HotplugLoopState) -> Result<()> {
    let (toggles, mut thresholds, keep_cfg, temp_on, temp_off) = load_config();

    let snap = idle_snapshot_now();
    if snap.is_empty() {
        return Ok(());
    }

    // 任务 A: 屏幕状态 → 选择对应白名单. screen_on_now() 读 sense_snapshot
    // (由 screen_detect uevent 线程实时推送), 读不到时按亮屏处理 (保守方向).
    let screen_on = screen_on_now();
    let selected_keep = if screen_on { &keep_cfg.screen_on } else { &keep_cfg.screen_off };

    let loads = snapshot_to_loads(&snap);
    let thermal_c = read_thermal_c();

    // 需求: 温度双套 — 按屏幕状态选 hard 线 (新字段优先, 未配置回落旧字段);
    // soft 线超限 (且未达 hard) 记预警日志 (10s 节流).
    let temp_sel = TempScreenCfg::pick(&temp_on, &temp_off, screen_on);
    if temp_sel.hard_c > 0.0 {
        thresholds.thermal_force_all_on_c = temp_sel.hard_c;
    }
    if temp_sel.soft_c > 0.0 && thermal_c >= temp_sel.soft_c && thermal_c < thresholds.thermal_force_all_on_c {
        warn_throttle(&format!(
            "hotplug: thermal soft warning {:.1}C >= {:.1}C (hard={:.1}C)",
            thermal_c, temp_sel.soft_c, thresholds.thermal_force_all_on_c));
    }

    // 任务 A 配套修复 (真机实测): 灭屏瞬间 FPS 监控会把"没有帧"判成丢帧,
    // fas_panic 旁路随即强制全核在线, 与刚切小的息屏白名单互相拉扯,
    // 形成 disable→enable 振荡. 灭屏时 FAS 主循环本来就挂起
    // (FrameUpdate 直接 continue), 这里同步切断旁路; 触摸旁路同理.
    let fas_panic_now = screen_on && fas::is_fas_panic();
    let enabled = toggles.lockscreen_onoff && toggles.screens_onoff;

    // 漏洞 1 + 需求: 触摸加速可配置 (modules.touch.{on,off} 经全局快照):
    // enabled=false 关旁路; duration_ms 由 threshold 运行期原子量承接;
    // extra_cores 在 decision 后过滤非白名单核的唤醒配额.
    let (touch_enabled, touch_extra, _touch_ms) = crate::utils::touch_cfg_snapshot();
    let touch_down = screen_on && touch_enabled && touch_signal::is_touch_down();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    // ── Phase 2 / ticket-07: App 规则偏置 (漏洞 3) ──
    // 根据当前前台包名查询 AppRuleEngine, 给 hotplug off/on 阈值叠加偏置.
    // 注意: 这是"运行期动态偏置", 不会回写到 config.yaml; 退出前台 App
    // 立即恢复原阈值. 调用成本 = O(n) 线性扫表 (n 一般 < 50).
    //
    // 安全约束:
    //   - off_threshold_idle_pct clamp 到 [50.0, 100.0] (WebUI slider 范围)
    //   - on_threshold_util_pct  clamp 到 [5.0, 95.0]  (避免 < 5 触发频繁开核)
    if let Some(pkg) = app_rule_bias_pkg_now() {
        if let Some(bias) = query_app_rule_bias(&pkg) {
            thresholds.off_threshold_idle_pct =
                (thresholds.off_threshold_idle_pct + bias.hotplug_idle_offset)
                    .clamp(50.0, 100.0);
            thresholds.on_threshold_util_pct =
                (thresholds.on_threshold_util_pct + bias.hotplug_util_offset)
                    .clamp(5.0, 95.0);
            debug!(
                "hotplug: app_rule bias pkg={} off_off={:+.1} on_off={:+.1}",
                pkg, bias.hotplug_idle_offset, bias.hotplug_util_offset
            );
        }
    }

    if state.decider.is_none() {
        state.decider = Some(ThresholdDecider::new(thresholds, threshold::CpuAllowList::default()));
    }
    let decider = state.decider.as_mut().unwrap();
    decider.set_thresholds(thresholds);

    // 任务 A: 每 tick 应用当前屏幕状态对应的 keep_cores 白名单.
    // WebUI 改配置 → 下一 tick (≤200ms) 生效, 无需重启 daemon;
    // 屏幕亮灭切换 → 白名单自动跟着切, 新保护核由 decider 的强制在线逻辑立即拉起.
    if state.last_screen_on != Some(screen_on) {
        info!(
            "hotplug: screen {} -> keep_cores {:?} (min_online={})",
            if screen_on { "ON" } else { "OFF" },
            selected_keep, thresholds.min_online_cores
        );
        state.last_screen_on = Some(screen_on);
    }
    decider.set_keep_cores(selected_keep);

    // 失明对账: sysfs 实际 online ↔ decider 视图.
    // 外部 (用户 echo 0 > online / 其他 governor) 改核状态时 decider 收不到通知;
    // 视图漂移会让白名单强制在线分支误以为核还在线而永不拉起.
    // 对账后: 白名单内核被外部关掉 → 下一 tick 立即进 to_enable (200-400ms 回线);
    // mark_online_at(false) 打点 disabled_at 不会挡白名单分支 (它 bypass cooldown).
    if let Some(actual_online) = read_sysfs_online() {
        // 同步全局在线位图 (CLG 等消费方跳过全离线 policy 的频率写入)
        let mut mask = 0u32;
        for &c in &actual_online { mask |= 1u32 << c; }
        crate::utils::set_online_mask(mask);
        for cpu in 0..=threshold::MAX_CPU_ID {
            let is_on = actual_online.contains(&cpu);
            if decider.is_cpu_online_view(cpu) != is_on {
                decider.mark_online_at(cpu, is_on, now_ms);
                debug!("hotplug: reconcile cpu{} actual_online={} (external drift)", cpu, is_on);
            }
        }
    }

    let mut decision = decider.tick(&loads, thermal_c, fas_panic_now, enabled, touch_down, now_ms);

    // 需求: 触摸额外唤醒核配额 — extra_cores < 8 时, 非白名单核只保留
    // id 最大的 extra_cores 个 (大核优先), 白名单核不受限.
    if touch_down && touch_extra < 8 {
        let mut quota = touch_extra;
        let mut sorted: Vec<u32> = decision.to_enable.clone();
        sorted.sort_unstable_by(|a, b| b.cmp(a)); // 大核优先分配配额
        let mut keep = std::collections::HashSet::new();
        for &c in &sorted {
            if decider.allow_list().is_protected(c) {
                keep.insert(c);
            } else if quota > 0 {
                quota -= 1;
                keep.insert(c);
            }
        }
        decision.to_enable.retain(|&c| keep.contains(&c));
    }

    // Bug 1 彻底修复: decider.tick() 不再改 entry.online (collect-only).
    // 写成功才调 decider.mark_online, 写失败什么都不动 (entry.online 保持决策前真实状态)
    //
    // Bug 2 修复: 只对 to_disable.last() 做回读 (而不是每个 disable 都 sleep),
    //              sleep 从 50ms 减到 15ms 减少 tick 阻塞
        for cpu in &decision.to_enable {
        if apply_enable(*cpu) {
            // write succeeded -> set entry.online to true and record timestamp
            decider.mark_online_at(*cpu, true, now_ms);
            state.last_enable_unix_ms.insert(*cpu, now_ms);
        }
        // write failed does nothing (entry.online stays false, will retry next tick)
    }

    if !decision.to_disable.is_empty() {
        let last_disable_cpu = *decision.to_disable.last().unwrap();
        let disable_count = decision.to_disable.len();
        for (i, cpu) in decision.to_disable.iter().enumerate() {
            let last_enable_ms = state.last_enable_unix_ms.get(cpu).copied().unwrap_or(0);
            match apply_disable_with_fallback(*cpu, last_enable_ms, now_ms) {
                DisableOutcome::SkippedMinDuration => {
                    debug!("hotplug: cpu{} disable skipped (min-offline-duration guard)", *cpu);
                }
                DisableOutcome::WriteOnline => {
                    decider.mark_online_at(*cpu, false, now_ms);
                    state.last_enable_unix_ms.remove(cpu);
                    if i == disable_count - 1 {
                        let actual = verify_online(*cpu, false);
                        if actual {
                            decider.mark_online_at(*cpu, true, now_ms);
                            debug!("hotplug: cpu{} external override detected", *cpu);
                        }
                    }
                }
                DisableOutcome::FreqFloorFallback => {
                    debug!("hotplug: cpu{} pseudo-disabled via freq-floor (kernel lock)", *cpu);
                }
            }
        }
        let _ = last_disable_cpu;
    }

    // Bug 3: FAS panic 清除节流 — 如果 500ms 内又 set 了, 就不 clear
    if fas_panic_now {
        let recently_cleared = state.last_fas_panic_clear_ms
            .map(|t| (now_ms - t) < 500)
            .unwrap_or(false);
        if !recently_cleared {
            // 是新 panic, 清掉让下一 tick 走正常 hysteresis
            fas::clear_fas_panic();
            state.last_fas_panic_clear_ms = Some(now_ms);
        }
        // 否则保留 panic 状态, 下个 tick 继续全开 (避免抖动)
    }

    let now = Instant::now();
    if state.last_state_write.map_or(true, |t| now.duration_since(t) >= Duration::from_millis(500)) {
        let mask = decider.online_mask();
        let active_keep = decider.active_keep_cores();
        if let Err(e) = write_state_yaml(mask, thermal_c, toggles, thresholds, screen_on, &active_keep) {
            debug!("hotplug: write state.yaml failed: {}", e);
        }
        state.last_state_write = Some(now);
    }

    Ok(())
}

/// 直接调用入口 (单次 tick, 不启线程)
pub fn run_once() -> Result<()> {
    let mut state = HotplugLoopState::default();
    run_one_tick(&mut state)
}

/// 创建空 state.yaml (customize.sh 启动时 init 用)
pub fn init_state_file() -> Result<()> {
    let _ = fs::create_dir_all(hotplug_dir());
    let body = "# core-pilot hotplug state (will be populated on first daemon tick)\n\
                 online_mask: 0x0\n\
                 thermal_c: 0.0\n";
    let p = state_path();
    if !Path::new(&p).exists() {
        let mut f = fs::File::create(&p)?;
        f.write_all(body.as_bytes())?;
    }
    Ok(())
}

// ════════════════════════════════════════════════════════════════
//  Phase 2 / ticket-07: App 规则偏置相关单元测试
// ════════════════════════════════════════════════════════════════
//
// 由于 GLOBAL_APP_RULE_ENGINE 是 OnceLock, 不能在 #[cfg(test)] 下覆盖,
// 这里直接测"偏置量计算 + clamp 流程" 的纯函数部分 (与 run_one_tick 内部
// 逻辑等价). query_app_rule_bias 的端到端联调留在 main 启动后真机验证.

#[cfg(test)]
mod online_reconcile_tests {
    use super::*;

    #[test]
    fn parse_online_list_formats() {
        // 内核三种典型输出
        assert_eq!(parse_online_list("0-7"), vec![0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(parse_online_list("0-3,5-7"), vec![0, 1, 2, 3, 5, 6, 7]);
        assert_eq!(parse_online_list("0,2,4-6"), vec![0, 2, 4, 5, 6]);
        // 带换行/空格 (cat 回读带 \n)
        assert_eq!(parse_online_list("0-1\n"), vec![0, 1]);
        assert_eq!(parse_online_list(" 0-2 , 5 "), vec![0, 1, 2, 5]);
        // 越界值丢弃 (MAX_CPU_ID=7)
        assert_eq!(parse_online_list("0-63").len(), 8);
        assert_eq!(parse_online_list("99"), Vec::<u32>::new());
        // 畸形输入不出 panic
        assert_eq!(parse_online_list(""), Vec::<u32>::new());
        assert_eq!(parse_online_list("a-b"), Vec::<u32>::new());
        assert_eq!(parse_online_list("7-3"), Vec::<u32>::new()); // 倒序区间丢弃
    }

    #[test]
    fn decider_view_query_reflects_mark_online() {
        let mut d = ThresholdDecider::new(
            HotplugThresholds::default(), threshold::CpuAllowList::default());
        assert!(!d.is_cpu_online_view(3), "unknown cpu defaults to offline view");
        d.mark_online_at(3, true, 1_000);
        assert!(d.is_cpu_online_view(3));
        d.mark_online_at(3, false, 2_000);
        assert!(!d.is_cpu_online_view(3));
    }
}

#[cfg(test)]
mod app_rule_bias_tests {
    use super::*;
    use crate::scheduler::app_rule::{AppRule, AppRuleBias, AppRuleEngine, RuleStrength, RuleType};

    /// 模拟 run_one_tick 内部对 thresholds 的偏置叠加 + clamp 流程
    fn apply_app_rule_bias(
        off_threshold: f32,
        on_threshold: f32,
        bias: AppRuleBias,
    ) -> (f32, f32) {
        let off = (off_threshold + bias.hotplug_idle_offset).clamp(50.0, 100.0);
        let on = (on_threshold + bias.hotplug_util_offset).clamp(5.0, 95.0);
        (off, on)
    }

    fn mk_rule(pkg: &str, t: RuleType, s: RuleStrength) -> AppRule {
        AppRule {
            package: pkg.to_string(),
            rule_type: t,
            strength: s,
            max_freq_scale: None,
            target_util_offset: None,
            disable_burst: false,
            boost_threshold_offset: 0,
        }
    }

    #[test]
    fn restrict_raises_off_threshold() {
        // 默认 off=95, Restrict Medium 偏置 +3 → 98
        let r = mk_rule("com.test.game", RuleType::Restrict, RuleStrength::Medium);
        let bias = AppRuleBias::from_rule(Some(&r));
        let (off, on) = apply_app_rule_bias(95.0, 30.0, bias);
        assert!((off - 98.0).abs() < 0.001);
        // on 偏置为 0 → 不变
        assert!((on - 30.0).abs() < 0.001);
    }

    #[test]
    fn boost_lowers_off_threshold() {
        // 默认 off=95, Boost Medium 偏置 -3 → 92
        let r = mk_rule("com.test.game", RuleType::Boost, RuleStrength::Medium);
        let bias = AppRuleBias::from_rule(Some(&r));
        let (off, _on) = apply_app_rule_bias(95.0, 30.0, bias);
        assert!((off - 92.0).abs() < 0.001);
    }

    #[test]
    fn boost_threshold_offset_propagates_to_on() {
        // Boost + boost_threshold_offset=-5 → on 30 - 5 = 25
        let r = AppRule {
            boost_threshold_offset: -5,
            ..mk_rule("com.test.game", RuleType::Boost, RuleStrength::Medium)
        };
        let bias = AppRuleBias::from_rule(Some(&r));
        let (_off, on) = apply_app_rule_bias(95.0, 30.0, bias);
        assert!((on - 25.0).abs() < 0.001);
    }

    #[test]
    fn clamps_protect_extreme_values() {
        // Restrict Heavy 偏置 +5, 原始 off=99 → 99+5=104 → clamp 到 100
        let r = mk_rule("p", RuleType::Restrict, RuleStrength::Heavy);
        let bias = AppRuleBias::from_rule(Some(&r));
        let (off, _) = apply_app_rule_bias(99.0, 30.0, bias);
        assert_eq!(off, 100.0);
    }

    #[test]
    fn no_rule_means_no_change() {
        let bias = AppRuleBias::from_rule(None);
        let (off, on) = apply_app_rule_bias(95.0, 30.0, bias);
        assert!((off - 95.0).abs() < 0.001);
        assert!((on - 30.0).abs() < 0.001);
    }

    #[test]
    fn engine_match_returns_correct_rule() {
        let eng = AppRuleEngine::new(vec![
            AppRule {
                boost_threshold_offset: -3,
                ..mk_rule("com.tencent.tmgp.pubgmhd", RuleType::Boost, RuleStrength::Heavy)
            },
            AppRule {
                disable_burst: true,
                ..mk_rule("com.android.settings", RuleType::Restrict, RuleStrength::Light)
            },
        ]);
        let boost = eng.match_rule("com.tencent.tmgp.pubgmhd").unwrap();
        let bias = AppRuleBias::from_rule(Some(boost));
        assert_eq!(boost.rule_type, RuleType::Boost);
        assert!((bias.hotplug_idle_offset - (-5.0)).abs() < 0.001);

        let restrict = eng.match_rule("com.android.settings").unwrap();
        let bias2 = AppRuleBias::from_rule(Some(restrict));
        assert_eq!(restrict.rule_type, RuleType::Restrict);
        assert!(bias2.disable_burst);

        assert!(eng.match_rule("com.no.such.app").is_none());
    }

    /// Phase 2 / ticket-07-fix: matched_pkg 字段改为 Option<String>.
    /// 这里断言 bias.matched_pkg 是 owned String (不是 &str), 防止未来
    /// 误改回 &'static str 引发编译失败.
    #[test]
    fn matched_pkg_is_some_string() {
        let r = mk_rule("com.test.app", RuleType::Boost, RuleStrength::Heavy);
        let bias = AppRuleBias::from_rule(Some(&r));
        let pkg: String = bias.matched_pkg.expect("matched_pkg should be Some");
        assert_eq!(pkg, "com.test.app");
    }
}