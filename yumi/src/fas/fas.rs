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
use std::os::unix::fs::PermissionsExt;

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
        if path_ref.exists() {
            let _ = fs::set_permissions(path_ref, fs::Permissions::from_mode(0o444));
        }
        Self { file, last_value: None }
    }

    pub fn write_value(&mut self, value: u32) {
        if self.last_value == Some(value) { return; }
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

pub struct PolicyController {
    pub max_writer: FastWriter,
    pub min_writer: FastWriter,
    pub available_freqs: Vec<u32>,
    pub current_freq: u32,
    pub policy_id: usize,
}

impl PolicyController {
    pub fn apply_freq_safe(&mut self, target_freq: u32) {
        if target_freq == self.current_freq { return; }

        let range = (*self.available_freqs.last().unwrap() - *self.available_freqs.first().unwrap()) as f32;
        let percentage = if range > 0.0 {
            ((target_freq - *self.available_freqs.first().unwrap()) as f32 / range * 100.0) as u32
        } else { 0 };

        log::debug!("FAS[P{}] {} {} → {} Hz ({:.1} MHz, {}%)",
            self.policy_id,
            if target_freq > self.current_freq { "↑" } else { "↓" },
            self.current_freq, target_freq,
            target_freq as f32 / 1000.0, percentage);

        if target_freq > self.current_freq {
            self.max_writer.write_value(target_freq);
            self.min_writer.write_value(target_freq);
        } else {
            self.min_writer.write_value(target_freq);
            self.max_writer.write_value(target_freq);
        }
        self.current_freq = target_freq;
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
    fn count(&self) -> usize { if self.filled { 120 } else { self.pos } }
    fn clear(&mut self) { self.buf = [0.0; 120]; self.pos = 0; self.filled = false; }
}

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
    // 可配置
    heavy_frame_threshold_ms: f32,
    loading_cumulative_ms: f32,
    post_loading_ignore_frames: u32,
    post_loading_perf_min: f32,
    post_loading_perf_max: f32,
    instant_error_threshold_ms: f32,
    perf_floor: f32,
    freq_hysteresis: f32,
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

        for (idx, &policy_id) in clusters.iter().enumerate() {
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
                max_writer.write_value(max_f);
                min_writer.write_value(max_f);

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
    }

    /// 将 perf_index 映射到频率并应用，含迟滞防抖
    fn apply_freqs(&mut self) {
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
            }
        }
    }

    pub fn update_frame(&mut self, frame_delta_ns: u64) {
        if frame_delta_ns == 0 || self.policies.is_empty() { return; }

        let budget_ns = (1_000_000_000.0 / self.current_target_fps.max(1.0)) as u64;
        let min_ns = budget_ns * 60 / 100;
        let max_ns = budget_ns * 20;
        let actual_ms = frame_delta_ns as f32 / 1_000_000.0;
        let budget_ms = budget_ns as f32 / 1_000_000.0;
        let is_heavy = actual_ms > self.heavy_frame_threshold_ms;

        if frame_delta_ns < min_ns { return; }

        // ── 重帧 & 加载状态机 ──
        if is_heavy {
            self.consecutive_loading_frames += 1;
            self.heavy_frame_streak_ms += actual_ms;

            if !self.is_in_loading_state && self.heavy_frame_streak_ms > self.loading_cumulative_ms {
                self.is_in_loading_state = true;
                let old = self.perf_index;
                self.perf_index = 400.0;
                self.apply_freqs();
                log::info!("FAS: 🔄 enter loading ({} frames, {:.0}ms) | Perf {:.0}→{:.0}",
                    self.consecutive_loading_frames, self.heavy_frame_streak_ms, old, self.perf_index);
            }
            log::debug!("FAS: heavy {:.1}ms ({:.1}x) [streak:{}, {:.0}ms]",
                actual_ms, actual_ms / budget_ms, self.consecutive_loading_frames, self.heavy_frame_streak_ms);
            return;
        } else {
            if self.consecutive_loading_frames > 0 {
                log::debug!("FAS: burst end ({} frames, {:.0}ms)",
                    self.consecutive_loading_frames, self.heavy_frame_streak_ms);
                self.consecutive_loading_frames = 0;
                self.heavy_frame_streak_ms = 0.0;
            }
            if self.is_in_loading_state {
                self.is_in_loading_state = false;
                self.ema_actual_ms = 0.0;
                let old = self.perf_index;
                self.perf_index = self.perf_index.clamp(self.post_loading_perf_min, self.post_loading_perf_max);
                self.post_loading_ignore = self.post_loading_ignore_frames;
                self.gear_change_dampen_frames = 60;
                log::info!("FAS: ✅ exit loading | Perf {:.0}→{:.0} | ignore {} frames",
                    old, self.perf_index, self.post_loading_ignore);
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
        let max_fps = self.fps_window.max_fps();
        let avg_fps = self.fps_window.mean();

        let next_gear = self.fps_gears.iter().copied()
            .filter(|&g| g > self.current_target_fps + 0.5).reduce(f32::min);
        let prev_gear = self.fps_gears.iter().copied()
            .filter(|&g| g < self.current_target_fps - 0.5).reduce(f32::max);

        if self.upgrade_cooldown > 0 { self.upgrade_cooldown -= 1; }
        if self.gear_change_dampen_frames > 0 { self.gear_change_dampen_frames -= 1; }

        // ── 档位管理 ──
        if let Some(target) = next_gear {
            if self.upgrade_cooldown > 0 {
                self.upgrade_confirm_frames = 0;
            } else if max_fps >= target - 5.0
                && avg_fps >= self.current_target_fps * 0.9
                && self.fps_window.count() >= 60
            {
                self.upgrade_confirm_frames += 1;
                self.downgrade_confirm_frames = 0;
                if self.upgrade_confirm_frames >= 60 {
                    log::info!("FAS: 🚀 {:.0}→{:.0}fps (max={:.1} avg={:.1})",
                        self.current_target_fps, target, max_fps, avg_fps);
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

        if let Some(target) = prev_gear {
            if avg_fps < self.current_target_fps - 10.0 {
                self.downgrade_confirm_frames += 1;
                if self.downgrade_confirm_frames >= 30 {
                    let old_fps = self.current_target_fps;
                    self.current_target_fps = target;
                    self.downgrade_confirm_frames = 0;
                    self.ema_actual_ms = 0.0;
                    self.perf_index = (self.perf_index - 200.0).max(400.0);
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
            let a = if actual_ms < self.ema_actual_ms { 0.5 } else { 0.15 };
            self.ema_actual_ms = self.ema_actual_ms * (1.0 - a) + actual_ms * a;
        }

        let ema_err = ema_budget - self.ema_actual_ms;
        let inst_err = inst_budget - actual_ms;
        let act;

        // ── 蹦床 v6 ──
        let old_perf = self.perf_index;
        let damped = self.gear_change_dampen_frames > 0;

        if inst_err < -self.instant_error_threshold_ms {
            self.perf_index += if damped { 40.0 } else { 80.0 };
            act = if damped { "crit-d(+40)" } else { "crit(+80)" };
            self.consecutive_normal_frames = 0;
        } else if ema_err < -2.0 {
            self.perf_index += if damped { 15.0 } else { 40.0 };
            act = if damped { "heavy-d(+15)" } else { "heavy(+40)" };
            self.consecutive_normal_frames = 0;
        } else if ema_err < -0.5 {
            self.perf_index += if damped { 3.0 } else { 5.0 };
            act = if damped { "bounce-d(+3)" } else { "bounce(+5)" };
            self.consecutive_normal_frames = 0;
        } else {
            self.consecutive_normal_frames += 1;
            if ema_err < 1.0 {
                self.perf_index -= 3.0; act = "fine(-3)";
            } else if ema_err < 3.0 {
                self.perf_index -= 8.0; act = "surplus(-8)";
            } else {
                self.perf_index -= 15.0; act = "excess(-15)";
            }
            if self.consecutive_normal_frames >= 30 && self.perf_index > 600.0 {
                self.perf_index -= 80.0;
                log::debug!("FAS: fast decay after {} frames", self.consecutive_normal_frames);
                self.consecutive_normal_frames = 0;
            }
        }

        self.perf_index = self.perf_index.clamp(self.perf_floor, 1000.0);
        let max_inc = if damped { 50.0 } else { 100.0 };
        if self.perf_index > old_perf + max_inc { self.perf_index = old_perf + max_inc; }
        if damped && self.perf_index > 900.0 { self.perf_index = 900.0; }

        // ── 心跳 (每30帧) ──
        self.log_counter = self.log_counter.wrapping_add(1);
        if self.log_counter % 30 == 0 {
            log::info!("FAS | {:.0}fps avg:{:.1} | {:.2}ms ema:{:.2} | err:{:+.2}/{:+.2} | {} | P:{:.0}{}{}",
                self.current_target_fps, avg_fps, actual_ms, self.ema_actual_ms,
                ema_err, inst_err, act, self.perf_index,
                if self.upgrade_cooldown > 0 { format!(" cd:{}", self.upgrade_cooldown) } else { String::new() },
                if damped { format!(" damp:{}", self.gear_change_dampen_frames) } else { String::new() });

            for p in &self.policies {
                if let Ok(s) = fs::read_to_string(
                    format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_cur_freq", p.policy_id)) {
                    if let Ok(f) = s.trim().parse::<u32>() {
                        if f != p.current_freq {
                            log::warn!("FAS[P{}] freq mismatch: set={} actual={} MHz",
                                p.policy_id, p.current_freq / 1000, f / 1000);
                        }
                    }
                }
            }
        }

        self.apply_freqs();
    }
}