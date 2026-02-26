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
use std::sync::mpsc;
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
    #[serde(default = "default_perf_ceil")]
    perf_ceil: f32,
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
            perf_ceil: default_perf_ceil(),
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
fn default_perf_ceil() -> f32 { 850.0 }

/// sysfs 频率写入器，带缓存
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
        Self { file, last_value: None }
    }

    pub fn write_value(&mut self, value: u32) {
        if self.last_value == Some(value) { return; }
        self.do_write(value);
    }

    pub fn write_value_force(&mut self, value: u32) {
        self.do_write(value);
    }

    pub fn invalidate(&mut self) {
        self.last_value = None;
    }

    fn do_write(&mut self, value: u32) {
        if let Some(file) = &mut self.file {
            let val_str = format!("{}\n", value);
            let _ = file.seek(SeekFrom::Start(0));
            if let Err(e) = file.write_all(val_str.as_bytes()) {
                log::error!("FAS: failed to write freq {}: {}", value, e);
            }
            self.last_value = Some(value);
        }
    }
}

/// CPU 频率策略控制器
pub struct PolicyController {
    pub max_writer: FastWriter,
    pub min_writer: FastWriter,
    pub available_freqs: Vec<u32>,
    pub current_freq: u32,
    pub policy_id: usize,
    pub mismatch_count: u32,
    pub external_lock_cooldown: u32,
}

impl PolicyController {
    pub fn apply_freq_safe(&mut self, target_freq: u32) {
        let range = (*self.available_freqs.last().unwrap()
            - *self.available_freqs.first().unwrap()) as f32;
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

    pub fn force_reapply(&mut self) {
        self.max_writer.invalidate();
        self.min_writer.invalidate();
        self.min_writer.write_value_force(self.current_freq);
        self.max_writer.write_value_force(self.current_freq);
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

const LOADING_NORMAL_TOLERANCE: u32 = 3;
const SUSTAINED_LOADING_CYCLE_THRESHOLD: u32 = 3;
const SUSTAINED_LOADING_WINDOW_NS: u64 = 10_000_000_000;
const SUSTAINED_POST_LOADING_IGNORE: u32 = 30;
const POST_LOADING_DOWNGRADE_GUARD: u32 = 90;

const SOFT_LOADING_FPS_RATIO: f32 = 0.5;
const SOFT_LOADING_PERF_THRESHOLD: f32 = 700.0;
const SOFT_LOADING_CONFIRM_FRAMES: u32 = 30;
const SOFT_LOADING_PERF_CAP: f32 = 400.0;
const SOFT_LOADING_EXIT_FRAMES: u32 = 45;
const SOFT_LOADING_BREAKTHROUGH_FPS_RATIO: f32 = 0.65;
const SOFT_LOADING_BREAKTHROUGH_WINDOW: usize = 15;
const SOFT_LOADING_EXIT_FRAMES_BREAKTHROUGH: u32 = 20;
const SOFT_LOADING_PROBE_INTERVAL: u32 = 120;
const SOFT_LOADING_PROBE_DURATION: u32 = 15;
const SOFT_LOADING_PROBE_PERF_CAP: f32 = 700.0;
const SOFT_LOADING_PROBE_FPS_GAIN_RATIO: f32 = 0.3;
const SOFT_LOADING_PROBE_FAIL_DECAY_STEP: f32 = 100.0;

// 软加载内降档：连续稳定在某齿轮附近时直接降档
const SOFT_LOADING_DOWNGRADE_CHECK_FRAMES: u32 = 45;
const SOFT_LOADING_GEAR_MATCH_TOLERANCE: f32 = 8.0;

const LOADING_PERF_FLOOR: f32 = 600.0;
const LOADING_PERF_CEILING: f32 = 700.0;
const LOADING_REENTRY_COOLDOWN: u32 = 60;

const APP_SWITCH_GAP_MS: f32 = 3000.0;
const APP_SWITCH_RESUME_PERF: f32 = 600.0;
const APP_SWITCH_IGNORE_FRAMES: u32 = 8;

const FREQ_FORCE_REAPPLY_INTERVAL: u32 = 30;
const FIXED_MAX_FRAME_MS: f32 = 500.0;

const SCENE_TRANSITION_CV_THRESHOLD: f32 = 0.45;
const SCENE_TRANSITION_GUARD_FRAMES: u32 = 40;

const JANK_COOLDOWN_FRAMES_CRIT: u32 = 10;
const JANK_COOLDOWN_FRAMES_HEAVY: u32 = 5;

const SCENE_TRANSITION_MAX_CONTINUOUS: u32 = 120;
const SCENE_TRANSITION_FPS_FLOOR_RATIO: f32 = 0.3;
const SCENE_TRANSITION_FORCE_EXIT_FRAMES: u32 = 15;

const MISMATCH_LOCK_THRESHOLD: u32 = 8;
const MISMATCH_REAPPLY_SKIP_CYCLES: u32 = 3;

const DOWNGRADE_CONFIRM_FRAMES: u32 = 90;
const DOWNGRADE_BOOST_PERF_INC: f32 = 150.0;
const DOWNGRADE_BOOST_DURATION: u32 = 45;
const DOWNGRADE_PROXIMITY_RATIO: f32 = 0.92;
const UPGRADE_COOLDOWN_AFTER_DOWNGRADE: u32 = 90;
const STABLE_FORGIVE_FRAMES: u32 = 900;

const FAST_DECAY_FRAME_THRESHOLD: u32 = 60;
const FAST_DECAY_PERF_THRESHOLD: f32 = 650.0;
const FAST_DECAY_MAX_STEP: f32 = 30.0;
const FAST_DECAY_MIN_STEP: f32 = 5.0;
const FAST_DECAY_POST_JANK_SUPPRESS: u32 = 90;

// deficit 抑制阈值：EMA误差浅于此值时不增P，改为微衰减
const DEFICIT_SUPPRESS_MS: f32 = 0.3;


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
    soft_loading_confirm: u32,
    is_in_soft_loading: bool,
    soft_loading_exit_confirm: u32,
    soft_loading_frames_in_state: u32,
    soft_loading_probe_countdown: u32,
    soft_loading_probe_avg_before: f32,
    normal_frame_tolerance: u32,
    loading_cycle_count: u32,
    loading_cycle_first_ns: u64,
    sustained_loading: bool,
    post_loading_downgrade_guard: u32,
    loading_reentry_cooldown: u32,
    heavy_frame_threshold_ms: f32,
    loading_cumulative_ms: f32,
    post_loading_ignore_frames: u32,
    post_loading_perf_min: f32,
    post_loading_perf_max: f32,
    instant_error_threshold_ms: f32,
    perf_floor: f32,
    freq_hysteresis: f32,
    perf_ceil: f32,
    frame_time_accumulator_ns: u64,
    init_time: Instant,
    freq_force_counter: u32,
    scene_transition_guard: u32,
    scene_transition_continuous: u32,
    scene_transition_low_fps_frames: u32,
    jank_cooldown: u32,
    downgrade_boost_active: bool,
    downgrade_boost_remaining: u32,
    downgrade_boost_perf_saved: f32,
    consecutive_downgrade_count: u32,
    last_downgrade_from_fps: f32,
    stable_gear_frames: u32,
    mismatch_result_rx: Option<mpsc::Receiver<Vec<(usize, u32)>>>,
    post_jank_no_decay_frames: u32,
    mismatch_probe_skip: u32,
    // 软加载内齿轮匹配计数器
    soft_loading_gear_match_frames: u32,
    soft_loading_matched_gear: f32,
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
            perf_ceil: default_perf_ceil(),
            frame_time_accumulator_ns: 0,
            init_time: Instant::now(),
            freq_force_counter: 0,
            scene_transition_guard: 0,
            scene_transition_continuous: 0,
            scene_transition_low_fps_frames: 0,
            jank_cooldown: 0,
            downgrade_boost_active: false,
            downgrade_boost_remaining: 0,
            downgrade_boost_perf_saved: 0.0,
            consecutive_downgrade_count: 0,
            last_downgrade_from_fps: 0.0,
            stable_gear_frames: 0,
            mismatch_result_rx: None,
            post_jank_no_decay_frames: 0,
            mismatch_probe_skip: 0,
            soft_loading_gear_match_frames: 0,
            soft_loading_matched_gear: 0.0,
        }
    }

    fn max_gear_min_ns(&self) -> u64 {
        let max_gear = self.fps_gears.iter().copied().fold(60.0_f32, f32::max);
        let max_gear_budget_ns = (1_000_000_000.0 / max_gear) as u64;
        max_gear_budget_ns * 50 / 100
    }

    /// 查找最接近给定fps的低齿轮
    fn find_nearest_lower_gear(&self, fps: f32) -> Option<f32> {
        self.fps_gears.iter().copied()
            .filter(|&g| g <= fps + SOFT_LOADING_GEAR_MATCH_TOLERANCE
                      && g < self.current_target_fps - 0.5)
            .reduce(f32::max)
    }

    /// 检测FPS是否稳定在某个齿轮附近
    fn detect_native_gear(&self, avg_fps: f32) -> Option<f32> {
        if self.fps_window.count() < 20 { return None; }
        let stddev = self.fps_window.stddev();
        // FPS波动小（stddev < 10%均值），说明是原生帧率而非加载
        if avg_fps > 5.0 && stddev < avg_fps * 0.10 {
            for &gear in self.fps_gears.iter().rev() {
                if gear < self.current_target_fps - 0.5
                    && (avg_fps - gear).abs() < SOFT_LOADING_GEAR_MATCH_TOLERANCE
                {
                    return Some(gear);
                }
            }
        }
        None
    }

    pub fn load_policies(&mut self, config: &Config) {
        self.policies.clear();
        self.mismatch_result_rx = None;
        let _ = crate::utils::try_write_file(
            "/sys/module/perfmgr/parameters/perfmgr_enable", "0");
        let _ = crate::utils::try_write_file(
            "/sys/module/mtk_fpsgo/parameters/perfmgr_enable", "0");
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
                self.perf_ceil = rules.fas_rules.perf_ceil.max(self.perf_floor);
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
                let gov_path = format!(
                    "/sys/devices/system/cpu/cpufreq/policy{}/scaling_governor", policy_id);
                let _ = crate::utils::try_write_file(&gov_path, "performance");
                let avail_path = format!(
                    "/sys/devices/system/cpu/cpufreq/policy{}/scaling_available_frequencies",
                    policy_id);

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

                let mut max_writer = FastWriter::new(format!(
                    "/sys/devices/system/cpu/cpufreq/policy{}/scaling_max_freq", policy_id));
                let mut min_writer = FastWriter::new(format!(
                    "/sys/devices/system/cpu/cpufreq/policy{}/scaling_min_freq", policy_id));
                max_writer.write_value_force(max_f);
                min_writer.write_value_force(max_f);

                self.policies.push(PolicyController {
                    max_writer, min_writer,
                    available_freqs: avail_freqs,
                    current_freq: max_f,
                    policy_id: policy_id as usize,
                    mismatch_count: 0,
                    external_lock_cooldown: 0,
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
        self.scene_transition_continuous = 0;
        self.scene_transition_low_fps_frames = 0;
        self.jank_cooldown = 0;
        self.downgrade_boost_active = false;
        self.downgrade_boost_remaining = 0;
        self.downgrade_boost_perf_saved = 0.0;
        self.consecutive_downgrade_count = 0;
        self.last_downgrade_from_fps = 0.0;
        self.stable_gear_frames = 0;
        self.post_jank_no_decay_frames = 0;
        self.mismatch_probe_skip = 0;
        self.soft_loading_gear_match_frames = 0;
        self.soft_loading_matched_gear = 0.0;

        info!("FAS init | target:{:.0}fps margin:{:.1} clusters:{} perf:{:.0}",
            self.current_target_fps, self.fps_margin, self.policies.len(), self.perf_index);
        info!("FAS config | heavy:{}ms loading:{}ms ignore:{} post_perf:{}-{} \
            instant:{}ms floor:{} hyst:{}",
            self.heavy_frame_threshold_ms, self.loading_cumulative_ms,
            self.post_loading_ignore_frames, self.post_loading_perf_min,
            self.post_loading_perf_max, self.instant_error_threshold_ms,
            self.perf_floor, self.freq_hysteresis);
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

    /// perf_index → 频率映射，含迟滞防抖
    fn apply_freqs(&mut self) {
        self.freq_force_counter = self.freq_force_counter.wrapping_add(1);
        let force_this_cycle = self.freq_force_counter % FREQ_FORCE_REAPPLY_INTERVAL == 0;

        let ratio = self.perf_index / 1000.0;
        for policy in self.policies.iter_mut() {
            if policy.external_lock_cooldown > 0 {
                policy.external_lock_cooldown -= 1;
                if policy.external_lock_cooldown == 0 {
                    log::info!("FAS[P{}] lock cooldown expired, regaining control",
                        policy.policy_id);
                    policy.force_reapply();
                }
                continue;
            }
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
                let cur_idx = policy.available_freqs.iter()
                    .position(|&f| f == policy.current_freq);
                let tgt_idx = policy.available_freqs.iter()
                    .position(|&f| f == target_freq);
                let apply = match (cur_idx, tgt_idx) {
                    (Some(ci), Some(ti)) if (ci as i32 - ti as i32).abs() == 1 => {
                        let cur_r = (policy.current_freq as f32 - pmin) / (pmax - pmin);
                        (ratio - cur_r).abs() > self.freq_hysteresis
                    }
                    _ => true,
                };
                if apply { policy.apply_freq_safe(target_freq); }
            } else if force_this_cycle {
                policy.force_reapply();
            }
        }
    }

    /// 执行软加载内的快速降档
    fn perform_soft_loading_downgrade(&mut self, target_gear: f32) {
        let old_fps = self.current_target_fps;
        self.current_target_fps = target_gear;
        self.is_in_soft_loading = false;
        self.soft_loading_confirm = 0;
        self.soft_loading_exit_confirm = 0;
        self.soft_loading_frames_in_state = 0;
        self.soft_loading_probe_countdown = 0;
        self.soft_loading_gear_match_frames = 0;
        self.soft_loading_matched_gear = 0.0;
        self.fps_window.clear();
        self.ema_actual_ms = 0.0;
        self.downgrade_confirm_frames = 0;
        self.upgrade_confirm_frames = 0;
        self.downgrade_boost_active = false;
        self.downgrade_boost_remaining = 0;
        // 降档后给予适中的perf，不需要太高
        self.perf_index = 500.0;
        self.gear_change_dampen_frames = 60;
        self.upgrade_cooldown = UPGRADE_COOLDOWN_AFTER_DOWNGRADE;
        self.consecutive_downgrade_count = 1;
        self.last_downgrade_from_fps = old_fps;
        self.post_loading_downgrade_guard = 0;
        log::info!("FAS: 💤 soft-load downgrade {:.0}→{:.0}fps | P→{:.0}",
            old_fps, target_gear, self.perf_index);
    }

    pub fn update_frame(&mut self, frame_delta_ns: u64) {
        if frame_delta_ns == 0 || self.policies.is_empty() { return; }

        // 冷启动保护
        if self.init_time.elapsed().as_millis() < 3500 {
            if self.perf_index < 850.0 {
                self.perf_index = 850.0;
                self.apply_freqs();
            }
            return;
        }

        self.frame_time_accumulator_ns =
            self.frame_time_accumulator_ns.wrapping_add(frame_delta_ns);

        let budget_ns = (1_000_000_000.0 / self.current_target_fps.max(1.0)) as u64;
        let min_ns = self.max_gear_min_ns();
        let max_ns = (FIXED_MAX_FRAME_MS * 1_000_000.0) as u64;
        let actual_ms = frame_delta_ns as f32 / 1_000_000.0;
        let budget_ms = budget_ns as f32 / 1_000_000.0;
        let is_heavy = actual_ms > self.heavy_frame_threshold_ms;

        if frame_delta_ns < min_ns { return; }

        // ── 应用切换/息屏检测 ──
        if actual_ms > APP_SWITCH_GAP_MS {
            let was_loading = self.is_in_loading_state;
            let was_soft = self.is_in_soft_loading;

            self.is_in_soft_loading = false;
            self.soft_loading_confirm = 0;
            self.soft_loading_exit_confirm = 0;
            self.soft_loading_frames_in_state = 0;
            self.soft_loading_probe_countdown = 0;
            self.soft_loading_gear_match_frames = 0;
            self.soft_loading_matched_gear = 0.0;
            self.scene_transition_guard = 0;
            self.scene_transition_continuous = 0;
            self.scene_transition_low_fps_frames = 0;
            self.jank_cooldown = 0;
            self.downgrade_boost_active = false;
            self.downgrade_boost_remaining = 0;
            self.post_jank_no_decay_frames = 0;

            if was_loading || was_soft {
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

                log::info!("FAS: 📱 app switch (resume loading) ({:.0}ms) | \
                    Perf→{:.0} | ignore {} | reentry_cd {}",
                    actual_ms, self.perf_index,
                    self.post_loading_ignore, self.loading_reentry_cooldown);
            } else {
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

                log::info!("FAS: 📱 app switch ({:.0}ms) | Perf→{:.0} | ignore {} | guard {}",
                    actual_ms, self.perf_index,
                    self.post_loading_ignore, self.post_loading_downgrade_guard);
            }
            return;
        }

        // ── 重帧 & 硬加载状态机 ──
        if is_heavy {
            if self.loading_reentry_cooldown > 0 {
                self.perf_index = (self.perf_index + 30.0).min(1000.0);
                self.apply_freqs();
                self.loading_reentry_cooldown -= 1;
                log::debug!("FAS: heavy {:.1}ms during reentry cooldown ({}), perf→{:.0}",
                    actual_ms, self.loading_reentry_cooldown, self.perf_index);
                return;
            }

            self.consecutive_loading_frames += 1;
            self.heavy_frame_streak_ms += actual_ms;
            self.normal_frame_tolerance = 0;

            if !self.is_in_loading_state
                && self.heavy_frame_streak_ms > self.loading_cumulative_ms
            {
                self.is_in_loading_state = true;

                let now = self.frame_time_accumulator_ns;
                if self.loading_cycle_count == 0
                    || now.wrapping_sub(self.loading_cycle_first_ns)
                        > SUSTAINED_LOADING_WINDOW_NS
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
                    log::info!("FAS: 🔒 enter sustained loading ({}x cycles)",
                        self.loading_cycle_count);
                }

                if self.is_in_soft_loading {
                    self.is_in_soft_loading = false;
                    self.soft_loading_confirm = 0;
                    self.soft_loading_exit_confirm = 0;
                    self.soft_loading_frames_in_state = 0;
                    self.soft_loading_probe_countdown = 0;
                    self.soft_loading_gear_match_frames = 0;
                    self.soft_loading_matched_gear = 0.0;
                }

                let old = self.perf_index;
                self.perf_index = self.perf_index
                    .clamp(LOADING_PERF_FLOOR, LOADING_PERF_CEILING);
                if old != self.perf_index { self.apply_freqs(); }
                log::info!("FAS: 🔄 enter loading ({} frames, {:.0}ms) | P {:.0}→{:.0}{}",
                    self.consecutive_loading_frames, self.heavy_frame_streak_ms,
                    old, self.perf_index,
                    if self.sustained_loading { " [sustained]" } else { "" });
            }
            log::debug!("FAS: heavy {:.1}ms ({:.1}x) [streak:{}, {:.0}ms]",
                actual_ms, actual_ms / budget_ms,
                self.consecutive_loading_frames, self.heavy_frame_streak_ms);
            return;
        } else {
            if self.consecutive_loading_frames > 0 {
                self.normal_frame_tolerance += 1;
                if self.normal_frame_tolerance < LOADING_NORMAL_TOLERANCE {
                    log::debug!("FAS: loading tolerance {}/{} ({:.1}ms)",
                        self.normal_frame_tolerance, LOADING_NORMAL_TOLERANCE, actual_ms);
                    return;
                } else {
                    log::debug!("FAS: burst end ({} frames, {:.0}ms)",
                        self.consecutive_loading_frames, self.heavy_frame_streak_ms);
                    self.consecutive_loading_frames = 0;
                    self.heavy_frame_streak_ms = 0.0;
                    self.normal_frame_tolerance = 0;
                }
            }

            if self.is_in_loading_state {
                self.is_in_loading_state = false;
                self.fps_window.clear();
                self.downgrade_confirm_frames = 0;
                self.downgrade_boost_active = false;
                self.downgrade_boost_remaining = 0;
                self.ema_actual_ms = 0.0;
                let old = self.perf_index;

                if self.sustained_loading {
                    self.perf_index = self.perf_index
                        .clamp(self.post_loading_perf_min, self.post_loading_perf_max);
                    self.post_loading_ignore = SUSTAINED_POST_LOADING_IGNORE;
                    self.gear_change_dampen_frames = 120;
                    self.post_loading_downgrade_guard = POST_LOADING_DOWNGRADE_GUARD + 60;
                    self.loading_reentry_cooldown = LOADING_REENTRY_COOLDOWN;
                    log::info!("FAS: ✅ exit loading [sustained] | P {:.0}→{:.0} | \
                        ignore {} guard {} reentry_cd {}",
                        old, self.perf_index, self.post_loading_ignore,
                        self.post_loading_downgrade_guard, self.loading_reentry_cooldown);
                } else {
                    self.perf_index = self.perf_index
                        .clamp(self.post_loading_perf_min, self.post_loading_perf_max);
                    self.post_loading_ignore = self.post_loading_ignore_frames;
                    self.gear_change_dampen_frames = 60;
                    self.post_loading_downgrade_guard = POST_LOADING_DOWNGRADE_GUARD;
                    self.loading_reentry_cooldown = LOADING_REENTRY_COOLDOWN;
                    log::info!("FAS: ✅ exit loading | P {:.0}→{:.0} | \
                        ignore {} guard {} reentry_cd {}",
                        old, self.perf_index, self.post_loading_ignore,
                        self.post_loading_downgrade_guard, self.loading_reentry_cooldown);
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
        if self.post_jank_no_decay_frames > 0 { self.post_jank_no_decay_frames -= 1; }

        // ── 场景过渡检测 ──
        if self.scene_transition_guard > 0 && actual_ms > 200.0 {
            log::info!("FAS: ⚡ scene guard cleared by extreme frame {:.0}ms", actual_ms);
            self.scene_transition_guard = 0;
            self.scene_transition_continuous = 0;
            self.scene_transition_low_fps_frames = 0;
            self.jank_cooldown = 0;
        }

        if self.fps_window.count() >= 20 {
            let cv = if avg_fps > 1.0 { self.fps_window.stddev() / avg_fps } else { 0.0 };
            let fps_floor = self.current_target_fps * SCENE_TRANSITION_FPS_FLOOR_RATIO;
            let recent5 = self.fps_window.recent_mean(5);

            if cv > SCENE_TRANSITION_CV_THRESHOLD
                && avg_fps > fps_floor && recent5 > fps_floor
            {
                if self.scene_transition_guard == 0 {
                    self.scene_transition_continuous = 0;
                    self.scene_transition_low_fps_frames = 0;
                    log::info!("FAS: ⚡ scene transition (CV={:.2} avg={:.1}) | guard {}",
                        cv, avg_fps, SCENE_TRANSITION_GUARD_FRAMES);
                }
                self.scene_transition_continuous += 1;

                if self.scene_transition_continuous < SCENE_TRANSITION_MAX_CONTINUOUS {
                    self.scene_transition_guard = SCENE_TRANSITION_GUARD_FRAMES;
                } else if self.scene_transition_guard == 1 {
                    log::info!("FAS: ⚡ scene transition max ({}), force clearing",
                        self.scene_transition_continuous);
                    self.scene_transition_continuous = 0;
                    self.scene_transition_low_fps_frames = 0;
                }
            }

            // 连续低帧逃生
            let recent = self.fps_window.recent_mean(15);
            if self.scene_transition_guard > 0 && recent < fps_floor {
                self.scene_transition_low_fps_frames += 1;
                if self.scene_transition_low_fps_frames >= SCENE_TRANSITION_FORCE_EXIT_FRAMES {
                    log::info!("FAS: ⚡ scene guard force-exit ({:.1} < {:.0} for {}f)",
                        recent, fps_floor, self.scene_transition_low_fps_frames);
                    self.scene_transition_guard = 0;
                    self.scene_transition_continuous = 0;
                    self.scene_transition_low_fps_frames = 0;
                    self.jank_cooldown = 0;
                }
            } else if self.scene_transition_guard > 0 {
                self.scene_transition_low_fps_frames = 0;
            }
        }

        if self.scene_transition_guard == 0 && self.scene_transition_continuous > 0 {
            self.scene_transition_continuous = 0;
            self.scene_transition_low_fps_frames = 0;
        }

        // 持续加载超时恢复
        if self.sustained_loading && !self.is_in_loading_state {
            let now = self.frame_time_accumulator_ns;
            if now.wrapping_sub(self.loading_cycle_first_ns) > SUSTAINED_LOADING_WINDOW_NS * 2 {
                log::info!("FAS: 🔓 sustained loading cleared (stable >{}s)",
                    SUSTAINED_LOADING_WINDOW_NS * 2 / 1_000_000_000);
                self.sustained_loading = false;
                self.loading_cycle_count = 0;
            }
        }

        // ── 软加载检测 ──
        let soft_loading_fps_threshold = self.current_target_fps * SOFT_LOADING_FPS_RATIO;
        let effective_fps = if self.fps_window.count() >= 20 {
            let cv = if avg_fps > 1.0 { self.fps_window.stddev() / avg_fps } else { 0.0 };
            if cv > 0.3 { self.fps_window.recent_mean(20).min(avg_fps) } else { avg_fps }
        } else {
            avg_fps
        };

        if !self.is_in_soft_loading {
            // 进入软加载前先检测是否为原生低帧率：直接降档而非进入软加载
            if effective_fps < soft_loading_fps_threshold
                && self.perf_index >= SOFT_LOADING_PERF_THRESHOLD
                && self.fps_window.count() >= 15
                // 降档boost期间不进入软加载
                && !self.downgrade_boost_active
            {
                // 先检测是否是原生齿轮帧率
                if let Some(native_gear) = self.detect_native_gear(avg_fps) {
                    // FPS稳定在某齿轮附近，不是加载，直接降档
                    log::info!("FAS: 🎯 native gear detected: avg={:.1} → gear {:.0}",
                        avg_fps, native_gear);
                    self.perform_soft_loading_downgrade(native_gear);
                    self.apply_freqs();
                    return;
                }

                self.soft_loading_confirm += 1;
                if self.soft_loading_confirm >= SOFT_LOADING_CONFIRM_FRAMES {
                    self.is_in_soft_loading = true;
                    self.soft_loading_exit_confirm = 0;
                    self.soft_loading_frames_in_state = 0;
                    self.soft_loading_probe_countdown = 0;
                    self.soft_loading_gear_match_frames = 0;
                    self.soft_loading_matched_gear = 0.0;
                    let old = self.perf_index;
                    self.perf_index = SOFT_LOADING_PERF_CAP;
                    self.scene_transition_guard = 0;
                    self.scene_transition_continuous = 0;
                    self.scene_transition_low_fps_frames = 0;
                    self.jank_cooldown = 0;
                    self.downgrade_boost_active = false;
                    self.downgrade_boost_remaining = 0;
                    self.downgrade_confirm_frames = 0;
                    self.apply_freqs();
                    log::info!("FAS: 🌀 enter soft loading | eff:{:.1} avg:{:.1} < {:.0}×{:.0}% \
                        & P:{:.0}>={:.0} | P {:.0}→{:.0}",
                        effective_fps, avg_fps, self.current_target_fps,
                        SOFT_LOADING_FPS_RATIO * 100.0,
                        old, SOFT_LOADING_PERF_THRESHOLD, old, self.perf_index);
                }
            } else {
                self.soft_loading_confirm = 0;
            }
        } else {
            self.soft_loading_frames_in_state += 1;

            // 软加载内齿轮匹配：如果FPS稳定在某齿轮附近，直接降档退出
            if self.fps_window.count() >= 20 {
                if let Some(matched_gear) = self.find_nearest_lower_gear(avg_fps) {
                    let near = (avg_fps - matched_gear).abs() < SOFT_LOADING_GEAR_MATCH_TOLERANCE;
                    let stable = self.fps_window.stddev() < avg_fps * 0.15;
                    if near && stable {
                        if (matched_gear - self.soft_loading_matched_gear).abs() < 1.0 {
                            self.soft_loading_gear_match_frames += 1;
                        } else {
                            self.soft_loading_matched_gear = matched_gear;
                            self.soft_loading_gear_match_frames = 1;
                        }

                        if self.soft_loading_gear_match_frames >= SOFT_LOADING_DOWNGRADE_CHECK_FRAMES {
                            log::info!("FAS: 🎯 soft-load gear match: avg={:.1} → gear {:.0} ({}f)",
                                avg_fps, matched_gear, self.soft_loading_gear_match_frames);
                            self.perform_soft_loading_downgrade(matched_gear);
                            self.apply_freqs();
                            return;
                        }
                    } else {
                        self.soft_loading_gear_match_frames = 0;
                    }
                } else {
                    self.soft_loading_gear_match_frames = 0;
                }
            }

            let in_probe = self.soft_loading_probe_countdown > 0;

            if !in_probe {
                if self.perf_index > SOFT_LOADING_PERF_CAP {
                    self.perf_index = (self.perf_index - SOFT_LOADING_PROBE_FAIL_DECAY_STEP)
                        .max(SOFT_LOADING_PERF_CAP);
                }

                if self.soft_loading_frames_in_state % SOFT_LOADING_PROBE_INTERVAL == 0
                    && self.soft_loading_frames_in_state > 0
                {
                    self.soft_loading_probe_countdown = SOFT_LOADING_PROBE_DURATION;
                    self.soft_loading_probe_avg_before = avg_fps;
                    log::info!("FAS: 🔬 soft probe start | baseline:{:.1} | cap→{:.0}",
                        avg_fps, SOFT_LOADING_PROBE_PERF_CAP);
                }
            } else {
                if self.perf_index > SOFT_LOADING_PROBE_PERF_CAP {
                    self.perf_index = SOFT_LOADING_PROBE_PERF_CAP;
                }
                self.soft_loading_probe_countdown -= 1;

                if self.soft_loading_probe_countdown == 0 {
                    let probe_avg = self.fps_window
                        .recent_mean(SOFT_LOADING_PROBE_DURATION as usize);
                    let probe_max = self.fps_window
                        .recent_max(SOFT_LOADING_PROBE_DURATION as usize);
                    let gain = if self.soft_loading_probe_avg_before > 0.1 {
                        (probe_avg - self.soft_loading_probe_avg_before)
                            / self.soft_loading_probe_avg_before
                    } else { 0.0 };

                    if gain >= SOFT_LOADING_PROBE_FPS_GAIN_RATIO {
                        self.is_in_soft_loading = false;
                        self.soft_loading_confirm = 0;
                        self.soft_loading_exit_confirm = 0;
                        self.soft_loading_frames_in_state = 0;
                        self.soft_loading_gear_match_frames = 0;
                        self.soft_loading_matched_gear = 0.0;
                        self.fps_window.clear();
                        self.downgrade_confirm_frames = 0;
                        self.ema_actual_ms = 0.0;
                        self.post_loading_downgrade_guard = POST_LOADING_DOWNGRADE_GUARD / 2;
                        self.gear_change_dampen_frames = 60;
                        log::info!("FAS: ✅ exit soft loading [probe] | avg:{:.1} max:{:.1} \
                            vs baseline:{:.1} gain:{:+.0}% | guard {}",
                            probe_avg, probe_max, self.soft_loading_probe_avg_before,
                            gain * 100.0, self.post_loading_downgrade_guard);
                    } else {
                        log::info!("FAS: 🔬 soft probe end | avg:{:.1} max:{:.1} \
                            vs baseline:{:.1} gain:{:+.0}% | still loading",
                            probe_avg, probe_max,
                            self.soft_loading_probe_avg_before, gain * 100.0);
                    }
                }
            }

            let is_avg_recovered = avg_fps >= self.current_target_fps * 0.7;
            let recent = self.fps_window.recent_mean(SOFT_LOADING_BREAKTHROUGH_WINDOW);
            let is_breakthrough =
                recent >= self.current_target_fps * SOFT_LOADING_BREAKTHROUGH_FPS_RATIO;

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
                    self.soft_loading_gear_match_frames = 0;
                    self.soft_loading_matched_gear = 0.0;
                    self.fps_window.clear();
                    self.downgrade_confirm_frames = 0;
                    self.ema_actual_ms = 0.0;
                    self.post_loading_downgrade_guard = POST_LOADING_DOWNGRADE_GUARD / 2;
                    self.gear_change_dampen_frames = 60;
                    log::info!("FAS: ✅ exit soft loading | avg:{:.1} recent:{:.1} {} | guard {}",
                        avg_fps, recent,
                        if is_breakthrough { "[breakthrough]" } else { "[avg-recovered]" },
                        self.post_loading_downgrade_guard);
                }
            } else {
                self.soft_loading_exit_confirm = 0;
            }

            // 软加载期间禁止降档逻辑（但FIX-A的齿轮匹配降档在上面已处理）
            self.downgrade_confirm_frames = 0;
            self.downgrade_boost_active = false;
            self.downgrade_boost_remaining = 0;
        }

        // ── 升档 ──
        if !self.is_in_soft_loading {
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
                        self.consecutive_downgrade_count = 0;
                        self.stable_gear_frames = 0;
                        self.downgrade_boost_active = false;
                        self.downgrade_boost_remaining = 0;
                    }
                } else {
                    let probe_avg_threshold = if self.consecutive_downgrade_count >= 2 {
                        target - 10.0
                    } else {
                        self.current_target_fps - 5.0
                    };
                    if avg_fps >= probe_avg_threshold
                        && self.perf_index < 500.0
                        && self.upgrade_cooldown == 0
                    {
                        self.upgrade_confirm_frames += 1;
                        if self.upgrade_confirm_frames >= 90 {
                            log::info!("FAS: 🔍 probe {:.0}→{:.0}fps (avg={:.1} P={:.0} \
                                consec:{})",
                                self.current_target_fps, target, avg_fps,
                                self.perf_index, self.consecutive_downgrade_count);
                            self.current_target_fps = target;
                            self.upgrade_confirm_frames = 0;
                            self.ema_actual_ms = 0.0;
                            self.fps_window.clear();
                            self.perf_index = (self.perf_index + 200.0).min(600.0);
                            self.gear_change_dampen_frames = 90;
                            self.downgrade_boost_active = false;
                            self.downgrade_boost_remaining = 0;
                            self.stable_gear_frames = 0;
                        }
                    } else {
                        self.upgrade_confirm_frames =
                            self.upgrade_confirm_frames.saturating_sub(5);
                    }
                }
            } else {
                self.upgrade_confirm_frames = 0;
            }

            // ── 降档 ──
            let recent30_for_dg = self.fps_window.recent_mean(30);
            if let Some(target) = prev_gear {
                if self.post_loading_downgrade_guard > 0 {
                    self.downgrade_confirm_frames = 0;
                    self.downgrade_boost_active = false;
                    self.downgrade_boost_remaining = 0;
                    log::debug!("FAS: downgrade blocked (guard: {})",
                        self.post_loading_downgrade_guard);
                } else if self.scene_transition_guard > 0 {
                    self.downgrade_confirm_frames = 0;
                    self.downgrade_boost_active = false;
                    self.downgrade_boost_remaining = 0;
                    log::debug!("FAS: downgrade blocked (scene: {})",
                        self.scene_transition_guard);
                } else if avg_fps < self.current_target_fps - 10.0 {
                    if recent30_for_dg >= self.current_target_fps - 5.0 {
                        if self.downgrade_boost_active {
                            log::info!("FAS: ⬆ boost cancelled (recent30:{:.1} healthy) | P:{:.0}",
                                recent30_for_dg, self.perf_index);
                            self.downgrade_boost_active = false;
                            self.downgrade_boost_remaining = 0;
                        }
                        self.downgrade_confirm_frames = 0;
                    } else if !self.downgrade_boost_active
                        && self.downgrade_confirm_frames == 0
                    {
                        self.downgrade_boost_active = true;
                        self.downgrade_boost_remaining = DOWNGRADE_BOOST_DURATION;
                        self.downgrade_boost_perf_saved = self.perf_index;
                        self.perf_index =
                            (self.perf_index + DOWNGRADE_BOOST_PERF_INC).min(900.0);
                        log::info!("FAS: ⬆ boost start | avg:{:.1} r30:{:.1} | P:{:.0}→{:.0}",
                            avg_fps, recent30_for_dg,
                            self.downgrade_boost_perf_saved, self.perf_index);
                    } else if self.downgrade_boost_active
                        && self.downgrade_boost_remaining > 0
                    {
                        self.downgrade_boost_remaining -= 1;
                        if self.downgrade_boost_remaining == 0 {
                            // boost失败：不恢复旧P，而是给予中等值
                            // 因为更高的P也没用，恢复高P只会浪费功耗
                            let restored_perf = self.downgrade_boost_perf_saved
                                .min(self.perf_index);
                            self.perf_index = restored_perf;
                            self.downgrade_boost_active = false;
                            self.downgrade_confirm_frames = 1;
                            log::info!("FAS: ⬆ boost failed | avg:{:.1} r30:{:.1} | \
                                P restored→{:.0}",
                                avg_fps, recent30_for_dg, self.perf_index);
                        }
                    } else {
                        self.downgrade_confirm_frames += 1;
                        if self.downgrade_confirm_frames >= DOWNGRADE_CONFIRM_FRAMES {
                            let old_fps = self.current_target_fps;
                            self.current_target_fps = target;
                            self.downgrade_confirm_frames = 0;
                            self.downgrade_boost_active = false;
                            self.downgrade_boost_remaining = 0;
                            self.ema_actual_ms = 0.0;
                            self.fps_window.clear();
                            self.upgrade_confirm_frames = 0;
                            self.gear_change_dampen_frames = 60;

                            if (old_fps - self.last_downgrade_from_fps).abs() < 1.0 {
                                self.consecutive_downgrade_count += 1;
                            } else {
                                self.consecutive_downgrade_count = 1;
                            }
                            self.last_downgrade_from_fps = old_fps;
                            let backoff =
                                1u32 << self.consecutive_downgrade_count.min(4);
                            self.upgrade_cooldown =
                                UPGRADE_COOLDOWN_AFTER_DOWNGRADE * backoff;
                            log::info!("FAS: 💤 {:.0}→{:.0}fps (avg={:.1}) P={:.0} \
                                cd={} [consec:{}]",
                                old_fps, target, avg_fps, self.perf_index,
                                self.upgrade_cooldown, self.consecutive_downgrade_count);
                            self.stable_gear_frames = 0;
                        }
                    }
                } else {
                    if self.downgrade_boost_active {
                        log::info!("FAS: ⬆ boost succeeded | avg:{:.1} recovered | P:{:.0}",
                            avg_fps, self.perf_index);
                        self.downgrade_boost_active = false;
                        self.downgrade_boost_remaining = 0;
                    }
                    self.downgrade_confirm_frames = 0;
                }
            }

            // ── 稳定运行宽恕 ──
            if self.consecutive_downgrade_count > 0 {
                if avg_fps >= self.current_target_fps - 3.0
                    && self.fps_window.count() >= 60
                {
                    self.stable_gear_frames += 1;
                } else {
                    self.stable_gear_frames = self.stable_gear_frames.saturating_sub(3);
                }
                if self.stable_gear_frames >= STABLE_FORGIVE_FRAMES {
                    log::info!("FAS: 🕊 stable forgive | {:.0}fps for {}f, consec:{} → 0",
                        self.current_target_fps, self.stable_gear_frames,
                        self.consecutive_downgrade_count);
                    self.consecutive_downgrade_count = 0;
                    self.stable_gear_frames = 0;
                }
            }
        }

        // ── 双预算 & EMA ──
        let ema_budget = 1000.0 / (self.current_target_fps - self.fps_margin).max(1.0);
        let inst_budget = 1000.0 / self.current_target_fps;

        // EMA：缩小上下不对称差距 + jank后加速恢复
        if self.ema_actual_ms <= 0.0 {
            self.ema_actual_ms = actual_ms;
        } else {
            // jank冷却结束后用更快的下降系数，避免幻影deficit
            let a_down = if self.jank_cooldown == 0
                && self.post_jank_no_decay_frames > 0
            {
                0.30  // jank刚过，加速EMA恢复
            } else {
                0.20
            };
            let a = if actual_ms > self.ema_actual_ms { 0.25 } else { a_down };
            self.ema_actual_ms = self.ema_actual_ms * (1.0 - a) + actual_ms * a;
        }

        let ema_err = ema_budget - self.ema_actual_ms;
        let inst_err = inst_budget - actual_ms;
        let act;

        // ── 蹦床 v9 ──
        let old_perf = self.perf_index;
        let damped = self.gear_change_dampen_frames > 0;
        let in_scene_transition = self.scene_transition_guard > 0;

        let high_perf_scale = if self.perf_index > 750.0 {
            ((1000.0 - self.perf_index) / 250.0).clamp(0.2, 1.0)
        } else {
            1.0
        };

        let heavy_threshold = (ema_budget * 0.15).clamp(2.0, 4.0);
        let bounce_threshold = (ema_budget * 0.15).clamp(1.0, 2.0);

        if inst_err < -self.instant_error_threshold_ms {
            let inc = (if damped { 30.0 } else { 55.0 }) * high_perf_scale;
            self.perf_index += inc;
            act = if damped { "crit-d" } else { "crit" };
            self.consecutive_normal_frames = 0;
            self.jank_cooldown = JANK_COOLDOWN_FRAMES_CRIT;
            self.post_jank_no_decay_frames = FAST_DECAY_POST_JANK_SUPPRESS;
        } else if ema_err < -heavy_threshold {
            let inc = (if damped { 12.0 } else { 25.0 }) * high_perf_scale;
            self.perf_index += inc;
            act = if damped { "heavy-d" } else { "heavy" };
            self.consecutive_normal_frames = 0;
            self.jank_cooldown = self.jank_cooldown.max(JANK_COOLDOWN_FRAMES_HEAVY);
            self.post_jank_no_decay_frames = FAST_DECAY_POST_JANK_SUPPRESS / 2;
        } else if ema_err < -bounce_threshold {
            let inc = (if damped { 3.0 } else { 5.0 }) * high_perf_scale;
            self.perf_index += inc;
            act = if damped { "bounce-d" } else { "bounce" };
            self.consecutive_normal_frames = 0;
        } else {
            self.consecutive_normal_frames += 1;

            let in_jank_cooldown = self.jank_cooldown > 0;
            let low_perf_factor = if self.perf_index < 400.0 {
                (self.perf_index / 400.0).max(0.3)
            } else {
                1.0
            };

            let proximity_threshold = self.current_target_fps * DOWNGRADE_PROXIMITY_RATIO;
            let near_downgrade = avg_fps < proximity_threshold && avg_fps > 1.0;
            let proximity_factor = if near_downgrade { 0.2 } else { 1.0 };

            let in_downgrade_boost =
                self.downgrade_boost_active && self.downgrade_boost_remaining > 0;
            let boost_factor = if in_downgrade_boost { 0.0 } else { 1.0 };

            let decay_scale = low_perf_factor * proximity_factor * boost_factor;

            if in_jank_cooldown {
                let base = if in_scene_transition { 2.0 } else { 3.0 };
                let d = base * decay_scale;
                self.perf_index -= d;
                act = "fine-jc";
            } else if ema_err < -DEFICIT_SUPPRESS_MS {
                // 显著deficit：需要瞬时也超预算才加P，防止EMA滞后导致的幻影deficit
                if inst_err < 0.0 {
                    let inc = (-ema_err * 1.2).clamp(0.3, 1.5);
                    self.perf_index += inc;
                }
                act = if in_scene_transition { "deficit-s" } else { "deficit" };
            } else if ema_err < 0.0 {
                // 轻微deficit（-0.3~0）：不加P，但也仅微衰减，保持稳定
                let base = if in_scene_transition { 0.5 } else { 1.0 };
                let d = base * decay_scale;
                self.perf_index -= d;
                act = "near";
            } else if ema_err < 1.0 {
                let base = if in_scene_transition { 1.5 } else { 2.5 };
                let d = base * decay_scale;
                self.perf_index -= d;
                act = if in_scene_transition { "fine-s" } else { "fine" };
            } else if ema_err < 3.0 {
                let base = if in_scene_transition { 2.5 } else { 4.0 };
                let d = base * decay_scale;
                self.perf_index -= d;
                act = if in_scene_transition { "surplus-s" } else { "surplus" };
            } else {
                let base = if in_scene_transition { 4.0 } else { 7.0 };
                let d = base * decay_scale;
                self.perf_index -= d;
                act = if in_scene_transition { "excess-s" } else { "excess" };
            }

            // fast_decay
            let init_elapsed_ms = self.init_time.elapsed().as_millis();
            if self.consecutive_normal_frames >= FAST_DECAY_FRAME_THRESHOLD
                && self.perf_index > FAST_DECAY_PERF_THRESHOLD
                && !in_scene_transition && !in_jank_cooldown
                && !near_downgrade && !in_downgrade_boost
                && self.post_jank_no_decay_frames == 0
                && init_elapsed_ms > 8000
            {
                let step = ((self.perf_index - 500.0) / 500.0 * FAST_DECAY_MAX_STEP)
                    .clamp(FAST_DECAY_MIN_STEP, FAST_DECAY_MAX_STEP);
                self.perf_index -= step;
                log::debug!("FAS: fast_decay -{:.0} after {}f (P:{:.0}→{:.0})",
                    step, self.consecutive_normal_frames,
                    self.perf_index + step, self.perf_index);
                self.consecutive_normal_frames = 0;
            }
        }

        // clamp & 限速
        self.perf_index = self.perf_index.clamp(self.perf_floor, self.perf_ceil);
        let max_inc = if damped { 40.0 } else { 70.0 };
        if self.perf_index > old_perf + max_inc {
            self.perf_index = old_perf + max_inc;
        }
        if damped && self.perf_index > 900.0 {
            self.perf_index = 900.0;
        }

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

        // ── 心跳（每30帧） ──
        self.log_counter = self.log_counter.wrapping_add(1);
        if self.log_counter % 30 == 0 {
            log::info!("FAS | {:.0}fps avg:{:.1} | {:.2}ms ema:{:.2} | \
                err:{:+.2}/{:+.2} thr:h{:.1}/b{:.1} | {} | P:{:.0}{}{}{}{}{}{}{}",
                self.current_target_fps, avg_fps, actual_ms, self.ema_actual_ms,
                ema_err, inst_err, heavy_threshold, bounce_threshold,
                act, self.perf_index,
                if self.upgrade_cooldown > 0 {
                    format!(" cd:{}", self.upgrade_cooldown)
                } else { String::new() },
                if damped {
                    format!(" damp:{}", self.gear_change_dampen_frames)
                } else { String::new() },
                if self.post_loading_downgrade_guard > 0 {
                    format!(" guard:{}", self.post_loading_downgrade_guard)
                } else { String::new() },
                if self.is_in_soft_loading {
                    " [soft-load]".to_string()
                } else { String::new() },
                if self.scene_transition_guard > 0 {
                    format!(" [scene:{}]", self.scene_transition_guard)
                } else { String::new() },
                if self.jank_cooldown > 0 {
                    format!(" [jank-cd:{}]", self.jank_cooldown)
                } else { String::new() },
                if self.downgrade_boost_active {
                    format!(" [dg-boost:{}]", self.downgrade_boost_remaining)
                } else { String::new() });

            // ── 频率 mismatch 检测 ──
            if self.mismatch_probe_skip > 0 {
                self.mismatch_probe_skip -= 1;
            } else {
                let mut needs_reapply = false;
                if let Some(rx) = &self.mismatch_result_rx {
                    if let Ok(readings) = rx.try_recv() {
                        for (policy_id, actual_freq) in readings {
                            if let Some(p) = self.policies.iter_mut()
                                .find(|p| p.policy_id == policy_id
                                    && p.external_lock_cooldown == 0)
                            {
                                let diff = (actual_freq as i64
                                    - p.current_freq as i64).unsigned_abs();
                                let threshold = (p.current_freq as u64) * 15 / 100;
                                if diff > threshold {
                                    p.mismatch_count += 1;
                                    if p.mismatch_count >= MISMATCH_LOCK_THRESHOLD {
                                        p.external_lock_cooldown = 300;
                                        p.mismatch_count = 0;
                                        log::warn!("FAS[P{}] externally locked: \
                                            yielding 300f (actual={} MHz)",
                                            p.policy_id, actual_freq / 1000);
                                    } else {
                                        log::warn!("FAS[P{}] mismatch: set={} actual={} \
                                            MHz ({}%) [{}/{}]",
                                            p.policy_id, p.current_freq / 1000,
                                            actual_freq / 1000,
                                            diff * 100 / p.current_freq as u64,
                                            p.mismatch_count, MISMATCH_LOCK_THRESHOLD);
                                        needs_reapply = true;
                                    }
                                } else {
                                    p.mismatch_count =
                                        p.mismatch_count.saturating_sub(1);
                                }
                            }
                        }
                    }
                }

                if needs_reapply {
                    log::info!("FAS: 🔧 mismatch, force reapply unlocked policies");
                    for p in self.policies.iter_mut() {
                        if p.external_lock_cooldown == 0 {
                            p.force_reapply();
                        }
                    }
                    self.mismatch_probe_skip = MISMATCH_REAPPLY_SKIP_CYCLES;
                    self.mismatch_result_rx = None;
                    self.apply_freqs();
                    return;
                }

                // 发起新一轮异步 sysfs 读取
                let probe_targets: Vec<(usize, bool)> = self.policies.iter()
                    .map(|p| (p.policy_id, p.external_lock_cooldown > 0))
                    .collect();

                if !probe_targets.is_empty() {
                    let (tx, rx) = mpsc::channel();
                    self.mismatch_result_rx = Some(rx);
                    std::thread::spawn(move || {
                        let mut results = Vec::with_capacity(probe_targets.len());
                        for (policy_id, locked) in probe_targets {
                            if locked { continue; }
                            let path = format!(
                                "/sys/devices/system/cpu/cpufreq/policy{}/scaling_cur_freq",
                                policy_id);
                            if let Ok(s) = fs::read_to_string(&path) {
                                if let Ok(freq) = s.trim().parse::<u32>() {
                                    results.push((policy_id, freq));
                                }
                            }
                        }
                        let _ = tx.send(results);
                    });
                }
            }
        }

        self.apply_freqs();
    }
}