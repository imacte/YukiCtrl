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

use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct Meta {
    #[serde(default = "default_loglevel", alias = "Loglevel")]
    pub loglevel: String,
    
    #[serde(default = "default_language", alias = "Language")]
    pub language: String,
}

fn default_loglevel() -> String { "DEBUG".to_string() }
fn default_language() -> String { "en".to_string() }

// ════════════════════════════════════════════════════════════════
//  CPU Load Governor 配置
// ════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize, Clone)]
pub struct CpuLoadGovernorConfig {
    #[serde(default = "crate::utils::default_true")] pub enabled: bool,
    #[serde(default = "d_clg_up_thresh")] pub up_threshold: f32,
    #[serde(default = "d_clg_down_thresh")] pub down_threshold: f32,
    #[serde(default = "d_clg_smooth_up")] pub smoothing_up: f32,
    #[serde(default = "d_clg_smooth_down")] pub smoothing_down: f32,
    #[serde(default = "d_clg_down_rate")] pub down_rate_limit_ticks: u32,
    #[serde(default = "d_clg_up_rate")] pub up_rate_limit_ticks: u32,
    #[serde(default = "d_clg_headroom")] pub headroom_factor: f32,
    /// headroom 在 up_threshold 附近的过渡带宽度：从 up_threshold - headroom_ramp
    /// 到 up_threshold 线性由 1.0 渐变至 headroom_factor，避免阶跃导致振荡
    #[serde(default = "d_clg_headroom_ramp")] pub headroom_ramp: f32,
    #[serde(default = "d_clg_floor")] pub perf_floor: f32,
    #[serde(default = "d_clg_ceil")] pub perf_ceil: f32,
    #[serde(default = "d_clg_init")] pub perf_init: f32,
    /// 升频快速通道判定：target_perf 超过 current_perf 的幅度大于此值时直接快速升频
    #[serde(default = "d_clg_up_jump")] pub up_jump_threshold: f32,
    /// 低负载升频（负载未达 up_threshold 时）对 smoothing_up 的缩放系数
    #[serde(default = "d_clg_slow_up_scale")] pub slow_up_scale: f32,
    /// 滞回带内（down_threshold..up_threshold）降频时对 smoothing_down 的缩放系数，
    /// 用于防抖并避免高频锁定
    #[serde(default = "d_clg_slow_down_scale")] pub slow_down_scale: f32,
    /// 极低负载阈值：util 低于此值触发快速降频
    #[serde(default = "d_clg_down_fast_thresh")] pub down_fast_threshold: f32,
    /// 快速降频时对 smoothing_down 的放大倍数
    #[serde(default = "d_clg_down_fast_mult")] pub down_fast_mult: f32,
    /// 尖峰抑制：单 tick util 跳升超过此值时，其增量按 spike_decay 比例衰减，
    /// 避免孤立瞬时尖峰（如单核 0↔100%）瞬间拉满 perf
    #[serde(default = "d_clg_spike_jump")] pub spike_jump_threshold: f32,
    /// 尖峰增量保留比例（0.0=完全抑制，1.0=不抑制）
    #[serde(default = "d_clg_spike_decay")] pub spike_decay: f32,
}

fn d_clg_up_thresh() -> f32 { 0.80 }
fn d_clg_down_thresh() -> f32 { 0.50 }
fn d_clg_smooth_up() -> f32 { 0.60 }
fn d_clg_smooth_down() -> f32 { 0.30 }
fn d_clg_down_rate() -> u32 { 3 }
fn d_clg_up_rate() -> u32 { 2 }
fn d_clg_headroom() -> f32 { 1.25 }
fn d_clg_headroom_ramp() -> f32 { 0.15 }
fn d_clg_floor() -> f32 { 0.15 }
fn d_clg_ceil() -> f32 { 1.0 }
fn d_clg_init() -> f32 { 0.50 }
fn d_clg_up_jump() -> f32 { 0.35 }
fn d_clg_slow_up_scale() -> f32 { 0.02 }
fn d_clg_slow_down_scale() -> f32 { 0.5 }
fn d_clg_down_fast_thresh() -> f32 { 0.10 }
fn d_clg_down_fast_mult() -> f32 { 2.5 }
fn d_clg_spike_jump() -> f32 { 0.35 }
fn d_clg_spike_decay() -> f32 { 0.30 }

impl Default for CpuLoadGovernorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            up_threshold: d_clg_up_thresh(),
            down_threshold: d_clg_down_thresh(),
            smoothing_up: d_clg_smooth_up(),
            smoothing_down: d_clg_smooth_down(),
            down_rate_limit_ticks: d_clg_down_rate(),
            up_rate_limit_ticks: d_clg_up_rate(),
            headroom_factor: d_clg_headroom(),
            headroom_ramp: d_clg_headroom_ramp(),
            perf_floor: d_clg_floor(),
            perf_ceil: d_clg_ceil(),
            perf_init: d_clg_init(),
            up_jump_threshold: d_clg_up_jump(),
            slow_up_scale: d_clg_slow_up_scale(),
            slow_down_scale: d_clg_slow_down_scale(),
            down_fast_threshold: d_clg_down_fast_thresh(),
            down_fast_mult: d_clg_down_fast_mult(),
            spike_jump_threshold: d_clg_spike_jump(),
            spike_decay: d_clg_spike_decay(),
        }
    }
}

impl CpuLoadGovernorConfig {
    /// 校验并规范化配置：
    /// - 非有限值（NaN/±Inf，如 YAML 溢出值）回退默认，防止污染控制链
    /// - 阈值/系数限制在合理区间
    /// - floor/ceil/init 交叉约束，保证 f32::clamp 永不 panic
    pub fn normalize(&mut self) {
        if !self.up_threshold.is_finite() { self.up_threshold = d_clg_up_thresh(); }
        if !self.down_threshold.is_finite() { self.down_threshold = d_clg_down_thresh(); }
        if !self.smoothing_up.is_finite() { self.smoothing_up = d_clg_smooth_up(); }
        if !self.smoothing_down.is_finite() { self.smoothing_down = d_clg_smooth_down(); }
        if !self.headroom_factor.is_finite() { self.headroom_factor = d_clg_headroom(); }
        if !self.headroom_ramp.is_finite() { self.headroom_ramp = d_clg_headroom_ramp(); }
        if !self.perf_floor.is_finite() { self.perf_floor = d_clg_floor(); }
        if !self.perf_ceil.is_finite() { self.perf_ceil = d_clg_ceil(); }
        if !self.perf_init.is_finite() { self.perf_init = d_clg_init(); }
        if !self.up_jump_threshold.is_finite() { self.up_jump_threshold = d_clg_up_jump(); }
        if !self.slow_up_scale.is_finite() { self.slow_up_scale = d_clg_slow_up_scale(); }
        if !self.slow_down_scale.is_finite() { self.slow_down_scale = d_clg_slow_down_scale(); }
        if !self.down_fast_threshold.is_finite() { self.down_fast_threshold = d_clg_down_fast_thresh(); }
        if !self.down_fast_mult.is_finite() { self.down_fast_mult = d_clg_down_fast_mult(); }
        if !self.spike_jump_threshold.is_finite() { self.spike_jump_threshold = d_clg_spike_jump(); }
        if !self.spike_decay.is_finite() { self.spike_decay = d_clg_spike_decay(); }

        // 区间限制（语义约束）
        self.up_threshold = self.up_threshold.clamp(0.0, 1.0);
        self.down_threshold = self.down_threshold.clamp(0.0, 1.0);
        // 滞回语义：降频阈值不得高于升频阈值
        if self.down_threshold > self.up_threshold {
            self.down_threshold = self.up_threshold;
        }
        self.smoothing_up = self.smoothing_up.clamp(0.0, 1.0);
        self.smoothing_down = self.smoothing_down.clamp(0.0, 1.0);
        self.slow_up_scale = self.slow_up_scale.clamp(0.0, 1.0);
        self.slow_down_scale = self.slow_down_scale.clamp(0.0, 1.0);
        self.up_jump_threshold = self.up_jump_threshold.clamp(0.0, 1.0);
        self.down_fast_threshold = self.down_fast_threshold.clamp(0.0, 1.0);
        self.spike_jump_threshold = self.spike_jump_threshold.clamp(0.0, 1.0);
        self.spike_decay = self.spike_decay.clamp(0.0, 1.0);
        self.headroom_ramp = self.headroom_ramp.clamp(0.0, 1.0);
        // headroom 语义 >= 1（余量放大），down_fast_mult 语义 >= 1（放大）
        self.headroom_factor = self.headroom_factor.clamp(1.0, 3.0);
        self.down_fast_mult = self.down_fast_mult.clamp(1.0, 10.0);

        // 交叉约束（顺序保证 clamp 边界合法）
        if self.perf_floor > self.perf_ceil {
            self.perf_floor = self.perf_ceil;
        }
        self.perf_floor = self.perf_floor.clamp(0.0, 1.0);
        self.perf_ceil = self.perf_ceil.clamp(0.0, 1.0);
        if self.perf_floor > self.perf_ceil {
            self.perf_floor = self.perf_ceil;
        }
        self.perf_init = self.perf_init.clamp(self.perf_floor, self.perf_ceil);
    }
}

// ════════════════════════════════════════════════════════════════
//  核心模式与杂项配置
// ════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Mode {
    #[serde(default, alias = "CpuLoadGovernor")]
    pub cpu_load_governor: CpuLoadGovernorConfig,

    /// 需求: 目标负载 (WebUI 暴露, 按模式独立记忆).
    /// None = 未配置 → 回落 `fas::controller::mode_target_pressure()` 硬编码默认
    /// (powersave=40 / balance=60 / performance=75 / fast=85).
    #[serde(default)]
    pub target_load: Option<f32>,
}

// ════════════════════════════════════════════════════════════════
//  全模块亮屏/息屏双套配置中心 (modules.*)
//  结构统一: 每模块 { screen_on: {...}, screen_off: {...} },
//  daemon 在屏幕状态切换与 config 热重载时统一应用 (modules_ctrl.rs).
// ════════════════════════════════════════════════════════════════

/// 亮/息屏双套容器
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct ScreenScoped<T> {
    #[serde(default)]
    pub screen_on: T,
    #[serde(default)]
    pub screen_off: T,
}

impl<T: Default> Default for ScreenScoped<T> {
    fn default() -> Self {
        Self { screen_on: T::default(), screen_off: T::default() }
    }
}

impl<T> ScreenScoped<T> {
    pub fn pick(&self, screen_on: bool) -> &T {
        if screen_on { &self.screen_on } else { &self.screen_off }
    }
}

/// 显卡: 频率护栏 + 负载加速线 (Adreno kgsl, pct 相对硬件最高频)
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct GpuModuleCfg {
    /// 最低频率 % (0..=100)
    #[serde(default = "d_gpu_min")] pub min_pct: f32,
    /// 最高频率 % (10..=100)
    #[serde(default = "d_gpu_max")] pub max_pct: f32,
    /// 负载超过此值(%)时临时拉满最高频 (0 = 关闭加速)
    #[serde(default = "d_gpu_boost")] pub boost_util_pct: f32,
}
fn d_gpu_min() -> f32 { 0.0 }
fn d_gpu_max() -> f32 { 100.0 }
fn d_gpu_boost() -> f32 { 0.0 }
impl Default for GpuModuleCfg {
    fn default() -> Self {
        Self { min_pct: d_gpu_min(), max_pct: d_gpu_max(), boost_util_pct: d_gpu_boost() }
    }
}

/// 触摸加速: 开关 / 额外唤醒核数 / 保护时长
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct TouchModuleCfg {
    #[serde(default = "d_touch_en")] pub enabled: bool,
    /// 触摸时除白名单核外额外唤醒的核数 (0 = 仅白名单; 8 = 全部, 即历史行为)
    #[serde(default = "d_touch_cores")] pub extra_cores: u32,
    /// 触摸保护窗 (ms), 期间禁止关核
    #[serde(default = "d_touch_ms")] pub duration_ms: i64,
}
fn d_touch_en() -> bool { true }
fn d_touch_cores() -> u32 { 8 }
fn d_touch_ms() -> i64 { 200 }
impl Default for TouchModuleCfg {
    fn default() -> Self {
        Self { enabled: d_touch_en(), extra_cores: d_touch_cores(), duration_ms: d_touch_ms() }
    }
}

/// 内存交换: swappiness + 压力预警线
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct SwapModuleCfg {
    /// vm.swappiness (0..=200)
    #[serde(default = "d_swap_sw")] pub swappiness: u32,
    /// 内存压力预警线 (%) — 超限记日志 (监控语义, 不改变行为)
    #[serde(default = "d_swap_p")] pub pressure_pct: f32,
}
fn d_swap_sw() -> u32 { 100 }
fn d_swap_p() -> f32 { 20.0 }
impl Default for SwapModuleCfg {
    fn default() -> Self {
        Self { swappiness: d_swap_sw(), pressure_pct: d_swap_p() }
    }
}

/// 读写: 调度器 + 预读 (息屏套可独立收小; 空 scheduler = 不改)
#[derive(Debug, Deserialize, Clone)]
pub struct IoModuleCfg {
    #[serde(default)] pub scheduler: String,
    #[serde(default = "d_io_ra")] pub read_ahead_kb: String,
}
fn d_io_ra() -> String { "128".to_string() }
impl Default for IoModuleCfg {
    fn default() -> Self {
        Self { scheduler: String::new(), read_ahead_kb: d_io_ra() }
    }
}

/// 帧平滑: 掉帧判定与提频 (FAS 消费; 息屏 FAS 挂起, off 套为休眠结构)
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct FrameModuleCfg {
    /// 帧时间超出预算多少 ms 判掉帧
    #[serde(default = "d_frame_jank")] pub jank_margin_ms: f32,
    /// 掉帧提频总开关
    #[serde(default = "d_frame_boost_en")] pub boost_enabled: bool,
    /// 提频强度 (0..=2, 1 = 标准)
    #[serde(default = "d_frame_boost")] pub boost_strength: f32,
}
fn d_frame_jank() -> f32 { 4.0 }
fn d_frame_boost_en() -> bool { true }
fn d_frame_boost() -> f32 { 1.0 }
impl Default for FrameModuleCfg {
    fn default() -> Self {
        Self {
            jank_margin_ms: d_frame_jank(),
            boost_enabled: d_frame_boost_en(),
            boost_strength: d_frame_boost(),
        }
    }
}

/// 全模块双套配置
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ModulesConfig {
    #[serde(default)] pub gpu: ScreenScoped<GpuModuleCfg>,
    #[serde(default)] pub touch: ScreenScoped<TouchModuleCfg>,
    #[serde(default)] pub swap: ScreenScoped<SwapModuleCfg>,
    #[serde(default)] pub io: ScreenScoped<IoModuleCfg>,
    #[serde(default)] pub frame: ScreenScoped<FrameModuleCfg>,
}

/// 需求: CPU 频率护栏 (亮屏/息屏两套, 相对 policy 最高频的百分比).
///
/// CLG 决策出的 perf 会被 clamp 到 [min_pct, max_pct]/100 后再选频:
/// - 最低频率: 防止低负载时掉到极低频导致卡顿 (下限托底)
/// - 最高频率: 省电限频 (上限封顶, 如息屏 60% = 大核最高只到 60% 档位)
///
/// 默认 0/100 = 不限制, 与历史行为完全一致.
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct FreqLimits {
    #[serde(default = "d_fl_off")] pub screen_on_min_pct: f32,
    #[serde(default = "d_fl_on")]  pub screen_on_max_pct: f32,
    #[serde(default = "d_fl_off")] pub screen_off_min_pct: f32,
    #[serde(default = "d_fl_on")]  pub screen_off_max_pct: f32,
}

fn d_fl_off() -> f32 { 0.0 }
fn d_fl_on() -> f32 { 100.0 }

impl Default for FreqLimits {
    fn default() -> Self {
        Self {
            screen_on_min_pct: d_fl_off(),
            screen_on_max_pct: d_fl_on(),
            screen_off_min_pct: d_fl_off(),
            screen_off_max_pct: d_fl_on(),
        }
    }
}

impl FreqLimits {
    /// 取当前屏幕状态生效的 (floor, ceil) 百分比 (已 clamp 且 floor<=ceil)
    pub fn limits_for(&self, screen_on: bool) -> (f32, f32) {
        let (mut lo, mut hi) = if screen_on {
            (self.screen_on_min_pct, self.screen_on_max_pct)
        } else {
            (self.screen_off_min_pct, self.screen_off_max_pct)
        };
        lo = lo.clamp(0.0, 100.0);
        hi = hi.clamp(0.0, 100.0);
        if lo > hi { lo = hi; }
        (lo / 100.0, hi / 100.0)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct IOSettings {
    #[serde(default, rename = "Scheduler")] pub scheduler: String,
    #[serde(default = "default_read_ahead_kb")] pub read_ahead_kb: String,
    #[serde(default = "default_nomerges")] pub nomerges: String,
    #[serde(default = "default_iostats")] pub iostats: String,
}

impl Default for IOSettings {
    fn default() -> Self {
        Self {
            scheduler: String::new(),
            read_ahead_kb: default_read_ahead_kb(),
            nomerges: default_nomerges(),
            iostats: default_iostats(),
        }
    }
}

fn default_read_ahead_kb() -> String { "128".to_string() }
fn default_nomerges() -> String { "2".to_string() }
fn default_iostats() -> String { "0".to_string() }

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CpuIdle {
    pub current_governor: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct FunctionToggles {
    #[serde(rename = "CpuIdleScalingGovernor")] pub cpu_idle_scaling_governor: bool,
    #[serde(rename = "IOOptimization")] pub io_optimization: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default, alias = "Meta")]
    pub meta: Meta,
    #[serde(default)]
    pub function: FunctionToggles,
    #[serde(default, rename = "IO_Settings")]
    pub io_settings: IOSettings,
    #[serde(default, rename = "CpuIdle")]
    pub cpu_idle: CpuIdle,
    /// 需求: CPU 频率护栏 (亮屏/息屏两套); 缺失 = 全开不限制
    #[serde(default)]
    pub freq_limits: FreqLimits,
    /// 需求: 全模块亮/息屏双套配置 (gpu/touch/swap/io/frame)
    #[serde(default)]
    pub modules: ModulesConfig,

    // 按场景划分的性能模式
    #[serde(default)] pub powersave: Mode,
    #[serde(default)] pub balance: Mode,
    #[serde(default)] pub performance: Mode,
    #[serde(default)] pub fast: Mode,

    /// Phase 2 / ticket-07: 按前台包名施加调度偏置 (Restrict / Boost).
    /// YAML 示例:
    /// ```yaml
    /// app_rules:
    ///   - package: com.tencent.tmgp.pubgmhd
    ///     rule_type: boost
    ///     strength: heavy
    ///   - package: com.android.settings
    ///     rule_type: restrict
    ///     strength: light
    ///     disable_burst: true
    /// ```
    /// 默认空列表 — 不配置即不施加偏置 (向后兼容).
    #[serde(default)]
    pub app_rules: Vec<crate::scheduler::app_rule::AppRule>,
}

impl Config {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn get_mode(&self, mode_name: &str) -> Option<&Mode> {
        match mode_name {
            "powersave" => Some(&self.powersave),
            "balance" => Some(&self.balance),
            "performance" => Some(&self.performance),
            "fast" => Some(&self.fast),
            _ => None,
        }
    }

    /// 需求: 当前模式的目标负载 — 配置值优先, 未配置回落硬编码默认
    /// (powersave=40 / balance=60 / performance=75 / fast=85).
    pub fn target_load_of(&self, mode_name: &str) -> f32 {
        self.get_mode(mode_name)
            .and_then(|m| m.target_load)
            .map(|v| v.clamp(5.0, 95.0))
            .unwrap_or_else(|| crate::scheduler::fas::controller::mode_target_pressure(mode_name))
    }
}