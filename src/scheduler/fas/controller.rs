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

use crate::fas_types::{FasRulesConfig, PerAppProfile};
use crate::monitor::sense_snapshot::{sense_now, SenseSnapshot};
use std::time::Instant;
use log::{debug, info, warn};

use crate::i18n::t_with_args;
use crate::fluent_args;

use super::fps_window::FpsWindow;
use super::pid::{PidController, fps_norm};
use super::policy_controller::PolicyController;

// ════════════════════════════════════════════════════════════════
//  FasController — 主控制器
//
//  帧率档位匹配 + PID 控制
//  CPU 负载集成: core_utils 参与频率分配
// ════════════════════════════════════════════════════════════════

pub struct FasController {
    pub(super) cfg: FasRulesConfig,
    pub(super) fps_margin: f32,

    pub(super) pid: PidController,

    pub(super) fps_gears: Vec<f32>,
    pub(super) current_target_fps: f32,
    pub(super) perf_index: f32,
    pub(super) ema_actual_ms: f32,

    pub policies: Vec<PolicyController>,

    pub(super) fps_window: FpsWindow,
    pub(super) log_counter: u32,
    pub(super) consecutive_normal_frames: u32,

    // 加载
    pub(super) is_loading: bool,
    pub(super) loading_frames: u32,
    pub(super) loading_cumulative_ms: f32,
    pub(super) loading_normal_tolerance: u32,
    pub(super) post_loading_ignore: u32,
    pub(super) post_loading_downgrade_guard: u32,

    // 齿轮
    pub(super) upgrade_confirm_frames: u32,
    pub(super) downgrade_confirm_frames: u32,
    pub(super) upgrade_cooldown: u32,
    pub(super) gear_dampen_frames: u32,
    pub(super) consecutive_downgrade_count: u32,
    pub(super) last_downgrade_from_fps: f32,
    pub(super) stable_gear_frames: u32,

    // 降档 Boost
    pub(super) downgrade_boost_active: bool,
    pub(super) downgrade_boost_remaining: u32,
    pub(super) downgrade_boost_perf_saved: f32,

    // Jank
    pub(super) jank_cooldown: u32,
    pub(super) jank_streak: u32,

    // 时间
    pub(super) init_time: Instant,
    pub(super) freq_force_counter: u32,

    // 缓存
    pub(super) cached_norm: f32,
    pub(super) cached_budget_ms: f32,
    pub(super) cached_ema_budget: f32,

    // 温度感知
    pub(super) current_temperature: f64,
    pub(super) temp_threshold: f64,

    // [新] CPU 负载数据 — 由 SystemLoadUpdate 事件更新
    pub(super) foreground_max_util: f32,
    pub(super) core_utils: Vec<f32>,

    // 当前游戏包名
    pub(super) current_package: String,
    // 当前游戏的 per-app 配置
    pub(super) active_profile: Option<PerAppProfile>,

    // perf 地板死锁连续帧计数
    pub(super) floor_stuck_frames: u32,

    // util_cap EMA 平滑值，防止 200ms 采样周期的滞后数据造成断崖
    pub(super) ema_fg_util: f32,

    // ─── Phase 2 / ticket-06: 综合压力指数 ───
    /// 当前模式的 target_pressure (0..=100). 由 scheduler::mod.rs::ModeChange 事件更新.
    /// 默认 balance = 60.
    pub(super) target_pressure: f32,
    /// 最近一次算出的综合压力指数 (EMA 平滑). 供 `pid.compute()` 当 fg_util 替代.
    pub(super) ema_pressure_index: f32,
    /// 最近一次 frame_drop_active 状态 (用于压力指数的 frame 项权重).
    pub(super) last_frame_drop_active: bool,

    // [Jank 恢复保护] crit/heavy 后的 perf 最低值保护
    // 防止恢复帧到来后 PID 在 2-3 帧内将 perf 从 1.0 衰减到 floor，
    // 导致后续帧频率不足再次 jank 形成连锁掉帧
    pub(super) post_jank_perf_floor: f32,
    pub(super) post_jank_guard_frames: u32,

    // ─── 需求: 亮/息屏双套帧参数 (set_frame_params) ───
    /// 掉帧提频总开关 (modules.frame.{screen_on,screen_off}.boost_enabled)
    pub(super) frame_boost_enabled: bool,
    /// 提频强度缩放 0..=2 (1.0 = 标准幅度; 缩放 cfg.downgrade_boost_perf_inc)
    pub(super) frame_boost_strength: f32,
    /// 首次 set_frame_params 时记录的 boost 增量基准 (未缩放值)
    pub(super) frame_boost_base_inc: Option<f32>,

    // [动态 PID] 基于 CPU 利用率的 target_fps 偏移
    // 范围 [-3.0, 0.0]：当 CPU 利用率持续偏低时逐步降低有效 target_fps，
    // 让 PID 少给频率，节省功耗；利用率回升时逐步恢复
    pub(super) target_fps_offset: f32,
    pub(super) util_sample_timer: Instant,
}

impl FasController {
    pub fn new() -> Self {
        let cfg = FasRulesConfig::default();
        let pid_ctrl = PidController::new(cfg.pid.kp, cfg.pid.ki, cfg.pid.kd);
        Self {
            fps_margin: 3.0,
            perf_index: cfg.perf_init,
            pid: pid_ctrl,
            fps_gears: cfg.fps_gears.clone(),
            current_target_fps: 60.0,
            ema_actual_ms: 0.0,
            policies: Vec::new(),
            fps_window: FpsWindow::new(),
            log_counter: 0,
            consecutive_normal_frames: 0,
            is_loading: false,
            loading_frames: 0,
            loading_cumulative_ms: 0.0,
            loading_normal_tolerance: 0,
            post_loading_ignore: 0,
            post_loading_downgrade_guard: 0,
            upgrade_confirm_frames: 0,
            downgrade_confirm_frames: 0,
            upgrade_cooldown: 0,
            gear_dampen_frames: 0,
            consecutive_downgrade_count: 0,
            last_downgrade_from_fps: 0.0,
            stable_gear_frames: 0,
            downgrade_boost_active: false,
            downgrade_boost_remaining: 0,
            downgrade_boost_perf_saved: 0.0,
            jank_cooldown: 0,
            jank_streak: 0,
            init_time: Instant::now(),
            freq_force_counter: 0,
            cached_norm: 1.0,
            cached_budget_ms: 16.67,
            cached_ema_budget: 17.54,
            current_temperature: 0.0,
            temp_threshold: 0.0,
            foreground_max_util: 0.0,
            core_utils: Vec::new(),
            current_package: String::new(),
            active_profile: None,
            floor_stuck_frames: 0,
            ema_fg_util: 0.0,
            target_pressure: 60.0,        // balance default
            ema_pressure_index: 0.0,
            last_frame_drop_active: false,
            frame_boost_enabled: true,
            frame_boost_strength: 1.0,
            frame_boost_base_inc: None,
            post_jank_perf_floor: 0.0,
            post_jank_guard_frames: 0,
            target_fps_offset: 0.0,
            util_sample_timer: Instant::now(),
            cfg,
        }
    }

    // ════════════════════════════════════════════════════════════
    //  CPU 负载接口 (来自 SystemLoadUpdate 事件)
    // ════════════════════════════════════════════════════════════

    /// 更新前台最重线程的 CPU 利用率
    pub fn update_cpu_util(&mut self, fg_util: f32) {
        let prev = self.ema_fg_util;
        self.foreground_max_util = fg_util;
        // EMA smooth fg_util to prevent 200ms sampling lag causing cliff drops
        if self.ema_fg_util <= 0.001 {
            self.ema_fg_util = fg_util;
        } else {
            // Rise fast (alpha=0.4), fall slow (alpha=0.15) to prevent transient lows from killing freq
            let alpha = if fg_util > self.ema_fg_util { 0.40 } else { 0.15 };
            self.ema_fg_util = self.ema_fg_util * (1.0 - alpha) + fg_util * alpha;
        }
        debug!(
            "[fas] update_cpu_util fg_util={:.2} ema {:.2} -> {:.2}",
            fg_util, prev, self.ema_fg_util,
        );
    }

    /// 更新各核心利用率快照
    pub fn update_core_utils(&mut self, utils: &[f32]) {
        self.core_utils.clear();
        self.core_utils.extend_from_slice(utils);
    }

    // ════════════════════════════════════════════════════════════
    //  辅助方法
    // ════════════════════════════════════════════════════════════

    /// 获取有效 perf_floor —— 根据目标帧率动态抬高地板
    /// 高刷游戏 (120/144fps) 的 budget 仅 6.9~8.3ms，perf 过低会导致
    /// CPU 频率不足以在 budget 内渲染完一帧，任何突发负载都立刻卡顿。
    ///
    /// 旧公式硬顶 0.35，导致 120fps 下 perf 完全贴地运行(日志中稳态P=0.350)，
    /// 遇到突发负载需要多帧才能爬升到足够频率，造成可感知卡顿。
    /// 新公式: floor = base + (target_fps - 60) * 0.004, 上限 0.45
    ///   60fps  → 0.22 (不变)
    ///   90fps  → 0.34
    ///   120fps → 0.40 (原 0.35，多出 5% headroom)
    ///   144fps → 0.45 (原 0.35)
    pub(super) fn effective_perf_floor(&self) -> f32 {
        let base = self.cfg.perf_floor;
        let fps_bonus = ((self.current_target_fps - 60.0).max(0.0) * 0.004).min(0.25);
        (base + fps_bonus).min(0.45)
    }

    /// 获取有效 perf_ceil
    pub(super) fn effective_perf_ceil(&self) -> f32 {
        self.cfg.perf_ceil
    }

    pub(super) fn next_gear(&self) -> Option<f32> {
        self.fps_gears.iter().copied()
            .filter(|&g| g > self.current_target_fps + 0.5).reduce(f32::min)
    }

    pub(super) fn prev_gear(&self) -> Option<f32> {
        self.fps_gears.iter().copied()
            .filter(|&g| g < self.current_target_fps - 0.5).reduce(f32::max)
    }

    pub(super) fn max_gear(&self) -> f32 {
        self.fps_gears.iter().copied().fold(60.0_f32, f32::max)
    }

    pub(super) fn min_frame_ns(&self) -> u64 {
        (1_000_000_000.0 / self.max_gear()) as u64 / 2
    }

    pub(super) fn refresh_cached_values(&mut self) {
        self.cached_norm = fps_norm(self.current_target_fps);
        self.cached_budget_ms = 1000.0 / self.current_target_fps.max(1.0);
        self.cached_ema_budget = 1000.0 / (self.current_target_fps - self.fps_margin).max(1.0);
        // 动态适配 PID 系数到当前 target_fps
        self.pid.adapt_to_target_fps(self.current_target_fps);
    }

    /// 基于综合压力指数动态偏移 target_fps (Phase 2 / ticket-06)
    ///
    /// 输入从 "CPU 利用率 0..1" 改为 "压力指数 0..100". 每秒采样一次 ema_pressure_index:
    ///   idx <= 30   → 压力太低, 重置偏移 (可能在菜单/暂停画面)
    ///   idx <= 50   → 逐步降低 target (-0.1/s), 最多 -3fps
    ///   idx >= 70   → 逐步恢复 (+0.1/s) 至 0
    ///
    /// 阈值参考 spec ticket-06 的 target_pressure 默认 60 上下浮 10/20.
    /// 效果: GPU bound / IO bound 等 "低压力" 场景自动放宽帧率目标, 减少无效拉频.
    pub(super) fn adjust_target_for_util(&mut self) {
        if self.util_sample_timer.elapsed().as_millis() < 1000 { return; }
        self.util_sample_timer = Instant::now();

        // jank_cooldown 期间禁止降低 target, 只允许恢复
        // 防止刚从团战卡顿恢复, 压力指数还没爬满就又把目标降下去
        let allow_decrease = self.jank_cooldown == 0 && self.jank_streak == 0;

        // Phase 2: 用 ema_pressure_index (0..=100) 替代 ema_fg_util (0..1)
        let idx = self.ema_pressure_index;
        if idx <= 30.0 {
            self.target_fps_offset = 0.0;
        } else if idx <= 50.0 && allow_decrease {
            self.target_fps_offset = (self.target_fps_offset - 0.1).max(-3.0);
        } else if idx >= 70.0 {
            self.target_fps_offset = (self.target_fps_offset + 0.1).min(0.0);
        }
    }

    /// 获取经过 util 偏移后的有效 target_fps
    #[inline]
    pub(super) fn effective_target_fps(&self) -> f32 {
        (self.current_target_fps + self.target_fps_offset).max(10.0)
    }

    // ════════════════════════════════════════════════════════════
    //  公共接口：游戏生命周期
    // ════════════════════════════════════════════════════════════

    /// 通知 FAS 当前前台游戏变化
    pub fn set_game(&mut self, _pid: i32, package: &str) {
        debug!("[fas] set_game pid={} pkg={}", _pid, package);
        self.current_package = package.to_string();
        let profile = self.cfg.per_app_profiles.get(package).cloned();
        if let Some(ref p) = profile {
            if let Some(m) = p.fps_margin { self.fps_margin = m; }
            if let Some(ref gears) = p.target_fps {
                if !gears.is_empty() {
                    self.fps_gears = gears.clone();
                    if !self.fps_gears.iter().any(|&g| (g - self.current_target_fps).abs() < 0.5) {
                        self.current_target_fps = self.fps_gears.iter().copied()
                            .fold(60.0_f32, f32::max);
                    }
                    self.refresh_cached_values();
                }
            }
            debug!(
                "[fas] applied per-app profile pkg={} margin={:.2} gears={:?} target={:.0}",
                package, self.fps_margin, self.fps_gears, self.current_target_fps,
            );
            info!("{}", t_with_args("fas-set-game", &fluent_args!(
                "pkg" => package,
                "gears" => format!("{:?}", self.fps_gears),
                "target" => format!("{:.0}", self.current_target_fps)
            )));
        } else {
            warn!("{}", t_with_args("fas-no-profile", &fluent_args!(
                "pkg" => package,
                "gears" => format!("{:?}", self.fps_gears)
            )));
        }
        self.active_profile = profile;
    }

    /// 通知 FAS 退出游戏
    pub fn clear_game(&mut self) {
        debug!("[fas] clear_game (was pkg={})", self.current_package);
        self.current_package.clear();
        self.active_profile = None;
        self.foreground_max_util = 0.0;
        self.ema_fg_util = 0.0;
        self.core_utils.clear();
        self.target_fps_offset = 0.0;
        // 恢复全局 margin 和 gears
        self.fps_margin = self.cfg.fps_margin;
        self.fps_gears = self.cfg.fps_gears.clone();
    }

    pub fn set_temperature(&mut self, temp: f64) { self.current_temperature = temp; }
    pub fn set_temp_threshold(&mut self, thresh: f64) { self.temp_threshold = thresh; }

    // ════════════════════════════════════════════════════════════
    //  Phase 2 / ticket-06: 综合压力指数
    // ════════════════════════════════════════════════════════════

    /// 需求: 亮/息屏双套帧参数 — 由 scheduler 在屏幕状态切换与 config 热重载时调用.
    ///
    /// - `jank_margin_ms`: 帧时间超出预算多少 ms 判掉帧 → 换算为 fps_margin
    ///   (帧数 = ms × 当前目标帧率 / 1000, clamp 0.5..=10)
    /// - `boost_enabled`: 掉帧提频总开关 (false 时降档 boost 不再触发, 在途 boost 立即撤销)
    /// - `boost_strength`: 0..=2, 相对基准 (首次调用时的配置值) 缩放提频增量
    pub fn set_frame_params(&mut self, jank_margin_ms: f32, boost_enabled: bool, boost_strength: f32) {
        let margin_frames = (jank_margin_ms.max(0.5) * self.current_target_fps / 1000.0)
            .clamp(0.5, 10.0);
        self.fps_margin = margin_frames;
        self.frame_boost_enabled = boost_enabled;
        self.frame_boost_strength = boost_strength.clamp(0.0, 2.0);
        if self.frame_boost_base_inc.is_none() {
            self.frame_boost_base_inc = Some(self.cfg.downgrade_boost_perf_inc);
        }
        if let Some(base) = self.frame_boost_base_inc {
            self.cfg.downgrade_boost_perf_inc = base * self.frame_boost_strength;
        }
        if !boost_enabled {
            self.downgrade_boost_active = false;
            self.downgrade_boost_remaining = 0;
        }
        debug!(
            "[fas] frame params: jank={:.1}ms(margin={:.1}f) boost={} strength={:.2}",
            jank_margin_ms, margin_frames, boost_enabled, self.frame_boost_strength
        );
    }

    /// 设置当前模式的 target_pressure (0..=100).
    /// 由 `scheduler::mod.rs::DaemonEvent::ModeChange` 在模式切换时调用.
    /// 见 `mode_target_pressure()` 的 4 种模式映射表.
    pub fn set_target_pressure(&mut self, target: f32) {
        let clamped = target.clamp(0.0, 100.0);
        debug!(
            "[fas] set_target_pressure target={:.2} (clamped={:.2})",
            target, clamped
        );
        self.target_pressure = clamped;
    }

    /// Phase 2 / ticket-07: 设置 target_pressure 同时叠加 App 规则偏置.
    ///
    /// 由 scheduler::mod.rs 在 ModeChange 事件或前台包变化时调用:
    ///   target = mode_target_pressure(mode) + bias.target_util_offset
    ///   并 clamp 到 [5.0, 95.0] (避免极端值破坏 PID 工作点).
    ///
    /// 设计要点:
    /// - 入口处允许 target < 5 或 > 95 (例如 Restrict Heavy 时 base=60 + (-35)=25
    ///   仍然合理; 但 base=40 + (-35)=5 是边界, 不应跌破 5 避免 PID 失效)
    /// - bias=0 时行为与 set_target_pressure 完全一致 (向后兼容)
    pub fn set_target_pressure_with_app_bias(&mut self, target: f32, bias_offset: i32) {
        let biased = target + bias_offset as f32;
        let clamped = biased.clamp(5.0, 95.0);
        debug!(
            "[fas] set_target_pressure_with_app_bias base={:.2} bias={} -> biased={:.2} (clamped to 5..=95) = {:.2}",
            target, bias_offset, biased, clamped,
        );
        self.target_pressure = clamped;
    }

    /// 喂入综合压力指数 (调用方已经在别处算过).
    /// 这里做 EMA 平滑后存进 self.ema_pressure_index, 供 pid.compute() 替代 fg_util.
    pub fn update_pressure_index(&mut self, raw: f32, frame_drop_active: bool) {
        self.last_frame_drop_active = frame_drop_active;
        let v = raw.clamp(0.0, 100.0);
        let prev = self.ema_pressure_index;
        if self.ema_pressure_index <= 0.001 {
            self.ema_pressure_index = v;
        } else {
            // 上升快 (alpha=0.4), 下降慢 (alpha=0.15) — 防止瞬时低值杀掉 perf
            let alpha = if v > self.ema_pressure_index { 0.40 } else { 0.15 };
            self.ema_pressure_index = self.ema_pressure_index * (1.0 - alpha) + v * alpha;
        }
        debug!(
            "[fas] update_pressure_index raw={:.2} frame_drop={} ema {:.2} -> {:.2}",
            v, frame_drop_active, prev, self.ema_pressure_index,
        );
    }

    /// 一次性更新: 拉 sense_now() + 调用 compute_pressure_index + EMA 平滑.
    /// 这是 FAS tick 主循环推荐入口 (替代每帧 push fg_util).
    pub fn tick_pressure_index(&mut self, frame_drop_active: bool) -> f32 {
        let snap = sense_now();
        let p = compute_pressure_index(&snap, frame_drop_active);
        self.update_pressure_index(p, frame_drop_active);
        self.ema_pressure_index
    }

    pub(super) fn reset_runtime(&mut self) {
        let floor = self.effective_perf_floor();
        let ceil = self.effective_perf_ceil();
        // effective floor 上限 0.45 可能超过低 perf_ceil 配置，min 保证 clamp 边界合法
        self.perf_index = self.cfg.perf_init.clamp(floor.min(ceil), ceil);
        self.ema_actual_ms = 0.0;
        self.pid.reset();
        self.fps_window.clear();
        self.log_counter = 0;
        self.consecutive_normal_frames = 0;
        self.is_loading = false;
        self.loading_frames = 0;
        self.loading_cumulative_ms = 0.0;
        self.loading_normal_tolerance = 0;
        self.post_loading_ignore = 0;
        self.post_loading_downgrade_guard = 0;
        self.upgrade_confirm_frames = 0;
        self.downgrade_confirm_frames = 0;
        self.upgrade_cooldown = 0;
        self.gear_dampen_frames = 0;
        self.consecutive_downgrade_count = 0;
        self.last_downgrade_from_fps = 0.0;
        self.stable_gear_frames = 0;
        self.downgrade_boost_active = false;
        self.downgrade_boost_remaining = 0;
        self.jank_cooldown = 0;
        self.jank_streak = 0;
        self.freq_force_counter = 0;
        self.floor_stuck_frames = 0;
        self.ema_fg_util = 0.0;
        self.post_jank_perf_floor = 0.0;
        self.post_jank_guard_frames = 0;
        self.target_fps_offset = 0.0;
        self.util_sample_timer = Instant::now();
        // 模式切换时清掉旧压力记忆, 让新模式从 0 开始累
        self.ema_pressure_index = 0.0;
        self.last_frame_drop_active = false;
    }
}

// ════════════════════════════════════════════════════════════════
//  Phase 2 / ticket-06: 综合压力指数 (free function)
//
//  把八路感知合成一个 0..=100 的标量, 供 PID/调速器当作"系统忙闲度".
//  不依赖 hotplug 模块, 也不依赖 fas/cpu_load_governor 任何内部状态.
// ════════════════════════════════════════════════════════════════

/// 综合压力指数 (0..=100).
///
/// 权重 (来自 spec ticket-04 / ticket-06):
/// - cpu_util_avg × 0.40     (CPU 平均利用率, 来自 CpuIdleSnapshot)
/// - gpu_load_pct × 0.25     (GPU 负载百分比; NaN/0 → 不可用)
/// - io_psi × 0.15           (IO 压力 PSI avg10 %)
/// - mem_psi × 0.10          (内存压力 PSI full %)
/// - (frame_drop_active ? 1.0 : 0.0) × 0.10
///
/// **向后兼容**: 任何一路不可用 (NaN/0/不存在) → 该项权重被剔除, 剩余项
/// 按原比例重新归一化. 例如无 GPU 时:
///   pressure = cpu×(0.40/0.75) + io×(0.15/0.75) + mem×(0.10/0.75) + frame×(0.10/0.75)
///
/// 这样哪怕设备没 GPU devfreq (某些 MTK/老高通), 也不会让公式失真.
pub fn compute_pressure_index(snap: &SenseSnapshot, frame_drop_active: bool) -> f32 {
    // ---- 1. CPU 利用率 (0..=100) ----
    let cpu = snap.cpu_util_avg().clamp(0.0, 100.0);
    let cpu_available = cpu > 0.0;

    // ---- 2. GPU 负载 (0..=100, NaN 表示没 GPU devfreq) ----
    let gpu_raw = snap.gpu.load_pct;
    let gpu_available = gpu_raw.is_finite() && gpu_raw > 0.0;
    let gpu = if gpu_available { gpu_raw.clamp(0.0, 100.0) } else { 0.0 };

    // ---- 3. IO PSI (avg10 是 0..=100 的百分比) ----
    let io = snap.io.some_pct.clamp(0.0, 100.0);
    let io_available = io > 0.0;

    // ---- 4. 内存 PSI (full avg10 是 0..=100 的百分比; swap.monitor 直接 push pct) ----
    let mem = snap.swap.mem_full_us as f32 / 10_000.0 * 100.0;
    let mem_clamped = mem.clamp(0.0, 100.0);
    let mem_available = mem_clamped > 0.0;

    // ---- 5. frame_drop_active (boolean) ----
    let frame_v = if frame_drop_active { 1.0 } else { 0.0 };
    let frame_available = true; // boolean 总可用

    // ---- 权重 + 归一化 ----
    let w_cpu = if cpu_available { 0.40 } else { 0.0 };
    let w_gpu = if gpu_available { 0.25 } else { 0.0 };
    let w_io = if io_available { 0.15 } else { 0.0 };
    let w_mem = if mem_available { 0.10 } else { 0.0 };
    let w_frame = if frame_available { 0.10 } else { 0.0 };
    let w_sum = w_cpu + w_gpu + w_io + w_mem + w_frame;

    if w_sum < 0.001 {
        return 0.0;
    }

    let raw = cpu * w_cpu + gpu * w_gpu + io * w_io + mem_clamped * w_mem + frame_v * w_frame;
    (raw / w_sum).clamp(0.0, 100.0)
}

/// mode 名 → target_pressure 映射.
///
/// 与 project 原 4 种模式保持兼容, 只是从 "target_util 偏移" 改成
/// "target_pressure 目标值" (0..=100). Mode 名 == `scheduler/config.rs::Mode` 字段.
pub fn mode_target_pressure(mode_name: &str) -> f32 {
    match mode_name {
        "powersave" => 40.0,
        "balance" => 60.0,
        "performance" => 75.0,
        "fast" | "extreme" => 85.0,
        _ => 60.0,
    }
}

#[cfg(test)]
mod pressure_index_tests {
    use super::*;
    use crate::monitor::cpu_monitor::CpuIdleEntry;
    use crate::monitor::sense_snapshot::{GpuState, IoState, SwapState};

    fn make_snap(cpu_utils: Vec<f32>, gpu_load: f32, io_pct: f32, mem_full_us: u64) -> SenseSnapshot {
        let mut snap = SenseSnapshot::default();
        snap.cpu.cpus = cpu_utils
            .into_iter()
            .enumerate()
            .map(|(i, util)| CpuIdleEntry {
                cpu_id: i as u32,
                idle_pct: 100.0 - util,
                util_pct: util,
            })
            .collect();
        snap.gpu = GpuState {
            load_pct: gpu_load,
            ..Default::default()
        };
        snap.io = IoState {
            some_pct: io_pct,
            ..Default::default()
        };
        snap.swap = SwapState {
            mem_full_us,
            ..Default::default()
        };
        snap.screen_on = true;
        snap
    }

    #[test]
    fn pressure_full_no_frame_drop() {
        // cpu 80*0.40 + gpu 60*0.25 + io 30*0.15 + mem 20*0.10 + 0*0.10
        // = 32 + 15 + 4.5 + 2 + 0 = 53.5
        let snap = make_snap(vec![80.0; 4], 60.0, 30.0, 2000);
        let p = compute_pressure_index(&snap, false);
        assert!((p - 53.5).abs() < 0.001, "got {p}");
    }

    #[test]
    fn pressure_full_with_frame_drop() {
        // 上面 + frame drop → +0.10 → 53.5 + 10 = 63.5
        let snap = make_snap(vec![80.0; 4], 60.0, 30.0, 2000);
        let p = compute_pressure_index(&snap, true);
        assert!((p - 63.5).abs() < 0.001, "got {p}");
    }

    #[test]
    fn pressure_no_gpu_renormalize() {
        // gpu=0 → 不可用, 剩余 cpu/io/mem/frame 权重和 = 0.75
        // 80*(0.40/0.75) + 30*(0.15/0.75) + 20*(0.10/0.75) + 0
        // ≈ 42.667 + 6.0 + 2.667 = 51.333
        let snap = make_snap(vec![80.0; 4], 0.0, 30.0, 2000);
        let p = compute_pressure_index(&snap, false);
        assert!((p - 51.333).abs() < 0.01, "got {p}");
    }

    #[test]
    fn pressure_zero_when_all_unavailable() {
        // 全不可用 → 0
        let snap = make_snap(vec![], 0.0, 0.0, 0);
        let p = compute_pressure_index(&snap, false);
        assert_eq!(p, 0.0);
    }

    #[test]
    fn pressure_clamps_to_100() {
        let snap = make_snap(vec![100.0; 8], 100.0, 100.0, 100_000);
        let p = compute_pressure_index(&snap, true);
        assert_eq!(p, 100.0);
    }

    #[test]
    fn mode_mapping_covers_all_4_modes() {
        assert_eq!(mode_target_pressure("powersave"), 40.0);
        assert_eq!(mode_target_pressure("balance"), 60.0);
        assert_eq!(mode_target_pressure("performance"), 75.0);
        assert_eq!(mode_target_pressure("extreme"), 85.0);
        assert_eq!(mode_target_pressure("fast"), 85.0);
        assert_eq!(mode_target_pressure("unknown"), 60.0);
        assert_eq!(mode_target_pressure(""), 60.0);
    }
}
