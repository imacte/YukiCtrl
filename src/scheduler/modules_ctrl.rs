/*
 * Copyright (C) 2026 yuki
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

//! 全模块亮/息屏双套配置的应用层 (modules.*)
//!
//! 统一入口 `apply_screen_scoped`: 屏幕状态切换 / config 热重载时调用,
//! 把当前屏幕状态的 gpu/touch/swap/io 配置写到 sysfs.
//! frame 双套由 FAS 侧 set_frame_params 消费; touch 的 duration/extra
//! 由 hotplug 主循环消费 (见各自调用点).
//!
//! GPU 加速 (boost_util_pct > 0) 由独立轻量线程驱动: 读 sense 快照的
//! gpu 负载, 超线时临时拉满 max_gpu_clock, 回落时恢复配置上限.

use crate::scheduler::config::{Config, GpuModuleCfg, IoModuleCfg, ModulesConfig, SwapModuleCfg, TouchModuleCfg};
use crate::utils;
use log::{debug, info, warn};
use std::fs;
use std::sync::{Arc, RwLock};
use std::time::Duration;

const KGSL_BASE: &str = "/sys/class/kgsl/kgsl-3d0";

struct GpuRange { min_hz: u64, max_hz: u64 }

fn clocks_list(path: String) -> Vec<u64> {
    fs::read_to_string(&path)
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(|s| s.parse::<u64>().ok())
        .collect()
}

/// GPU 可用档位表 (Hz, 升序). 真机实测: kgsl-3d0 devfreq/available_frequencies
/// 为降序输出 (903M..231M), 排序后 first=min / last=max.
fn gpu_clocks_table() -> Vec<u64> {
    let mut clocks = clocks_list(format!("{}/devfreq/available_frequencies", KGSL_BASE));
    if clocks.is_empty() {
        clocks = clocks_list(format!("{}/freq_table_mhz", KGSL_BASE))
            .into_iter()
            .map(|m| if m < 1000 { m * 1_000_000 } else { m })
            .collect();
    }
    clocks.sort_unstable();
    clocks.dedup();
    clocks
}

fn gpu_hw_range() -> Option<GpuRange> {
    let clocks = gpu_clocks_table();
    if clocks.len() < 2 { return None; }
    let min_hz = *clocks.first()?;
    let max_hz = *clocks.last()?;
    if max_hz > min_hz { Some(GpuRange { min_hz, max_hz }) } else { None }
}

/// gpu 配置 → Hz: pct 相对硬件档位范围插值, 并吸附到最近可用档
fn gpu_pct_to_hz(pct: f32, range: &GpuRange, clocks: &[u64]) -> u64 {
    let p = pct.clamp(0.0, 100.0) as f64 / 100.0;
    let hz = range.min_hz as f64 + (range.max_hz - range.min_hz) as f64 * p;
    clocks.iter().copied()
        .min_by_key(|&c| (c as f64 - hz).abs() as u64)
        .unwrap_or(hz as u64)
}

fn apply_gpu(cfg: &GpuModuleCfg) {
    let range = match gpu_hw_range() {
        Some(r) => r,
        None => { debug!("[modules] gpu clocks unavailable, skip"); return; }
    };
    let clocks = gpu_clocks_table();
    let mut min_hz = gpu_pct_to_hz(cfg.min_pct, &range, &clocks);
    let mut max_hz = gpu_pct_to_hz(cfg.max_pct, &range, &clocks);
    if min_hz > max_hz { std::mem::swap(&mut min_hz, &mut max_hz); }
    // 真机实测: kgsl-3d0 下频率写入节点是 devfreq/min_freq + devfreq/max_freq (Hz),
    // 而非 min/max_gpu_clock.
    utils::try_write_file(format!("{}/devfreq/min_freq", KGSL_BASE), min_hz.to_string());
    utils::try_write_file(format!("{}/devfreq/max_freq", KGSL_BASE), max_hz.to_string());
    info!("[modules] gpu limits min={:.0}%({min_hz}Hz) max={:.0}%({max_hz}Hz)",
          cfg.min_pct, cfg.max_pct);
}

fn apply_swap(cfg: &SwapModuleCfg) {
    let v = cfg.swappiness.clamp(0, 200).to_string();
    if utils::try_write_file("/proc/sys/vm/swappiness", v.clone()).is_ok() {
        info!("[modules] swappiness={v}");
    }
}

fn apply_io(cfg: &IoModuleCfg) {
    let mut n_ra = 0;
    let mut n_sch = 0;
    if let Ok(entries) = fs::read_dir("/sys/block") {
        for e in entries.flatten() {
            let base = e.path().join("queue");
            let ra = base.join("read_ahead_kb");
            if ra.exists() && utils::try_write_file(&ra, cfg.read_ahead_kb.clone()).is_ok() {
                n_ra += 1;
            }
            if !cfg.scheduler.is_empty() {
                let sch = base.join("scheduler");
                if sch.exists() && utils::try_write_file(&sch, cfg.scheduler.clone()).is_ok() {
                    n_sch += 1;
                }
            }
        }
    }
    info!("[modules] io read_ahead={}KB on {n_ra} devs, scheduler='{}' on {n_sch} devs",
          cfg.read_ahead_kb, cfg.scheduler);
}

/// 屏幕状态切换 / config 热重载时统一应用当前屏幕状态的模块配置.
/// touch 的 enabled/duration/extra 与 frame 双套不在此处应用 —
/// 它们分别由 hotplug 主循环与 FAS set_frame_params 实时消费.
/// IO 亮屏套由现役链路 (IO_Settings + apply_system_tweaks) 管理,
/// 这里只应用息屏套 (modules.io.screen_off), 避免双写打架.
pub fn apply_screen_scoped(m: &ModulesConfig, screen_on: bool) {
    apply_gpu(m.gpu.pick(screen_on));
    apply_swap(m.swap.pick(screen_on));
    if !screen_on {
        apply_io(m.io.pick(false));
    }
}

/// 触摸配置的实时查询 (hotplug 每 tick 调用)
pub fn touch_cfg_now(cfg: &Config, screen_on: bool) -> TouchModuleCfg {
    *cfg.modules.touch.pick(screen_on)
}

/// 把当前屏幕状态的触摸配置推送到全局快照 (hotplug 200ms tick 消费).
/// 调用点: scheduler 启动 / config 热重载 / 屏幕状态切换.
pub fn update_touch_global(cfg: &Config, screen_on: bool) {
    let t = touch_cfg_now(cfg, screen_on);
    utils::set_touch_cfg_snapshot((t.enabled, t.extra_cores, t.duration_ms));
    crate::scheduler::hotplug::threshold::set_touch_protect_ms(t.duration_ms);
}

/// GPU 负载超过加速线时临时拉满 max_gpu_clock, 回落后恢复配置上限.
/// 500ms 周期; 无 kgsl 或加速关闭 (0%) 时线程静默空转, 开销可忽略.
pub fn spawn_gpu_boost_thread(cfg: Arc<RwLock<Config>>,
                              screen_state: Arc<RwLock<bool>>) {
    std::thread::Builder::new()
        .name("gpu_boost".to_string())
        .spawn(move || {
            let mut boosted = false;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let (gcfg, screen_on) = {
                    let c = cfg.read().unwrap();
                    let s = *screen_state.read().unwrap();
                    (*c.modules.gpu.pick(s), s)
                };
                if gcfg.boost_util_pct <= 0.0 || !screen_on {
                    if boosted {
                        apply_gpu(&gcfg); // 恢复配置上限
                        boosted = false;
                    }
                    continue;
                }
                let load = crate::monitor::sense_snapshot::sense_now().gpu.load_pct;
                if load >= gcfg.boost_util_pct && !boosted {
                    if let Some(r) = gpu_hw_range() {
                        let clocks = gpu_clocks_table();
                        let hz = gpu_pct_to_hz(100.0, &r, &clocks);
                        if utils::try_write_file(format!("{}/devfreq/max_freq", KGSL_BASE), hz.to_string()).is_ok() {
                            info!("[modules] gpu boost ON (load={:.0}% >= {:.0}%)", load, gcfg.boost_util_pct);
                            boosted = true;
                        }
                    }
                } else if load < gcfg.boost_util_pct * 0.8 && boosted {
                    apply_gpu(&gcfg); // 滞回 80% 回落
                    boosted = false;
                    info!("[modules] gpu boost OFF (load={:.0}%)", load);
                }
            }
        })
        .map(|_| ())
        .unwrap_or_else(|e| warn!("[modules] gpu boost thread spawn failed: {e}"));
}
