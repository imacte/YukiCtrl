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

use crate::scheduler::config::Config;
use std::fs::{self, File, OpenOptions};
use std::io::{Write, Seek, SeekFrom};
use std::path::Path;
use log::info;
use serde::Deserialize;
use std::time::Instant;

#[derive(Deserialize, Default)]
struct RulesYaml {
    #[serde(default)]
    fas_rules: FasRules,
}

#[derive(Deserialize)]
struct FasRules {
    #[serde(default = "default_gears")]
    fps_gears: Vec<f32>,
    #[serde(default = "default_margin")]
    fps_margin: String,
    #[serde(default = "default_heavy_frame_ms")]
    heavy_frame_threshold_ms: f32,
    #[serde(default = "default_loading_cumulative_ms")]
    loading_cumulative_ms: f32,
    #[serde(default = "default_post_loading_ignore")]
    post_loading_ignore_frames: u32,
    #[serde(default = "default_post_loading_perf_min")]
    post_loading_perf_min: f32,
    #[serde(default = "default_post_loading_perf_max")]
    post_loading_perf_max: f32,
    #[serde(default = "default_instant_error_threshold")]
    instant_error_threshold_ms: f32,
    #[serde(default = "default_perf_floor")]
    perf_floor: f32,
    #[serde(default = "default_hysteresis")]
    freq_hysteresis: f32,
}

impl Default for FasRules {
    fn default() -> Self {
        Self {
            fps_gears: default_gears(),
            fps_margin: default_margin(),
            heavy_frame_threshold_ms: default_heavy_frame_ms(),
            loading_cumulative_ms: default_loading_cumulative_ms(),
            post_loading_ignore_frames: default_post_loading_ignore(),
            post_loading_perf_min: default_post_loading_perf_min(),
            post_loading_perf_max: default_post_loading_perf_max(),
            instant_error_threshold_ms: default_instant_error_threshold(),
            perf_floor: default_perf_floor(),
            freq_hysteresis: default_hysteresis(),
        }
    }
}

fn default_gears() -> Vec<f32> { vec![20.0, 24.0, 30.0, 45.0, 60.0, 90.0, 120.0, 144.0] }
fn default_margin() -> String { "3.0".to_string() }
fn default_heavy_frame_ms() -> f32 { 150.0 }
fn default_loading_cumulative_ms() -> f32 { 2500.0 }
fn default_post_loading_ignore() -> u32 { 5 }
fn default_post_loading_perf_min() -> f32 { 500.0 }
fn default_post_loading_perf_max() -> f32 { 800.0 }
fn default_instant_error_threshold() -> f32 { 4.0 }
fn default_perf_floor() -> f32 { 150.0 }
fn default_hysteresis() -> f32 { 0.015 }

/// sysfs 频率写入器，带缓存和强制写入支持。
/// 强制写入用于对抗 thermal-engine 等外部进程偷改频率。
pub struct FastWriter {
    file: Option<File>,
    last_value: Option<u32>,
}

impl FastWriter {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let path_ref = path.as_ref();
        let _ = crate::utils::enable_perm(path_ref);
        let file = OpenOptions::new().write(true).open(path_ref)
            .map_err(|e| log::error!("FAS: failed to open {}: {}", path_ref.display(), e))
            .ok();
        // 不修改 sysfs 权限为 0o444（拦不住 root 级 thermal-engine，且可能导致写入失败）
        Self { file, last_value: None }
    }

    /// 带缓存写入（值未变则跳过）
    pub fn write_value(&mut self, value: u32) {
        if self.last_value == Some(value) { return; }
        self.do_write(value);
    }

    /// 强制写入，无视缓存（用于 apply_freq_safe 和 mismatch 修复）
    pub fn write_value_force(&mut self, value: u32) {
        self.do_write(value);
    }

    /// 清除缓存，下次 write_value 必定执行写入
    pub fn invalidate(&mut self) {
        self.last_value = None;
    }

    fn do_write(&mut self, value: u32) {
        if let Some(file) = &mut self.file {
            let mut buf = itoa::Buffer::new();
            let val_str = buf.format(value);
            let _ = file.seek(SeekFrom::Start(0));
            if let Err(e) = file.write_all(val_str.as_bytes()) {
                log::error!("FAS: failed to write freq {}: {}", val_str, e);
            }
            let _ = file.set_len(val_str.len() as u64);
            self.last_value = Some(value);
        }
    }
}

/// CPU 频率策略控制器，始终强制写入以对抗外部干扰。
pub struct PolicyController {
    pub max_writer: FastWriter,
    pub min_writer: FastWriter,
    pub available_freqs: Vec<u32>,
    pub current_freq: u32,
    pub policy_id: usize,
}

impl PolicyController {
    pub fn apply_freq_safe(&mut self, target_freq: u32) {
        // 不跳过 target == current_freq，外部可能已改掉实际频率
        let range = (*self.available_freqs.last().unwrap() - *self.available_freqs.first().unwrap()) as f32;
        let percentage = if range > 0.0 {
            ((target_freq - *self.available_freqs.first().unwrap()) as f32 / range * 100.0) as u32
        } else { 0 };

        if target_freq != self.current_freq {
            log::debug!("FAS[P{}] {} {} → {} Hz ({:.1} MHz, {}%)",
                self.policy_id,
                if target_freq > self.current_freq { "↑" } else { "↓" },
                self.current_freq, target_freq,
                target_freq as f32 / 1000.0, percentage);
        }

        if target_freq >= self.current_freq {
            self.max_writer.write_value_force(target_freq);
            self.min_writer.write_value_force(target_freq);
        } else {
            self.min_writer.write_value_force(target_freq);
            self.max_writer.write_value_force(target_freq);
        }
        self.current_freq = target_freq;
    }

    /// 强制重写当前频率（用于 mismatch 修复）
    pub fn force_reapply(&mut self) {
        self.max_writer.invalidate();
        self.min_writer.invalidate();
        self.max_writer.write_value_force(self.current_freq);
        self.min_writer.write_value_force(self.current_freq);
    }
}

struct FpsWindow {
    buf: [f32; 120],
    pos: usize,
    filled: bool,
}

impl FpsWindow {
    fn new() -> Self { Self { buf: [0.0; 120], pos: 0, filled: false } }
    fn push(&mut self, fps: f32) {
        self.buf[self.pos] = fps;
        self.pos = (self.pos + 1) % 120;
        if self.pos == 0 { self.filled = true; }
    }
    fn max_fps(&self) -> f32 {
        let len = if self.filled { 120 } else { self.pos.max(1) };
        self.buf[..len].iter().copied().fold(0.0, f32::max)
    }
    fn mean(&self) -> f32 {
        let len = if self.filled { 120 } else { self.pos.max(1) };
        self.buf[..len].iter().sum::<f32>() / len as f32
    }
    /// 最近 n 帧的均值（短窗口，避免被120帧历史稀释）
    fn recent_mean(&self, n: usize) -> f32 {
        let total = if self.filled { 120 } else { self.pos };
        if total == 0 { return 0.0; }
        let count = n.min(total);
        let mut sum = 0.0;
        for i in 0..count {
            let idx = (self.pos + 120 - 1 - i) % 120;
            sum += self.buf[idx];
        }
        sum / count as f32
    }
    /// 最近 n 帧的最大值（用于 probe 等短窗口场景）
    fn recent_max(&self, n: usize) -> f32 {
        let total = if self.filled { 120 } else { self.pos };
        if total == 0 { return 0.0; }
        let count = n.min(total);
        let mut max_val: f32 = 0.0;
        for i in 0..count {
            let idx = (self.pos + 120 - 1 - i) % 120;
            if self.buf[idx] > max_val { max_val = self.buf[idx]; }
        }
        max_val
    }
    /// 标准差（用于场景过渡检测，CV = stddev/mean 判断帧率波动程度）
    fn stddev(&self) -> f32 {
        let len = if self.filled { 120 } else { self.pos.max(1) };
        let avg = self.mean();
        let variance: f32 = self.buf[..len].iter()
            .map(|&x| (x - avg) * (x - avg))
            .sum::<f32>() / len as f32;
        variance.sqrt()
    }
    fn count(&self) -> usize { if self.filled { 120 } else { self.pos } }
    fn clear(&mut self) { self.buf = [0.0; 120]; self.pos = 0; self.filled = false; }
}

// ── 常量 ──

/// 重帧序列中允许夹杂的非重帧容忍帧数
const LOADING_NORMAL_TOLERANCE: u32 = 3;
/// 时间窗口内反复进出 loading 达此次数则进入持续加载模式
const SUSTAINED_LOADING_CYCLE_THRESHOLD: u32 = 3;
/// 持续加载循环计数的时间窗口（10秒）
const SUSTAINED_LOADING_WINDOW_NS: u64 = 10_000_000_000;
/// 持续加载模式下退出后需观察的稳定帧数
const SUSTAINED_POST_LOADING_IGNORE: u32 = 30;
/// 退出 loading 后的降档保护帧数
const POST_LOADING_DOWNGRADE_GUARD: u32 = 90;

/// 软加载：avg_fps 低于目标帧率此比例时疑似加载
const SOFT_LOADING_FPS_RATIO: f32 = 0.5;
/// 软加载：perf 高于此阈值且 fps 仍低 → 提频无效
const SOFT_LOADING_PERF_THRESHOLD: f32 = 700.0;
/// 软加载：需连续满足条件的帧数
const SOFT_LOADING_CONFIRM_FRAMES: u32 = 30;
/// 软加载状态下 perf 压制目标
const SOFT_LOADING_PERF_CAP: f32 = 400.0;
/// 退出软加载需连续正常的帧数（均值恢复路径）
const SOFT_LOADING_EXIT_FRAMES: u32 = 45;
/// 突破检测：recent_mean 达目标此比例视为引擎解锁
const SOFT_LOADING_BREAKTHROUGH_FPS_RATIO: f32 = 0.65;
/// 突破检测的短窗口帧数
const SOFT_LOADING_BREAKTHROUGH_WINDOW: usize = 15;
/// 突破路径的退出确认帧数
const SOFT_LOADING_EXIT_FRAMES_BREAKTHROUGH: u32 = 20;
/// probe 间隔帧数
const SOFT_LOADING_PROBE_INTERVAL: u32 = 120;
/// probe 持续帧数
const SOFT_LOADING_PROBE_DURATION: u32 = 15;
/// probe 期间 perf 上限
const SOFT_LOADING_PROBE_PERF_CAP: f32 = 700.0;
/// probe 后帧率提升超此比例则退出软加载
const SOFT_LOADING_PROBE_FPS_GAIN_RATIO: f32 = 0.3;

/// 硬加载期间 perf 下限（保证加载有足够 CPU）
const LOADING_PERF_FLOOR: f32 = 600.0;
/// 硬加载期间 perf 上限（IO 密集型，高频浪费功耗）
const LOADING_PERF_CEILING: f32 = 700.0;
/// 退出 loading 后的重入冷却帧数，防止 exit→重帧→re-enter 死循环
const LOADING_REENTRY_COOLDOWN: u32 = 60;

/// 单帧间隔超此值视为应用切换/息屏（毫秒）
const APP_SWITCH_GAP_MS: f32 = 3000.0;
/// 应用切换恢复后的初始 perf
const APP_SWITCH_RESUME_PERF: f32 = 600.0;
/// 应用切换恢复后跳过的帧数
const APP_SWITCH_IGNORE_FRAMES: u32 = 8;

/// 定期强制重写频率的间隔帧数
const FREQ_FORCE_REAPPLY_INTERVAL: u32 = 30;

/// max_ns 固定上限（毫秒），消除高帧率下 heavy 检测盲区
const FIXED_MAX_FRAME_MS: f32 = 500.0;

/// 变异系数超此值视为场景过渡
const SCENE_TRANSITION_CV_THRESHOLD: f32 = 0.4;
/// 场景过渡期间暂停降档的帧数
const SCENE_TRANSITION_GUARD_FRAMES: u32 = 60;

/// crit 后的衰减冷却帧数
const JANK_COOLDOWN_FRAMES_CRIT: u32 = 20;
/// heavy 后的衰减冷却帧数
const JANK_COOLDOWN_FRAMES_HEAVY: u32 = 10;
/// bounce 后的衰减冷却帧数
const JANK_COOLDOWN_FRAMES_BOUNCE: u32 = 5;

pub struct FasController {
    fps_gears: Vec<f32>,
    fps_margin: f32,
    current_target_fps: f32,
    perf_index: f32,
    ema_actual_ms: f32,
    upgrade_confirm_frames: u32,
    downgrade_confirm_frames: u32,
    fps_window: FpsWindow,
    global_max_freq: u32,
    global_min_freq: u32,
    pub policies: Vec<PolicyController>,
    log_counter: u32,
    consecutive_normal_frames: u32,
    consecutive_loading_frames: u32,
    heavy_frame_streak_ms: f32,
    is_in_loading_state: bool,
    post_loading_ignore: u32,
    upgrade_cooldown: u32,
    gear_change_dampen_frames: u32,
    // 软加载检测
    soft_loading_confirm: u32,
    is_in_soft_loading: bool,
    soft_loading_exit_confirm: u32,
    soft_loading_frames_in_state: u32,
    soft_loading_probe_countdown: u32,
    soft_loading_probe_avg_before: f32,
    // 重帧容忍窗口
    normal_frame_tolerance: u32,
    // 持续加载检测
    loading_cycle_count: u32,
    loading_cycle_first_ns: u64,
    sustained_loading: bool,
    // 退出 loading 后的降档保护
    post_loading_downgrade_guard: u32,
    // 重入冷却
    loading_reentry_cooldown: u32,
    // 可配置参数
    heavy_frame_threshold_ms: f32,
    loading_cumulative_ms: f32,
    post_loading_ignore_frames: u32,
    post_loading_perf_min: f32,
    post_loading_perf_max: f32,
    instant_error_threshold_ms: f32,
    perf_floor: f32,
    freq_hysteresis: f32,
    // 帧时间累加器
    frame_time_accumulator_ns: u64,
    init_time: Instant,
    // 频率强制重写计数器
    freq_force_counter: u32,
    // 场景过渡降档保护
    scene_transition_guard: u32,
    // 掉帧后衰减冷却
    jank_cooldown: u32,
}

impl FasController {
    pub fn new() -> Self {
        Self {
            fps_gears: default_gears(),
            fps_margin: 3.0,
            current_target_fps: 60.0,
            perf_index: 400.0,
            ema_actual_ms: 0.0,
            upgrade_confirm_frames: 0,
            downgrade_confirm_frames: 0,
            fps_window: FpsWindow::new(),
            global_max_freq: 9999999,
            global_min_freq: 0,
            policies: Vec::new(),
            log_counter: 0,
            consecutive_normal_frames: 0,
            consecutive_loading_frames: 0,
            heavy_frame_streak_ms: 0.0,
            is_in_loading_state: false,
            post_loading_ignore: 0,
            upgrade_cooldown: 0,
            gear_change_dampen_frames: 0,
            soft_loading_confirm: 0,
            is_in_soft_loading: false,
            soft_loading_exit_confirm: 0,
            soft_loading_frames_in_state: 0,
            soft_loading_probe_countdown: 0,
            soft_loading_probe_avg_before: 0.0,
            normal_frame_tolerance: 0,
            loading_cycle_count: 0,
            loading_cycle_first_ns: 0,
            sustained_loading: false,
            post_loading_downgrade_guard: 0,
            loading_reentry_cooldown: 0,
            heavy_frame_threshold_ms: default_heavy_frame_ms(),
            loading_cumulative_ms: default_loading_cumulative_ms(),
            post_loading_ignore_frames: default_post_loading_ignore(),
            post_loading_perf_min: default_post_loading_perf_min(),
            post_loading_perf_max: default_post_loading_perf_max(),
            instant_error_threshold_ms: default_instant_error_threshold(),
            perf_floor: default_perf_floor(),
            freq_hysteresis: default_hysteresis(),
            frame_time_accumulator_ns: 0,
            init_time: Instant::now(),
            freq_force_counter: 0,
            scene_transition_guard: 0,
            jank_cooldown: 0,
        }
    }

    /// 基于最高档帧率计算 min_ns（保证任何档位都能看到快帧，避免升档自锁）
    fn max_gear_min_ns(&self) -> u64 {
        let max_gear = self.fps_gears.iter().copied().fold(60.0_f32, f32::max);
        let max_gear_budget_ns = (1_000_000_000.0 / max_gear) as u64;
        max_gear_budget_ns * 50 / 100
    }

    pub fn load_policies(&mut self, config: &Config) {
        self.policies.clear();
        let _ = crate::utils::try_write_file("/sys/module/perfmgr/parameters/perfmgr_enable", "0");
        let _ = crate::utils::try_write_file("/sys/module/mtk_fpsgo/parameters/perfmgr_enable", "0");
        log::debug!("FAS: disabled system FEAS/FPSGO");

        let root = crate::common::get_module_root();
        let rules_path = root.join("config/rules.yaml");

        if let Ok(content) = fs::read_to_string(&rules_path) {
            if let Ok(rules) = serde_yaml::from_str::<RulesYaml>(&content) {
                if !rules.fas_rules.fps_gears.is_empty() {
                    self.fps_gears = rules.fas_rules.fps_gears;
                }
                if let Ok(margin) = rules.fas_rules.fps_margin.parse::<f32>() {
                    self.fps_margin = margin;
                }
                self.heavy_frame_threshold_ms = rules.fas_rules.heavy_frame_threshold_ms;
                self.loading_cumulative_ms = rules.fas_rules.loading_cumulative_ms;
                self.post_loading_ignore_frames = rules.fas_rules.post_loading_ignore_frames;
                self.post_loading_perf_min = rules.fas_rules.post_loading_perf_min;
                self.post_loading_perf_max = rules.fas_rules.post_loading_perf_max;
                self.instant_error_threshold_ms = rules.fas_rules.instant_error_threshold_ms;
                self.perf_floor = rules.fas_rules.perf_floor;
                self.freq_hysteresis = rules.fas_rules.freq_hysteresis;
            }
        }

        let core_info = &config.core_framework;
        let clusters = vec![
            core_info.small_core_path,
            core_info.medium_core_path,
            core_info.big_core_path,
            core_info.super_big_core_path,
        ];

        let mut global_max = 0u32;
        let mut global_min = u32::MAX;

        for (_idx, &policy_id) in clusters.iter().enumerate() {
            if policy_id != -1 {
                let gov_path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_governor", policy_id);
                let _ = crate::utils::try_write_file(&gov_path, "performance");
                let avail_path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_available_frequencies", policy_id);

                let mut avail_freqs: Vec<u32> = fs::read_to_string(&avail_path)
                    .unwrap_or_default()
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if avail_freqs.is_empty() { continue; }
                avail_freqs.sort_unstable();
                avail_freqs.dedup();

                let min_f = *avail_freqs.first().unwrap();
                let max_f = *avail_freqs.last().unwrap();

                let mut max_writer = FastWriter::new(
                    format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_max_freq", policy_id));
                let mut min_writer = FastWriter::new(
                    format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_min_freq", policy_id));
                max_writer.write_value_force(max_f);
                min_writer.write_value_force(max_f);

                self.policies.push(PolicyController {
                    max_writer, min_writer,
                    available_freqs: avail_freqs,
                    current_freq: max_f,
                    policy_id: policy_id as usize,
                });

                if max_f > global_max { global_max = max_f; }
                if min_f < global_min { global_min = min_f; }
            }
        }

        self.global_max_freq = if global_max == 0 { 9999999 } else { global_max };
        self.global_min_freq = if global_min == u32::MAX { 0 } else { global_min };
        self.current_target_fps = *self.fps_gears.iter()
            .reduce(|a, b| if a > b { a } else { b }).unwrap_or(&60.0);

        self.perf_index = 400.0;
        self.ema_actual_ms = 0.0;
        self.upgrade_confirm_frames = 0;
        self.downgrade_confirm_frames = 0;
        self.fps_window = FpsWindow::new();
        self.log_counter = 0;
        self.consecutive_normal_frames = 0;
        self.consecutive_loading_frames = 0;
        self.heavy_frame_streak_ms = 0.0;
        self.is_in_loading_state = false;
        self.post_loading_ignore = 0;
        self.upgrade_cooldown = 0;
        self.gear_change_dampen_frames = 0;
        self.soft_loading_confirm = 0;
        self.is_in_soft_loading = false;
        self.soft_loading_exit_confirm = 0;
        self.soft_loading_frames_in_state = 0;
        self.soft_loading_probe_countdown = 0;
        self.soft_loading_probe_avg_before = 0.0;
        self.normal_frame_tolerance = 0;
        self.loading_cycle_count = 0;
        self.loading_cycle_first_ns = 0;
        self.sustained_loading = false;
        self.post_loading_downgrade_guard = 0;
        self.loading_reentry_cooldown = 0;
        self.frame_time_accumulator_ns = 0;
        self.freq_force_counter = 0;
        self.scene_transition_guard = 0;
        self.jank_cooldown = 0;

        info!("FAS init | target:{:.0}fps margin:{:.1} clusters:{} perf:{:.0}",
            self.current_target_fps, self.fps_margin, self.policies.len(), self.perf_index);
        info!("FAS config | heavy:{}ms loading:{}ms ignore:{} post_perf:{}-{} instant:{}ms floor:{} hyst:{}",
            self.heavy_frame_threshold_ms, self.loading_cumulative_ms,
            self.post_loading_ignore_frames, self.post_loading_perf_min, self.post_loading_perf_max,
            self.instant_error_threshold_ms, self.perf_floor, self.freq_hysteresis);
        for p in &self.policies {
            info!("FAS[P{}] {}-{} MHz (init: {} MHz)", p.policy_id,
                p.available_freqs.first().unwrap() / 1000,
                p.available_freqs.last().unwrap() / 1000,
                p.current_freq / 1000);
        }

        self.init_time = Instant::now();
        self.perf_index = 850.0;
        self.apply_freqs();
    }

    /// 将 perf_index 映射到频率并应用，含迟滞防抖
    fn apply_freqs(&mut self) {
        // 定期强制写入，对抗外部干扰
        self.freq_force_counter = self.freq_force_counter.wrapping_add(1);
        let force_this_cycle = self.freq_force_counter % FREQ_FORCE_REAPPLY_INTERVAL == 0;

        let ratio = self.perf_index / 1000.0;
        for policy in self.policies.iter_mut() {
            let pmin = *policy.available_freqs.first().unwrap() as f32;
            let pmax = *policy.available_freqs.last().unwrap() as f32;
            let target_val = pmin + ratio * (pmax - pmin);

            let target_freq = policy.available_freqs.iter().copied()
                .min_by(|&a, &b| {
                    ((a as f32 - target_val).abs())
                        .partial_cmp(&(b as f32 - target_val).abs()).unwrap()
                })
                .unwrap_or(pmax as u32);

            if target_freq != policy.current_freq {
                let cur_idx = policy.available_freqs.iter().position(|&f| f == policy.current_freq);
                let tgt_idx = policy.available_freqs.iter().position(|&f| f == target_freq);
                let apply = match (cur_idx, tgt_idx) {
                    (Some(ci), Some(ti)) if (ci as i32 - ti as i32).abs() == 1 => {
                        let cur_r = (policy.current_freq as f32 - pmin) / (pmax - pmin);
                        (ratio - cur_r).abs() > self.freq_hysteresis
                    }
                    _ => true,
                };
                if apply { policy.apply_freq_safe(target_freq); }
            } else if force_this_cycle {
                // 即使目标没变也定期重写，纠正被外部偷改的频率
                policy.force_reapply();
            }
        }
    }

    pub fn update_frame(&mut self, frame_delta_ns: u64) {
        if frame_delta_ns == 0 || self.policies.is_empty() { return; }

        // 冷启动 3.5 秒内保持高 perf 应对 Shader 编译
        if self.init_time.elapsed().as_millis() < 3500 {
            if self.perf_index < 850.0 {
                self.perf_index = 850.0;
                self.apply_freqs();
            }
            return;
        }

        self.frame_time_accumulator_ns = self.frame_time_accumulator_ns.wrapping_add(frame_delta_ns);

        let budget_ns = (1_000_000_000.0 / self.current_target_fps.max(1.0)) as u64;
        // min_ns 基于最高档帧率，保证低档位也能看到快帧（避免升档自锁）
        let min_ns = self.max_gear_min_ns();
        // max_ns 固定 500ms，消除高帧率下 heavy 检测盲区
        let max_ns = (FIXED_MAX_FRAME_MS * 1_000_000.0) as u64;
        let actual_ms = frame_delta_ns as f32 / 1_000_000.0;
        let budget_ms = budget_ns as f32 / 1_000_000.0;
        let is_heavy = actual_ms > self.heavy_frame_threshold_ms;

        if frame_delta_ns < min_ns { return; }

        // ── 应用切换/息屏检测（frame_delta > 3s 必为非渲染事件）──
        if actual_ms > APP_SWITCH_GAP_MS {
            let was_loading = self.is_in_loading_state;
            let was_soft = self.is_in_soft_loading;

            self.is_in_soft_loading = false;
            self.soft_loading_confirm = 0;
            self.soft_loading_exit_confirm = 0;
            self.soft_loading_frames_in_state = 0;
            self.soft_loading_probe_countdown = 0;
            self.scene_transition_guard = 0;
            self.jank_cooldown = 0;

            if was_loading || was_soft {
                // 之前在加载中，切回来大概率还在加载
                self.is_in_loading_state = false;
                self.consecutive_loading_frames = 0;
                self.heavy_frame_streak_ms = 0.0;
                self.normal_frame_tolerance = 0;
                self.perf_index = LOADING_PERF_CEILING;
                self.fps_window.clear();
                self.ema_actual_ms = 0.0;
                self.post_loading_ignore = APP_SWITCH_IGNORE_FRAMES;
                self.loading_reentry_cooldown = LOADING_REENTRY_COOLDOWN * 2;
                self.gear_change_dampen_frames = 90;
                self.post_loading_downgrade_guard = POST_LOADING_DOWNGRADE_GUARD;
                self.apply_freqs();

                log::info!("FAS: 📱 app switch (resume loading) ({:.0}ms gap) | \
                    Perf→{:.0} | ignore {} | reentry_cd {}",
                    actual_ms, self.perf_index,
                    self.post_loading_ignore, self.loading_reentry_cooldown);
            } else {
                // 正常游玩时的 app switch
                self.is_in_loading_state = false;
                self.consecutive_loading_frames = 0;
                self.heavy_frame_streak_ms = 0.0;
                self.normal_frame_tolerance = 0;

                self.fps_window.clear();
                self.ema_actual_ms = 0.0;
                self.downgrade_confirm_frames = 0;
                self.upgrade_confirm_frames = 0;
                self.perf_index = APP_SWITCH_RESUME_PERF;
                self.post_loading_ignore = APP_SWITCH_IGNORE_FRAMES;
                self.gear_change_dampen_frames = 60;
                self.post_loading_downgrade_guard = POST_LOADING_DOWNGRADE_GUARD;
                self.loading_reentry_cooldown = LOADING_REENTRY_COOLDOWN;
                self.apply_freqs();

                log::info!("FAS: 📱 app switch detected ({:.0}ms gap) | \
                    Perf→{:.0} | ignore {} | guard {}",
                    actual_ms, self.perf_index,
                    self.post_loading_ignore, self.post_loading_downgrade_guard);
            }
            return;
        }

        // ── 重帧 & 硬加载状态机 ──
        if is_heavy {
            // 重入冷却期：重帧交给 perf 提升处理，不触发 loading（防死循环）
            if self.loading_reentry_cooldown > 0 {
                self.perf_index = (self.perf_index + 30.0).min(1000.0);
                self.apply_freqs();
                self.loading_reentry_cooldown -= 1;
                log::debug!("FAS: heavy {:.1}ms during reentry cooldown ({}), boost perf→{:.0}",
                    actual_ms, self.loading_reentry_cooldown, self.perf_index);
                return;
            }

            self.consecutive_loading_frames += 1;
            self.heavy_frame_streak_ms += actual_ms;
            self.normal_frame_tolerance = 0;

            if !self.is_in_loading_state && self.heavy_frame_streak_ms > self.loading_cumulative_ms {
                self.is_in_loading_state = true;

                // 记录 loading 循环次数
                let now = self.frame_time_accumulator_ns;
                if self.loading_cycle_count == 0
                    || now.wrapping_sub(self.loading_cycle_first_ns) > SUSTAINED_LOADING_WINDOW_NS
                {
                    self.loading_cycle_count = 1;
                    self.loading_cycle_first_ns = now;
                } else {
                    self.loading_cycle_count += 1;
                }

                if !self.sustained_loading
                    && self.loading_cycle_count >= SUSTAINED_LOADING_CYCLE_THRESHOLD
                {
                    self.sustained_loading = true;
                    log::info!("FAS: 🔒 enter sustained loading ({}x cycles in window)",
                        self.loading_cycle_count);
                }

                // 硬加载优先级高于软加载
                if self.is_in_soft_loading {
                    self.is_in_soft_loading = false;
                    self.soft_loading_confirm = 0;
                    self.soft_loading_exit_confirm = 0;
                    self.soft_loading_frames_in_state = 0;
                    self.soft_loading_probe_countdown = 0;
                }

                // 进入 loading：perf 钳位到 600-700（足够做解压，不过热）
                let old = self.perf_index;
                self.perf_index = self.perf_index.clamp(LOADING_PERF_FLOOR, LOADING_PERF_CEILING);
                if old != self.perf_index {
                    self.apply_freqs();
                }
                log::info!("FAS: 🔄 enter loading ({} frames, {:.0}ms) | Perf {:.0}→{:.0}{}",
                    self.consecutive_loading_frames, self.heavy_frame_streak_ms, old, self.perf_index,
                    if self.sustained_loading { " [sustained]" } else { "" });
            }
            log::debug!("FAS: heavy {:.1}ms ({:.1}x) [streak:{}, {:.0}ms]",
                actual_ms, actual_ms / budget_ms, self.consecutive_loading_frames, self.heavy_frame_streak_ms);
            return;
        } else {
            // 非重帧，使用容忍窗口而非立即打断
            if self.consecutive_loading_frames > 0 {
                self.normal_frame_tolerance += 1;
                if self.normal_frame_tolerance < LOADING_NORMAL_TOLERANCE {
                    log::debug!("FAS: loading tolerance {}/{} (non-heavy {:.1}ms in streak)",
                        self.normal_frame_tolerance, LOADING_NORMAL_TOLERANCE, actual_ms);
                    return;
                } else {
                    log::debug!("FAS: burst end ({} frames, {:.0}ms, tolerance exhausted)",
                        self.consecutive_loading_frames, self.heavy_frame_streak_ms);
                    self.consecutive_loading_frames = 0;
                    self.heavy_frame_streak_ms = 0.0;
                    self.normal_frame_tolerance = 0;
                }
            }

            // 退出硬加载状态
            if self.is_in_loading_state {
                self.is_in_loading_state = false;
                self.fps_window.clear();
                self.downgrade_confirm_frames = 0;
                self.ema_actual_ms = 0.0;
                let old = self.perf_index;

                if self.sustained_loading {
                    self.perf_index = self.perf_index.clamp(self.post_loading_perf_min, self.post_loading_perf_max);
                    self.post_loading_ignore = SUSTAINED_POST_LOADING_IGNORE;
                    self.gear_change_dampen_frames = 120;
                    self.post_loading_downgrade_guard = POST_LOADING_DOWNGRADE_GUARD + 60;
                    self.loading_reentry_cooldown = LOADING_REENTRY_COOLDOWN;
                    log::info!("FAS: ✅ exit loading [sustained] | Perf {:.0}→{:.0} | ignore {} | guard {} | reentry_cd {}",
                        old, self.perf_index, self.post_loading_ignore, self.post_loading_downgrade_guard,
                        self.loading_reentry_cooldown);
                } else {
                    self.perf_index = self.perf_index.clamp(self.post_loading_perf_min, self.post_loading_perf_max);
                    self.post_loading_ignore = self.post_loading_ignore_frames;
                    self.gear_change_dampen_frames = 60;
                    self.post_loading_downgrade_guard = POST_LOADING_DOWNGRADE_GUARD;
                    self.loading_reentry_cooldown = LOADING_REENTRY_COOLDOWN;
                    log::info!("FAS: ✅ exit loading | Perf {:.0}→{:.0} | ignore {} | guard {} | reentry_cd {}",
                        old, self.perf_index, self.post_loading_ignore, self.post_loading_downgrade_guard,
                        self.loading_reentry_cooldown);
                }
            }
        }

        if self.is_in_loading_state { return; }
        if self.post_loading_ignore > 0 {
            self.post_loading_ignore -= 1;
            return;
        }
        if frame_delta_ns > max_ns { return; }

        let current_fps = 1_000_000_000.0 / frame_delta_ns as f32;
        self.fps_window.push(current_fps);
        let _max_fps = self.fps_window.max_fps();
        let avg_fps = self.fps_window.mean();

        let next_gear = self.fps_gears.iter().copied()
            .filter(|&g| g > self.current_target_fps + 0.5).reduce(f32::min);
        let prev_gear = self.fps_gears.iter().copied()
            .filter(|&g| g < self.current_target_fps - 0.5).reduce(f32::max);

        if self.upgrade_cooldown > 0 { self.upgrade_cooldown -= 1; }
        if self.gear_change_dampen_frames > 0 { self.gear_change_dampen_frames -= 1; }
        if self.post_loading_downgrade_guard > 0 { self.post_loading_downgrade_guard -= 1; }
        if self.loading_reentry_cooldown > 0 { self.loading_reentry_cooldown -= 1; }
        if self.scene_transition_guard > 0 { self.scene_transition_guard -= 1; }
        if self.jank_cooldown > 0 { self.jank_cooldown -= 1; }

        // ── 场景过渡检测（CV = stddev/mean，稳定时<0.1，过渡时>0.5）──
        if self.fps_window.count() >= 20 {
            let cv = if avg_fps > 1.0 { self.fps_window.stddev() / avg_fps } else { 0.0 };
            if cv > SCENE_TRANSITION_CV_THRESHOLD {
                if self.scene_transition_guard == 0 {
                    log::info!("FAS: ⚡ scene transition detected (CV={:.2}, avg={:.1}, std={:.1}) | guard {}",
                        cv, avg_fps, self.fps_window.stddev(), SCENE_TRANSITION_GUARD_FRAMES);
                }
                self.scene_transition_guard = SCENE_TRANSITION_GUARD_FRAMES;
            }
        }

        // 持续加载模式超时恢复
        if self.sustained_loading && !self.is_in_loading_state {
            let now = self.frame_time_accumulator_ns;
            if now.wrapping_sub(self.loading_cycle_first_ns) > SUSTAINED_LOADING_WINDOW_NS * 2 {
                log::info!("FAS: 🔓 sustained loading cleared (stable for >{}s)",
                    SUSTAINED_LOADING_WINDOW_NS * 2 / 1_000_000_000);
                self.sustained_loading = false;
                self.loading_cycle_count = 0;
            }
        }

        // ── 软加载检测（高 perf + 低 fps = 提频无效，瓶颈不在 CPU）──
        let soft_loading_fps_threshold = self.current_target_fps * SOFT_LOADING_FPS_RATIO;

        if !self.is_in_soft_loading {
            if avg_fps < soft_loading_fps_threshold
                && self.perf_index >= SOFT_LOADING_PERF_THRESHOLD
                && self.fps_window.count() >= 15
            {
                self.soft_loading_confirm += 1;
                if self.soft_loading_confirm >= SOFT_LOADING_CONFIRM_FRAMES {
                    self.is_in_soft_loading = true;
                    self.soft_loading_exit_confirm = 0;
                    self.soft_loading_frames_in_state = 0;
                    self.soft_loading_probe_countdown = 0;
                    let old = self.perf_index;
                    self.perf_index = SOFT_LOADING_PERF_CAP;
                    self.apply_freqs();
                    log::info!("FAS: 🌀 enter soft loading | avg:{:.1} < {:.0}×{:.0}% \
                        & perf:{:.0}>={:.0} | Perf {:.0}→{:.0}",
                        avg_fps, self.current_target_fps, SOFT_LOADING_FPS_RATIO * 100.0,
                        old, SOFT_LOADING_PERF_THRESHOLD, old, self.perf_index);
                }
            } else {
                self.soft_loading_confirm = 0;
            }
        } else {
            // ── 软加载状态中 ──
            self.soft_loading_frames_in_state += 1;

            // ── probe：定期短暂放开频率限制，观察帧率是否响应（防死锁逃生）──
            let in_probe = self.soft_loading_probe_countdown > 0;

            if !in_probe {
                if self.perf_index > SOFT_LOADING_PERF_CAP {
                    self.perf_index = SOFT_LOADING_PERF_CAP;
                }

                if self.soft_loading_frames_in_state % SOFT_LOADING_PROBE_INTERVAL == 0
                    && self.soft_loading_frames_in_state > 0
                {
                    self.soft_loading_probe_countdown = SOFT_LOADING_PROBE_DURATION;
                    self.soft_loading_probe_avg_before = avg_fps;
                    log::info!("FAS: 🔬 soft loading probe start | baseline avg:{:.1} | lifting cap to {:.0}",
                        avg_fps, SOFT_LOADING_PROBE_PERF_CAP);
                }
            } else {
                if self.perf_index > SOFT_LOADING_PROBE_PERF_CAP {
                    self.perf_index = SOFT_LOADING_PROBE_PERF_CAP;
                }
                self.soft_loading_probe_countdown -= 1;

                // probe 结束评估：用 recent_mean 而非 max_fps（抗单帧噪声）
                if self.soft_loading_probe_countdown == 0 {
                    let probe_recent_avg = self.fps_window.recent_mean(SOFT_LOADING_PROBE_DURATION as usize);
                    let probe_recent_max = self.fps_window.recent_max(SOFT_LOADING_PROBE_DURATION as usize);
                    let gain = if self.soft_loading_probe_avg_before > 0.1 {
                        (probe_recent_avg - self.soft_loading_probe_avg_before) / self.soft_loading_probe_avg_before
                    } else {
                        0.0
                    };

                    if gain >= SOFT_LOADING_PROBE_FPS_GAIN_RATIO {
                        // 帧率有明显提升 → 提频有效，退出软加载
                        self.is_in_soft_loading = false;
                        self.soft_loading_confirm = 0;
                        self.soft_loading_exit_confirm = 0;
                        self.soft_loading_frames_in_state = 0;
                        self.fps_window.clear();
                        self.downgrade_confirm_frames = 0;
                        self.ema_actual_ms = 0.0;
                        self.post_loading_downgrade_guard = POST_LOADING_DOWNGRADE_GUARD / 2;
                        self.gear_change_dampen_frames = 60;
                        log::info!("FAS: ✅ exit soft loading [probe] | recent_avg:{:.1} recent_max:{:.1} vs baseline:{:.1} gain:{:+.0}% | guard {}",
                            probe_recent_avg, probe_recent_max, self.soft_loading_probe_avg_before,
                            gain * 100.0, self.post_loading_downgrade_guard);
                    } else {
                        // 帧率没响应 → 确实在加载，重新压制
                        self.perf_index = SOFT_LOADING_PERF_CAP;
                        self.apply_freqs();
                        log::info!("FAS: 🔬 soft loading probe end | recent_avg:{:.1} recent_max:{:.1} vs baseline:{:.1} gain:{:+.0}% | still loading, re-cap",
                            probe_recent_avg, probe_recent_max, self.soft_loading_probe_avg_before, gain * 100.0);
                    }
                }
            }

            // ── 常规退出检测（与 probe 并行）──
            let is_avg_recovered = avg_fps >= self.current_target_fps * 0.7;
            // 突破检测用 recent_mean 而非 max_fps（防偶发快帧假突破）
            let recent = self.fps_window.recent_mean(SOFT_LOADING_BREAKTHROUGH_WINDOW);
            let is_breakthrough = recent >= self.current_target_fps * SOFT_LOADING_BREAKTHROUGH_FPS_RATIO;

            if is_avg_recovered || is_breakthrough {
                self.soft_loading_exit_confirm += 1;
                let exit_frames_needed = if is_breakthrough {
                    SOFT_LOADING_EXIT_FRAMES_BREAKTHROUGH
                } else {
                    SOFT_LOADING_EXIT_FRAMES
                };

                if self.soft_loading_exit_confirm >= exit_frames_needed {
                    self.is_in_soft_loading = false;
                    self.soft_loading_confirm = 0;
                    self.soft_loading_exit_confirm = 0;
                    self.soft_loading_frames_in_state = 0;
                    self.soft_loading_probe_countdown = 0;
                    self.fps_window.clear();
                    self.downgrade_confirm_frames = 0;
                    self.ema_actual_ms = 0.0;
                    self.post_loading_downgrade_guard = POST_LOADING_DOWNGRADE_GUARD / 2;
                    self.gear_change_dampen_frames = 60;
                    log::info!("FAS: ✅ exit soft loading | avg:{:.1} recent15:{:.1} {} | guard {}",
                        avg_fps, recent,
                        if is_breakthrough { "[breakthrough]" } else { "[avg-recovered]" },
                        self.post_loading_downgrade_guard);
                }
            } else {
                self.soft_loading_exit_confirm = 0;
            }

            // 软加载期间阻止降档
            self.downgrade_confirm_frames = 0;
        }

        // ── 升档（用 recent_mean(30) 替代 max_fps，抗噪声防 ping-pong）──
        let recent30 = self.fps_window.recent_mean(30);
        if let Some(target) = next_gear {
            if self.upgrade_cooldown > 0 {
                self.upgrade_confirm_frames = 0;
            } else if recent30 >= target - 10.0
                && avg_fps >= self.current_target_fps * 0.9
                && self.fps_window.count() >= 60
            {
                self.upgrade_confirm_frames += 1;
                self.downgrade_confirm_frames = 0;
                if self.upgrade_confirm_frames >= 60 {
                    log::info!("FAS: 🚀 {:.0}→{:.0}fps (recent30={:.1} avg={:.1})",
                        self.current_target_fps, target, recent30, avg_fps);
                    self.current_target_fps = target;
                    self.upgrade_confirm_frames = 0;
                    self.ema_actual_ms = 0.0;
                    self.fps_window.clear();
                    self.gear_change_dampen_frames = 90;
                }
            } else {
                if avg_fps >= self.current_target_fps - 5.0
                    && self.perf_index < 300.0
                    && self.upgrade_cooldown == 0
                {
                    self.upgrade_confirm_frames += 1;
                    if self.upgrade_confirm_frames >= 120 {
                        log::info!("FAS: 🔍 probe {:.0}→{:.0}fps (avg={:.1} perf={:.0})",
                            self.current_target_fps, target, avg_fps, self.perf_index);
                        self.current_target_fps = target;
                        self.upgrade_confirm_frames = 0;
                        self.ema_actual_ms = 0.0;
                        self.fps_window.clear();
                        self.perf_index = (self.perf_index + 200.0).min(600.0);
                        self.gear_change_dampen_frames = 90;
                    }
                } else {
                    self.upgrade_confirm_frames = 0;
                }
            }
        } else {
            self.upgrade_confirm_frames = 0;
        }

        // ── 降档（不削减 perf 避免瀑布式崩溃；场景过渡期间暂停）──
        if let Some(target) = prev_gear {
            if self.is_in_soft_loading {
                // 软加载期间已阻止降档
            } else if self.post_loading_downgrade_guard > 0 {
                self.downgrade_confirm_frames = 0;
                log::debug!("FAS: downgrade blocked (post-loading guard: {})",
                    self.post_loading_downgrade_guard);
            } else if self.scene_transition_guard > 0 {
                self.downgrade_confirm_frames = 0;
                log::debug!("FAS: downgrade blocked (scene transition guard: {})",
                    self.scene_transition_guard);
            } else if avg_fps < self.current_target_fps - 10.0 {
                self.downgrade_confirm_frames += 1;
                if self.downgrade_confirm_frames >= 30 {
                    let old_fps = self.current_target_fps;
                    self.current_target_fps = target;
                    self.downgrade_confirm_frames = 0;
                    self.ema_actual_ms = 0.0;
                    // 不削减 perf_index，降档已降低目标，同 perf 更容易满足
                    self.fps_window.clear();
                    self.upgrade_cooldown = 150;
                    self.upgrade_confirm_frames = 0;
                    self.gear_change_dampen_frames = 60;
                    log::info!("FAS: 💤 {:.0}→{:.0}fps (avg={:.1}) perf={:.0} cd={}",
                        old_fps, target, avg_fps, self.perf_index, self.upgrade_cooldown);
                }
            } else {
                self.downgrade_confirm_frames = 0;
            }
        }

        // ── 双预算 & EMA ──
        let ema_budget = 1000.0 / (self.current_target_fps - self.fps_margin).max(1.0);
        let inst_budget = 1000.0 / self.current_target_fps;

        if self.ema_actual_ms <= 0.0 {
            self.ema_actual_ms = actual_ms;
        } else {
            // EMA 不对称：坏帧(actual>ema) alpha=0.45 快速感知，好帧 alpha=0.12 缓慢放松
            let a = if actual_ms > self.ema_actual_ms { 0.45 } else { 0.12 };
            self.ema_actual_ms = self.ema_actual_ms * (1.0 - a) + actual_ms * a;
        }

        let ema_err = ema_budget - self.ema_actual_ms;
        let inst_err = inst_budget - actual_ms;
        let act;

        // ── 蹦床 v6 ──
        let old_perf = self.perf_index;
        let damped = self.gear_change_dampen_frames > 0;
        let in_scene_transition = self.scene_transition_guard > 0;

        if inst_err < -self.instant_error_threshold_ms {
            self.perf_index += if damped { 40.0 } else { 80.0 };
            act = if damped { "crit-d(+40)" } else { "crit(+80)" };
            self.consecutive_normal_frames = 0;
            self.jank_cooldown = JANK_COOLDOWN_FRAMES_CRIT;
        } else if ema_err < -2.0 {
            self.perf_index += if damped { 15.0 } else { 40.0 };
            act = if damped { "heavy-d(+15)" } else { "heavy(+40)" };
            self.consecutive_normal_frames = 0;
            self.jank_cooldown = self.jank_cooldown.max(JANK_COOLDOWN_FRAMES_HEAVY);
        } else if ema_err < -0.5 {
            self.perf_index += if damped { 3.0 } else { 5.0 };
            act = if damped { "bounce-d(+3)" } else { "bounce(+5)" };
            self.consecutive_normal_frames = 0;
            self.jank_cooldown = self.jank_cooldown.max(JANK_COOLDOWN_FRAMES_BOUNCE);
        } else {
            self.consecutive_normal_frames += 1;

            let in_jank_cooldown = self.jank_cooldown > 0;
            // 低 perf 衰减减速（perf<400 时按比例缩减，防底部挣扎）
            let low_perf_factor = if self.perf_index < 400.0 {
                (self.perf_index / 400.0).max(0.3)
            } else {
                1.0
            };

            if ema_err < 1.0 || in_jank_cooldown {
                // 冷却期或微余量：fine 级别衰减
                let base = if in_scene_transition { 1.5 } else { 3.0 };
                let d = base * low_perf_factor;
                self.perf_index -= d;
                act = if in_jank_cooldown {
                    "fine-jc"
                } else if in_scene_transition {
                    "fine-s(-1.5)"
                } else {
                    "fine(-3)"
                };
            } else if ema_err < 3.0 && !in_jank_cooldown {
                let base = if in_scene_transition { 3.0 } else { 8.0 };
                let d = base * low_perf_factor;
                self.perf_index -= d;
                act = if in_scene_transition { "surplus-s(-3)" } else { "surplus(-8)" };
            } else if !in_jank_cooldown {
                let base = if in_scene_transition { 5.0 } else { 15.0 };
                let d = base * low_perf_factor;
                self.perf_index -= d;
                act = if in_scene_transition { "excess-s(-5)" } else { "excess(-15)" };
            } else {
                // 理论不可达，编译安全保留
                let d = 1.5 * low_perf_factor;
                self.perf_index -= d;
                act = "fine-jc";
            }

            // fast decay：连续正常帧且 perf 高时快速降频（场景过渡和冷却期间禁止）
            if self.consecutive_normal_frames >= 30 && self.perf_index > 600.0
                && !in_scene_transition && !in_jank_cooldown
            {
                self.perf_index -= 80.0;
                log::debug!("FAS: fast decay after {} frames", self.consecutive_normal_frames);
                self.consecutive_normal_frames = 0;
            }
        }

        self.perf_index = self.perf_index.clamp(self.perf_floor, 1000.0);
        let max_inc = if damped { 50.0 } else { 100.0 };
        if self.perf_index > old_perf + max_inc { self.perf_index = old_perf + max_inc; }
        if damped && self.perf_index > 900.0 { self.perf_index = 900.0; }

        // 软加载期间再次压制 perf（probe 期间用更高上限）
        if self.is_in_soft_loading {
            let cap = if self.soft_loading_probe_countdown > 0 {
                SOFT_LOADING_PROBE_PERF_CAP
            } else {
                SOFT_LOADING_PERF_CAP
            };
            if self.perf_index > cap {
                self.perf_index = cap;
            }
        }

        // ── 心跳（每30帧）：日志 + 频率 mismatch 检测 ──
        self.log_counter = self.log_counter.wrapping_add(1);
        if self.log_counter % 30 == 0 {
            log::info!("FAS | {:.0}fps avg:{:.1} | {:.2}ms ema:{:.2} | err:{:+.2}/{:+.2} | {} | P:{:.0}{}{}{}{}{}{}",
                self.current_target_fps, avg_fps, actual_ms, self.ema_actual_ms,
                ema_err, inst_err, act, self.perf_index,
                if self.upgrade_cooldown > 0 { format!(" cd:{}", self.upgrade_cooldown) } else { String::new() },
                if damped { format!(" damp:{}", self.gear_change_dampen_frames) } else { String::new() },
                if self.post_loading_downgrade_guard > 0 { format!(" guard:{}", self.post_loading_downgrade_guard) } else { String::new() },
                if self.is_in_soft_loading { " [soft-load]".to_string() } else { String::new() },
                if self.scene_transition_guard > 0 { format!(" [scene:{}]", self.scene_transition_guard) } else { String::new() },
                if self.jank_cooldown > 0 { format!(" [jank-cd:{}]", self.jank_cooldown) } else { String::new() });

            // 频率验证：检测 scaling_cur_freq 与设定值是否一致（5% 容忍度）
            let mut needs_reapply = false;
            for p in &self.policies {
                if let Ok(s) = fs::read_to_string(
                    format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_cur_freq", p.policy_id)) {
                    if let Ok(actual_freq) = s.trim().parse::<u32>() {
                        let diff = (actual_freq as i64 - p.current_freq as i64).unsigned_abs();
                        let threshold = (p.current_freq as u64) / 20;
                        if diff > threshold {
                            log::warn!("FAS[P{}] freq mismatch: set={} actual={} MHz (diff {}%)",
                                p.policy_id, p.current_freq / 1000, actual_freq / 1000,
                                diff * 100 / p.current_freq as u64);
                            needs_reapply = true;
                        }
                    }
                }
            }

            if needs_reapply {
                log::info!("FAS: 🔧 freq mismatch detected, force reapplying all policies");
                for p in self.policies.iter_mut() {
                    p.force_reapply();
                }
            }
        }

        self.apply_freqs();
    }
}