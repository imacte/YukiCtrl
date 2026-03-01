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
use crate::monitor::config::{FasRulesConfig, ClusterProfile, AdaptivePidConfig};
use std::fs::{self, File, OpenOptions};
use std::io::{Write, Seek, SeekFrom};
use std::path::Path;
use std::sync::mpsc;
use log::info;
use std::time::Instant;

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

pub struct PolicyController {
    pub max_writer: FastWriter,
    pub min_writer: FastWriter,
    pub available_freqs: Vec<u32>,
    pub current_freq: u32,
    pub policy_id: usize,
    pub mismatch_count: u32,
    pub external_lock_cooldown: u32,
    pub cluster_profile: ClusterProfile,
    pub freq_hold_frames: u32,
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
        self.freq_hold_frames = 2;
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

fn probe_policy_capacity(policy_id: i32) -> Option<u32> {
    let related_path = format!(
        "/sys/devices/system/cpu/cpufreq/policy{}/related_cpus", policy_id);
    let related_str = fs::read_to_string(&related_path)
        .or_else(|_| {
            let affected_path = format!(
                "/sys/devices/system/cpu/cpufreq/policy{}/affected_cpus", policy_id);
            fs::read_to_string(&affected_path)
        })
        .ok()?;

    let first_cpu: u32 = related_str.split_whitespace()
        .next()?
        .parse().ok()?;

    let cap_path = format!(
        "/sys/devices/system/cpu/cpu{}/cpu_capacity", first_cpu);
    let cap_str = fs::read_to_string(&cap_path).ok()?;
    cap_str.trim().parse::<u32>().ok()
}

fn auto_compute_capacity_weights(policy_ids: &[i32]) -> Option<Vec<(i32, f32)>> {
    let mut caps: Vec<(i32, u32)> = Vec::with_capacity(policy_ids.len());

    for &pid in policy_ids {
        if pid == -1 { continue; }
        let cap = probe_policy_capacity(pid)?;
        if cap == 0 { return None; } // 异常值
        caps.push((pid, cap));
    }

    if caps.is_empty() { return None; }

    // 找到最小算力值作为基准
    let min_cap = caps.iter().map(|&(_, c)| c).min().unwrap() as f32;

    let weights: Vec<(i32, f32)> = caps.iter().map(|&(pid, cap)| {
        let raw_ratio = cap as f32 / min_cap;
        let weight = if raw_ratio <= 1.01 {
            1.0
        } else {
            1.0 + (raw_ratio - 1.0).sqrt()
        };
        (pid, weight)
    }).collect();

    Some(weights)
}

struct AdaptiveGainScheduler {
    cfg: AdaptivePidConfig,

    // 基线系数（从 YAML 加载的初始值）
    base_kp: f32,
    base_ki: f32,
    base_kd: f32,

    // 增益乘子（运行时动态调整，乘到基线上）
    kp_mult: f32,
    ki_mult: f32,
    kd_mult: f32,

    // ── 帧率归一化 ──
    /// 当前 fps 归一化系数（60fps 基准）
    fps_norm: f32,

    // ── 观测状态 ──
    /// 连续 deficit 帧计数（inst_err < 0 时递增，否则归零）
    deficit_streak: u32,

    /// perf_index 变化方向历史（true=升，false=降），用于检测振荡
    direction_history: Vec<bool>,
    /// 上一帧的 perf_index
    prev_perf: f32,

    /// 评估帧计数器
    eval_counter: u32,

    /// 连续振荡评估次数（超过阈值后暂停响应）
    consecutive_osc_evals: u32,
}

impl AdaptiveGainScheduler {
    fn new(cfg: &AdaptivePidConfig, base_kp: f32, base_ki: f32, base_kd: f32) -> Self {
        Self {
            cfg: cfg.clone(),
            base_kp, base_ki, base_kd,
            kp_mult: 1.0,
            ki_mult: 1.0,
            kd_mult: 1.0,
            fps_norm: 1.0,
            deficit_streak: 0,
            direction_history: Vec::with_capacity(256),
            prev_perf: 0.5,
            eval_counter: 0,
            consecutive_osc_evals: 0,
        }
    }

    /// 每帧调用：喂入观测数据
    fn observe(&mut self, inst_err: f32, current_perf: f32) {
        if !self.cfg.enabled { return; }

        // 更新 deficit streak
        if inst_err < 0.0 {
            self.deficit_streak += 1;
        } else {
            self.deficit_streak = 0;
        }

        // 方向变化死区大幅增大
        // 旧值 0.001/fps_norm ≈ 0.0012 太小，PID 每帧变化 0.003~0.01 都会被记录
        // 新值 0.008：只有真正的方向性变化（非正常跟踪抖动）才计入
        let direction_deadzone = 0.008;
        let going_up = current_perf > self.prev_perf + direction_deadzone;
        let going_down = current_perf < self.prev_perf - direction_deadzone;
        if going_up || going_down {
            self.direction_history.push(going_up);
            // 振荡窗口按帧率缩放，确保覆盖固定时间段（~1秒）
            // 60fps → 60帧, 144fps → 144帧 (而非固定60帧=0.42秒)
            let scaled_window = ((self.cfg.oscillation_window as f32 / self.fps_norm.max(0.3))
                as usize).max(self.cfg.oscillation_window as usize);
            if self.direction_history.len() > scaled_window {
                let excess = self.direction_history.len() - scaled_window;
                self.direction_history.drain(0..excess);
            }
        }
        self.prev_perf = current_perf;

        // 评估间隔也按帧率缩放
        let scaled_eval_interval = ((self.cfg.eval_interval as f32 / self.fps_norm.max(0.3))
            as u32).max(self.cfg.eval_interval);
        self.eval_counter += 1;
        if self.eval_counter >= scaled_eval_interval {
            self.eval_counter = 0;
            self.evaluate();
        }
    }

    /// 检测方向翻转次数（升↔降 的次数）
    fn count_reversals(&self) -> u32 {
        if self.direction_history.len() < 2 { return 0; }
        let mut reversals = 0u32;
        for i in 1..self.direction_history.len() {
            if self.direction_history[i] != self.direction_history[i - 1] {
                reversals += 1;
            }
        }
        reversals
    }

    /// 周期性评估，调整增益乘子
    fn evaluate(&mut self) {
        let reversals = self.count_reversals();
        // 振荡阈值按帧率缩放（窗口变大了，阈值也要等比例提高）
        let scaled_osc_threshold = ((self.cfg.oscillation_threshold as f32
            / self.fps_norm.max(0.3)) as u32).max(self.cfg.oscillation_threshold);
        let is_oscillating = reversals >= scaled_osc_threshold;
        // deficit streak 阈值也按帧率缩放
        let scaled_deficit_threshold = ((self.cfg.deficit_streak_threshold as f32
            / self.fps_norm.max(0.3)) as u32).max(self.cfg.deficit_streak_threshold);
        let is_deficit = self.deficit_streak >= scaled_deficit_threshold;

        // ── 规则 ①：deficit streak → boost Kp ──
        if is_deficit {
            self.kp_mult += self.cfg.kp_boost_step;
            // deficit 时 Ki 也轻微提升（加速积分修正）
            self.ki_mult += self.cfg.kp_boost_step * 0.5;
            log::info!("FAS adaptive: deficit streak {} → Kp×{:.2} Ki×{:.2}",
                self.deficit_streak, self.kp_mult, self.ki_mult);
        }

        // ── 规则 ②：oscillation → boost Kd, reduce Kp ──
        // 连续检测到振荡超过 3 次，说明是结构性振荡（齿轮乒乓等），
        // PID 调参无法修复，暂停响应以避免 Kp 被削到极低导致系统迟钝
        if is_oscillating {
            self.consecutive_osc_evals += 1;
            if self.consecutive_osc_evals <= 3 {
                self.kd_mult += self.cfg.kd_boost_step;
                self.kp_mult -= self.cfg.kp_osc_reduce_step;
                log::info!("FAS adaptive: oscillation ({} reversals) → Kp×{:.2} Kd×{:.2}",
                    reversals, self.kp_mult, self.kd_mult);
            } else {
                log::debug!("FAS adaptive: oscillation ({} reversals) suppressed (consecutive={})",
                    reversals, self.consecutive_osc_evals);
            }
        } else {
            self.consecutive_osc_evals = 0;
        }

        // ── 规则 ③：稳定 → 缓慢回落到 1.0 ──
        if !is_deficit && !is_oscillating {
            // Kp 向 1.0 回落
            if self.kp_mult > 1.0 {
                self.kp_mult -= self.cfg.kp_decay_step;
            } else if self.kp_mult < 1.0 {
                self.kp_mult += self.cfg.kp_decay_step;
            }
            // Ki 向 1.0 回落
            if self.ki_mult > 1.0 {
                self.ki_mult -= self.cfg.kp_decay_step * 0.5;
            } else if self.ki_mult < 1.0 {
                self.ki_mult += self.cfg.kp_decay_step * 0.5;
            }
            // Kd 向 1.0 回落
            if self.kd_mult > 1.0 {
                self.kd_mult -= self.cfg.kd_decay_step;
            } else if self.kd_mult < 1.0 {
                self.kd_mult += self.cfg.kd_decay_step;
            }
        }

        // ── 边界限制 ──
        self.kp_mult = self.kp_mult.clamp(self.cfg.min_gain_mult, self.cfg.max_gain_mult);
        self.ki_mult = self.ki_mult.clamp(self.cfg.min_gain_mult, self.cfg.max_gain_mult);
        // Kd 乘子上限收紧到 1.5（原来用 max_gain_mult=2.5，过度阻尼导致系统迟钝）
        let kd_max = self.cfg.max_gain_mult.min(1.5);
        self.kd_mult = self.kd_mult.clamp(self.cfg.min_gain_mult, kd_max);

        // 消除微小偏移（避免永远 0.97 之类的浮点漂移）
        if (self.kp_mult - 1.0).abs() < self.cfg.kp_decay_step { self.kp_mult = 1.0; }
        if (self.ki_mult - 1.0).abs() < self.cfg.kp_decay_step * 0.5 { self.ki_mult = 1.0; }
        if (self.kd_mult - 1.0).abs() < self.cfg.kd_decay_step { self.kd_mult = 1.0; }
    }

    /// 获取当前有效 PID 系数（base × multiplier）
    fn effective_kp(&self) -> f32 { self.base_kp * self.kp_mult }
    fn effective_ki(&self) -> f32 { self.base_ki * self.ki_mult }
    fn effective_kd(&self) -> f32 { self.base_kd * self.kd_mult }

    /// 将有效系数同步到 PidController
    fn apply_to_pid(&self, pid: &mut PidController) {
        pid.kp = self.effective_kp();
        pid.ki = self.effective_ki();
        pid.kd = self.effective_kd();
    }

    fn reset(&mut self) {
        self.kp_mult = 1.0;
        self.ki_mult = 1.0;
        self.kd_mult = 1.0;
        self.fps_norm = 1.0;
        self.deficit_streak = 0;
        self.direction_history.clear();
        self.eval_counter = 0;
        self.consecutive_osc_evals = 0;
    }

    /// 返回是否任何乘子偏离了 1.0（用于日志）
    fn is_active(&self) -> bool {
        self.kp_mult != 1.0 || self.ki_mult != 1.0 || self.kd_mult != 1.0
    }
}

struct PidController {
    kp: f32,   // 可被 AdaptiveGainScheduler 动态修改
    ki: f32,
    kd: f32,
    integral: f32,
    prev_error: f32,
    /// 积分项饱和限幅（防 windup）
    integral_limit: f32,
}

impl PidController {
    fn new(kp: f32, ki: f32, kd: f32) -> Self {
        Self {
            kp, ki, kd,
            integral: 0.0,
            prev_error: 0.0,
            integral_limit: 0.15, // 积分项最大贡献 ±15% perf
        }
    }

    fn compute(&mut self, error: f32, inst_error: f32, dt_norm: f32) -> f32 {
        // 积分项：仅在 deficit 时积累，surplus 时快速衰减
        // 这避免了长期正余量导致积分膨胀，使降频后响应迟钝
        if error < 0.0 {
            self.integral += error * dt_norm;
        } else {
            // surplus 时积分快速泄放
            self.integral *= 0.85;
            self.integral += error * dt_norm * 0.3;
        }
        self.integral = self.integral.clamp(-self.integral_limit, self.integral_limit);

        // 微分项
        let derivative = (error - self.prev_error) / dt_norm.max(0.01);
        self.prev_error = error;

        // PID 输出（正=有余量可省电，负=需要加性能）
        // 使用 inst_error 作为 P 项输入（瞬时响应更快）
        // 使用 error (ema) 作为 I 项输入（长期趋势修正）
        let p_out = self.kp * inst_error;
        let i_out = self.ki * self.integral;
        let d_out = self.kd * derivative;

        p_out + i_out + d_out
    }

    fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
    }
}


pub struct FasController {
    cfg: FasRulesConfig,
    fps_margin: f32,

    // ── PID ──
    pid: PidController,
    /// 自适应增益调度器（进化 2）
    gain_scheduler: AdaptiveGainScheduler,

    // ── 核心状态 ──
    fps_gears: Vec<f32>,
    current_target_fps: f32,
    /// 归一化性能指标，0.0 = 最低性能，1.0 = 最高性能
    perf_index: f32,
    ema_actual_ms: f32,

    // ── 频率策略 ──
    global_max_freq: u32,
    global_min_freq: u32,
    pub policies: Vec<PolicyController>,

    // ── 窗口 & 计数器 ──
    fps_window: FpsWindow,
    log_counter: u32,
    consecutive_normal_frames: u32,

    // ── 硬加载状态机 ──
    consecutive_loading_frames: u32,
    heavy_frame_streak_ms: f32,
    is_in_loading_state: bool,
    normal_frame_tolerance: u32,
    loading_cycle_count: u32,
    loading_cycle_first_ns: u64,
    sustained_loading: bool,
    loading_reentry_cooldown: u32,

    // ── 加载后保护 ──
    post_loading_ignore: u32,
    post_loading_downgrade_guard: u32,

    // ── 齿轮切换 ──
    upgrade_confirm_frames: u32,
    downgrade_confirm_frames: u32,
    upgrade_cooldown: u32,
    gear_change_dampen_frames: u32,
    consecutive_downgrade_count: u32,
    last_downgrade_from_fps: f32,
    stable_gear_frames: u32,
    last_downgrade_perf: f32,
    probe_fail_count: u32,
    last_probe_gear: f32,

    // ── 软加载 ──
    soft_loading_confirm: u32,
    is_in_soft_loading: bool,
    soft_loading_exit_confirm: u32,
    soft_loading_frames_in_state: u32,
    soft_loading_probe_countdown: u32,
    soft_loading_probe_avg_before: f32,
    soft_loading_gear_match_frames: u32,
    soft_loading_matched_gear: f32,

    // ── Native gear 确认（防止瞬间误降档） ──
    native_gear_confirm_frames: u32,
    native_gear_candidate: f32,

    // ── 场景过渡 ──
    scene_transition_guard: u32,
    scene_transition_continuous: u32,
    scene_transition_low_fps_frames: u32,

    // ── Jank ──
    jank_cooldown: u32,
    post_jank_no_decay_frames: u32,
    /// 连续 jank 帧计数器（crit/heavy 需要连续确认）
    jank_streak: u32,

    // ── 降档 Boost ──
    downgrade_boost_active: bool,
    downgrade_boost_remaining: u32,
    downgrade_boost_perf_saved: f32,

    // ── Mismatch ──
    mismatch_result_rx: Option<mpsc::Receiver<Vec<(usize, u32)>>>,
    mismatch_probe_skip: u32,
    mismatch_compensation: f32,
    mismatch_consecutive_cycles: u32,

    // ── 启动/切换后的档位锁定 ──
    // (已废弃：原始版无此机制且表现更好，native gear 检测本身已足够准确)
    // 保留字段以避免重构，但不再使用
    startup_gear_lockout: u32,

    // ── 齿轮乒乓检测 ──
    /// 最近一次升档的目标齿轮
    last_upgrade_to_gear: f32,
    /// 连续在同两个齿轮间来回切换的次数
    gear_pingpong_count: u32,
    /// 乒乓冷却帧数（触发后禁止升档一段时间）
    gear_pingpong_cooldown: u32,
    /// 乒乓冷却期间，fps 持续大幅超出 target 的连续帧数
    /// 达到阈值后提前清除 pp-cd（逃逸机制）
    pp_overshoot_streak: u32,

    // ── perf 动态下限（避免 perf 跌到地板后触发误升档） ──
    /// 当前齿轮下的 perf 动态下限
    dynamic_perf_floor: f32,

    // ── 时间 ──
    frame_time_accumulator_ns: u64,
    init_time: Instant,
    freq_force_counter: u32,
}

impl FasController {
    pub fn new() -> Self {
        let cfg = FasRulesConfig::default();
        let pid = PidController::new(cfg.pid.kp, cfg.pid.ki, cfg.pid.kd);
        let gain_scheduler = AdaptiveGainScheduler::new(
            &cfg.adaptive_pid, cfg.pid.kp, cfg.pid.ki, cfg.pid.kd);
        Self {
            fps_margin: 3.0,
            perf_index: cfg.perf_init,
            pid,
            gain_scheduler,

            fps_gears: cfg.fps_gears.clone(),
            current_target_fps: 60.0,
            ema_actual_ms: 0.0,
            global_max_freq: 9999999,
            global_min_freq: 0,
            policies: Vec::new(),
            fps_window: FpsWindow::new(),
            log_counter: 0,
            consecutive_normal_frames: 0,

            consecutive_loading_frames: 0,
            heavy_frame_streak_ms: 0.0,
            is_in_loading_state: false,
            normal_frame_tolerance: 0,
            loading_cycle_count: 0,
            loading_cycle_first_ns: 0,
            sustained_loading: false,
            loading_reentry_cooldown: 0,

            post_loading_ignore: 0,
            post_loading_downgrade_guard: 0,

            upgrade_confirm_frames: 0,
            downgrade_confirm_frames: 0,
            upgrade_cooldown: 0,
            gear_change_dampen_frames: 0,
            consecutive_downgrade_count: 0,
            last_downgrade_from_fps: 0.0,
            stable_gear_frames: 0,
            last_downgrade_perf: 0.0,
            probe_fail_count: 0,
            last_probe_gear: 0.0,

            soft_loading_confirm: 0,
            is_in_soft_loading: false,
            soft_loading_exit_confirm: 0,
            soft_loading_frames_in_state: 0,
            soft_loading_probe_countdown: 0,
            soft_loading_probe_avg_before: 0.0,
            soft_loading_gear_match_frames: 0,
            soft_loading_matched_gear: 0.0,

            native_gear_confirm_frames: 0,
            native_gear_candidate: 0.0,

            scene_transition_guard: 0,
            scene_transition_continuous: 0,
            scene_transition_low_fps_frames: 0,

            jank_cooldown: 0,
            post_jank_no_decay_frames: 0,
            jank_streak: 0,

            downgrade_boost_active: false,
            downgrade_boost_remaining: 0,
            downgrade_boost_perf_saved: 0.0,

            mismatch_result_rx: None,
            mismatch_probe_skip: 0,
            mismatch_compensation: 0.0,
            mismatch_consecutive_cycles: 0,

            frame_time_accumulator_ns: 0,
            init_time: Instant::now(),
            freq_force_counter: 0,
            startup_gear_lockout: 0,
            last_upgrade_to_gear: 0.0,
            gear_pingpong_count: 0,
            gear_pingpong_cooldown: 0,
            pp_overshoot_streak: 0,
            dynamic_perf_floor: 0.15,

            cfg,
        }
    }

    fn max_gear_min_ns(&self) -> u64 {
        let max_gear = self.fps_gears.iter().copied().fold(60.0_f32, f32::max);
        let max_gear_budget_ns = (1_000_000_000.0 / max_gear) as u64;
        max_gear_budget_ns * 50 / 100
    }

    fn find_nearest_lower_gear(&self, fps: f32) -> Option<f32> {
        self.fps_gears.iter().copied()
            .filter(|&g| g <= fps + self.cfg.soft_loading_gear_match_tolerance
                      && g < self.current_target_fps - 0.5)
            .reduce(f32::max)
    }

    fn detect_native_gear(&self, avg_fps: f32) -> Option<f32> {
        if self.fps_window.count() < 20 { return None; }
        let stddev = self.fps_window.stddev();
        if avg_fps > 5.0 && stddev < avg_fps * 0.10 {
            for &gear in self.fps_gears.iter().rev() {
                if gear < self.current_target_fps - 0.5
                    && (avg_fps - gear).abs() < self.cfg.soft_loading_gear_match_tolerance
                {
                    return Some(gear);
                }
            }
        }
        None
    }

    pub fn load_policies(&mut self, config: &Config, fas_rules: &FasRulesConfig) {
        self.policies.clear();
        self.mismatch_result_rx = None;

        // 同步配置
        self.cfg = fas_rules.clone();
        self.pid = PidController::new(fas_rules.pid.kp, fas_rules.pid.ki, fas_rules.pid.kd);
        // 进化 2：重建自适应调度器
        self.gain_scheduler = AdaptiveGainScheduler::new(
            &fas_rules.adaptive_pid,
            fas_rules.pid.kp, fas_rules.pid.ki, fas_rules.pid.kd);

        if !fas_rules.fps_gears.is_empty() {
            self.fps_gears = fas_rules.fps_gears.clone();
        }
        if let Ok(margin) = fas_rules.fps_margin.parse::<f32>() {
            self.fps_margin = margin;
        }

        let _ = crate::utils::try_write_file(
            "/sys/module/perfmgr/parameters/perfmgr_enable", "0");
        let _ = crate::utils::try_write_file(
            "/sys/module/mtk_fpsgo/parameters/perfmgr_enable", "0");
        log::debug!("FAS: disabled system FEAS/FPSGO");

        let core_info = &config.core_framework;
        let clusters = vec![
            core_info.small_core_path,
            core_info.medium_core_path,
            core_info.big_core_path,
            core_info.super_big_core_path,
        ];

        // ── 进化 1：自动算力探测 ──
        let auto_weights = if fas_rules.auto_capacity_weight {
            match auto_compute_capacity_weights(&clusters) {
                Some(w) => {
                    info!("FAS: auto capacity detected:");
                    for &(pid, weight) in &w {
                        let cap = probe_policy_capacity(pid).unwrap_or(0);
                        info!("  P{}: capacity={} → weight={:.2}", pid, cap, weight);
                    }
                    Some(w)
                }
                None => {
                    log::warn!("FAS: auto capacity probe failed, falling back to YAML config");
                    None
                }
            }
        } else {
            info!("FAS: auto_capacity_weight=false, using YAML profiles");
            None
        };

        let mut global_max = 0u32;
        let mut global_min = u32::MAX;

        for (idx, &policy_id) in clusters.iter().enumerate() {
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

                // 进化 1：优先使用自动探测的 weight，fallback 到 YAML 配置
                let profile = if let Some(ref aw) = auto_weights {
                    aw.iter()
                        .find(|&&(pid, _)| pid == policy_id)
                        .map(|&(_, w)| ClusterProfile { capacity_weight: w })
                        .unwrap_or_else(|| {
                            fas_rules.cluster_profiles.get(idx)
                                .cloned().unwrap_or_default()
                        })
                } else {
                    fas_rules.cluster_profiles.get(idx)
                        .cloned().unwrap_or_default()
                };

                let weight_source = if auto_weights.is_some() { "auto" } else { "yaml" };
                info!("FAS[P{}] cluster={} freqs={}-{} MHz | weight={:.2} ({})",
                    policy_id, idx,
                    min_f / 1000, max_f / 1000,
                    profile.capacity_weight, weight_source);

                self.policies.push(PolicyController {
                    max_writer, min_writer,
                    available_freqs: avail_freqs,
                    current_freq: max_f,
                    policy_id: policy_id as usize,
                    mismatch_count: 0,
                    external_lock_cooldown: 0,
                    cluster_profile: profile,
                    freq_hold_frames: 0,
                });

                if max_f > global_max { global_max = max_f; }
                if min_f < global_min { global_min = min_f; }
            }
        }

        self.global_max_freq = if global_max == 0 { 9999999 } else { global_max };
        self.global_min_freq = if global_min == u32::MAX { 0 } else { global_min };
        self.current_target_fps = *self.fps_gears.iter()
            .reduce(|a, b| if a > b { a } else { b }).unwrap_or(&60.0);

        // ── 重置所有运行时状态 ──
        self.perf_index = self.cfg.perf_init;
        self.ema_actual_ms = 0.0;
        self.pid.reset();
        self.gain_scheduler.reset();
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
        self.jank_streak = 0;
        self.mismatch_probe_skip = 0;
        self.soft_loading_gear_match_frames = 0;
        self.soft_loading_matched_gear = 0.0;
        self.native_gear_confirm_frames = 0;
        self.native_gear_candidate = 0.0;
        self.mismatch_compensation = 0.0;
        self.mismatch_consecutive_cycles = 0;
        self.last_downgrade_perf = 0.0;
        self.probe_fail_count = 0;
        self.last_probe_gear = 0.0;
        // 不再使用 startup_gear_lockout：native gear 检测的 stddev 过滤已足够准确
        self.startup_gear_lockout = 0;
        self.last_upgrade_to_gear = 0.0;
        self.gear_pingpong_count = 0;
        self.gear_pingpong_cooldown = 0;
        self.pp_overshoot_streak = 0;
        self.dynamic_perf_floor = self.cfg.perf_floor;

        info!("FAS init | target:{:.0}fps margin:{:.1} clusters:{} perf:{:.2}",
            self.current_target_fps, self.fps_margin,
            self.policies.len(), self.perf_index);
        info!("FAS PID  | Kp={:.4} Ki={:.4} Kd={:.4} | adaptive={}",
            self.cfg.pid.kp, self.cfg.pid.ki, self.cfg.pid.kd,
            if self.cfg.adaptive_pid.enabled { "ON" } else { "OFF" });
        info!("FAS config | heavy:{}ms loading:{}ms floor:{:.2} ceil:{:.2} hyst:{}",
            self.cfg.heavy_frame_threshold_ms, self.cfg.loading_cumulative_ms,
            self.cfg.perf_floor, self.cfg.perf_ceil, self.cfg.freq_hysteresis);
        info!("FAS capacity | auto={} profiles={}",
            self.cfg.auto_capacity_weight, self.policies.len());

        self.init_time = Instant::now();
        // 冷启动：拉到高性能
        self.perf_index = self.cfg.perf_cold_boot;
        self.apply_freqs();
    }

    fn apply_freqs(&mut self) {
        self.freq_force_counter = self.freq_force_counter.wrapping_add(1);
        let force_this_cycle =
            self.freq_force_counter % self.cfg.freq_force_reapply_interval == 0;

        // 全局归一化 perf（0.0~1.0），加上 mismatch 补偿
        let global_ratio = (self.perf_index + self.mismatch_compensation).clamp(0.0, 1.0);

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

            // 频率保持冷却递减
            if policy.freq_hold_frames > 0 {
                policy.freq_hold_frames -= 1;
                if !force_this_cycle { continue; }
            }

            // ── capacity_weight 幂次曲线 ──
            // 所有集群都参与，weight 越大 → 同 global_ratio 下频率越低
            let weight = policy.cluster_profile.capacity_weight.max(0.1);
            let adjusted_ratio = global_ratio.powf(weight);

            let pmin = *policy.available_freqs.first().unwrap() as f32;
            let pmax = *policy.available_freqs.last().unwrap() as f32;
            let target_val = pmin + adjusted_ratio * (pmax - pmin);

            // 查找最接近的可用频率
            let target_freq = policy.available_freqs.iter().copied()
                .min_by(|&a, &b| {
                    ((a as f32 - target_val).abs())
                        .partial_cmp(&(b as f32 - target_val).abs()).unwrap()
                })
                .unwrap_or(pmax as u32);

            // 迟滞防抖
            if target_freq != policy.current_freq {
                let cur_idx = policy.available_freqs.iter()
                    .position(|&f| f == policy.current_freq);
                let tgt_idx = policy.available_freqs.iter()
                    .position(|&f| f == target_freq);
                let apply = match (cur_idx, tgt_idx) {
                    (Some(ci), Some(ti)) if (ci as i32 - ti as i32).abs() == 1 => {
                        let cur_r = (policy.current_freq as f32 - pmin) / (pmax - pmin);
                        (adjusted_ratio - cur_r).abs() > self.cfg.freq_hysteresis
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
        self.native_gear_confirm_frames = 0;
        self.native_gear_candidate = 0.0;
        self.fps_window.clear();
        self.ema_actual_ms = 0.0;
        self.pid.reset();
        self.gain_scheduler.reset();
        self.downgrade_confirm_frames = 0;
        self.upgrade_confirm_frames = 0;
        self.downgrade_boost_active = false;
        self.downgrade_boost_remaining = 0;
        // 从 0.50 提到 0.55，给 PID 更多稳定空间
        // 避免 perf 立刻跌到 dynamic_floor 附近触发误升档
        self.perf_index = 0.55;
        // soft-load 降档是游戏场景切换（如过场/大厅），不是升档失败，
        // 升档冷却和 dampen 应大幅缩短以加速恢复
        // F10: dampen=10, cooldown=10 → 30fps 下 ~0.33 秒即可开始升档
        self.gear_change_dampen_frames = 10;
        self.upgrade_cooldown = (self.cfg.upgrade_cooldown_after_downgrade / 6).max(10);
        self.consecutive_downgrade_count = 1;
        self.last_downgrade_from_fps = old_fps;
        self.last_downgrade_perf = 0.50;
        self.post_loading_downgrade_guard = 0;
        log::info!("FAS: soft-load downgrade {:.0}->{:.0}fps | P->{:.2}",
            old_fps, target_gear, self.perf_index);
        // 场景切换降档是全新上下文，重置乒乓和探测失败计数
        self.gear_pingpong_count = 0;
        self.gear_pingpong_cooldown = 0;
        self.pp_overshoot_streak = 0;
        self.probe_fail_count = 0;
        self.last_probe_gear = 0.0;
        self.last_upgrade_to_gear = 0.0;
    }

    pub fn update_frame(&mut self, frame_delta_ns: u64) {
        if frame_delta_ns == 0 || self.policies.is_empty() { return; }

        // 冷启动保护
        if self.init_time.elapsed().as_millis() < self.cfg.cold_boot_ms as u128 {
            if self.perf_index < self.cfg.perf_cold_boot {
                self.perf_index = self.cfg.perf_cold_boot;
                self.apply_freqs();
            }
            return;
        }

        self.frame_time_accumulator_ns =
            self.frame_time_accumulator_ns.wrapping_add(frame_delta_ns);

        let budget_ns = (1_000_000_000.0 / self.current_target_fps.max(1.0)) as u64;
        let min_ns = self.max_gear_min_ns();
        let max_ns = (self.cfg.fixed_max_frame_ms * 1_000_000.0) as u64;
        let actual_ms = frame_delta_ns as f32 / 1_000_000.0;
        let budget_ms = budget_ns as f32 / 1_000_000.0;
        let is_heavy = actual_ms > self.cfg.heavy_frame_threshold_ms;

        if frame_delta_ns < min_ns { return; }

        // ── 应用切换/息屏检测 ──
        if actual_ms > self.cfg.app_switch_gap_ms {
            let was_loading = self.is_in_loading_state;
            let was_soft = self.is_in_soft_loading;

            self.is_in_soft_loading = false;
            self.soft_loading_confirm = 0;
            self.soft_loading_exit_confirm = 0;
            self.soft_loading_frames_in_state = 0;
            self.soft_loading_probe_countdown = 0;
            self.soft_loading_gear_match_frames = 0;
            self.soft_loading_matched_gear = 0.0;
            self.native_gear_confirm_frames = 0;
            self.native_gear_candidate = 0.0;
            self.scene_transition_guard = 0;
            self.scene_transition_continuous = 0;
            self.scene_transition_low_fps_frames = 0;
            self.jank_cooldown = 0;
            self.downgrade_boost_active = false;
            self.downgrade_boost_remaining = 0;
            self.post_jank_no_decay_frames = 0;
            self.jank_streak = 0;
            self.mismatch_compensation = 0.0;
            self.mismatch_consecutive_cycles = 0;
            self.last_downgrade_perf = 0.0;
            self.probe_fail_count = 0;
            self.last_probe_gear = 0.0;
            self.last_upgrade_to_gear = 0.0;
            self.gear_pingpong_count = 0;
            self.gear_pingpong_cooldown = 0;
        self.pp_overshoot_streak = 0;
            self.dynamic_perf_floor = self.cfg.perf_floor;
            self.pid.reset();
            self.gain_scheduler.reset();

            if was_loading || was_soft {
                self.is_in_loading_state = false;
                self.consecutive_loading_frames = 0;
                self.heavy_frame_streak_ms = 0.0;
                self.normal_frame_tolerance = 0;
                self.perf_index = self.cfg.loading_perf_ceiling;
                self.fps_window.clear();
                self.ema_actual_ms = 0.0;
                self.post_loading_ignore = self.cfg.app_switch_ignore_frames;
                self.loading_reentry_cooldown = self.cfg.loading_reentry_cooldown * 2;
                self.gear_change_dampen_frames = 90;
                self.post_loading_downgrade_guard = self.cfg.post_loading_downgrade_guard;
                self.startup_gear_lockout = 0;
                self.apply_freqs();
                log::info!("FAS: app switch (resume loading) ({:.0}ms) | P->{:.2}",
                    actual_ms, self.perf_index);
            } else {
                self.is_in_loading_state = false;
                self.consecutive_loading_frames = 0;
                self.heavy_frame_streak_ms = 0.0;
                self.normal_frame_tolerance = 0;
                self.fps_window.clear();
                self.ema_actual_ms = 0.0;
                self.downgrade_confirm_frames = 0;
                self.upgrade_confirm_frames = 0;
                self.perf_index = self.cfg.app_switch_resume_perf;
                self.post_loading_ignore = self.cfg.app_switch_ignore_frames;
                self.gear_change_dampen_frames = 60;
                self.post_loading_downgrade_guard = self.cfg.post_loading_downgrade_guard;
                self.loading_reentry_cooldown = self.cfg.loading_reentry_cooldown;
                self.startup_gear_lockout = 0;
                self.apply_freqs();
                log::info!("FAS: app switch ({:.0}ms) | P->{:.2}", actual_ms, self.perf_index);
            }
            return;
        }

        // ── 重帧 & 硬加载状态机 ──
        if is_heavy {
            if self.loading_reentry_cooldown > 0 {
                self.perf_index = (self.perf_index + 0.03).min(1.0);
                self.apply_freqs();
                self.loading_reentry_cooldown -= 1;
                log::debug!("FAS: heavy {:.1}ms during reentry cooldown ({}), perf->{:.2}",
                    actual_ms, self.loading_reentry_cooldown, self.perf_index);
                return;
            }

            self.consecutive_loading_frames += 1;
            self.heavy_frame_streak_ms += actual_ms;
            self.normal_frame_tolerance = 0;

            if !self.is_in_loading_state
                && self.heavy_frame_streak_ms > self.cfg.loading_cumulative_ms
            {
                self.is_in_loading_state = true;

                let now = self.frame_time_accumulator_ns;
                if self.loading_cycle_count == 0
                    || now.wrapping_sub(self.loading_cycle_first_ns)
                        > self.cfg.sustained_loading_window_ns
                {
                    self.loading_cycle_count = 1;
                    self.loading_cycle_first_ns = now;
                } else {
                    self.loading_cycle_count += 1;
                }

                if !self.sustained_loading
                    && self.loading_cycle_count >= self.cfg.sustained_loading_cycle_threshold
                {
                    self.sustained_loading = true;
                    log::info!("FAS: enter sustained loading ({}x cycles)",
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
                self.native_gear_confirm_frames = 0;
                self.native_gear_candidate = 0.0;

                let old = self.perf_index;
                self.perf_index = self.perf_index
                    .clamp(self.cfg.loading_perf_floor, self.cfg.loading_perf_ceiling);
                if old != self.perf_index { self.apply_freqs(); }
                log::info!("FAS: enter loading ({} frames, {:.0}ms) | P {:.2}->{:.2}{}",
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
                if self.normal_frame_tolerance < self.cfg.loading_normal_tolerance {
                    log::debug!("FAS: loading tolerance {}/{} ({:.1}ms)",
                        self.normal_frame_tolerance, self.cfg.loading_normal_tolerance, actual_ms);
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
                self.pid.reset();
                self.gain_scheduler.reset();
                let old = self.perf_index;

                if self.sustained_loading {
                    self.perf_index = self.perf_index
                        .clamp(self.cfg.post_loading_perf_min, self.cfg.post_loading_perf_max);
                    self.post_loading_ignore = self.cfg.sustained_post_loading_ignore;
                    self.gear_change_dampen_frames = 120;
                    self.post_loading_downgrade_guard =
                        self.cfg.post_loading_downgrade_guard + 60;
                    self.loading_reentry_cooldown = self.cfg.loading_reentry_cooldown;
                    log::info!("FAS: exit loading [sustained] | P {:.2}->{:.2} | ignore {} guard {}",
                        old, self.perf_index, self.post_loading_ignore,
                        self.post_loading_downgrade_guard);
                } else {
                    self.perf_index = self.perf_index
                        .clamp(self.cfg.post_loading_perf_min, self.cfg.post_loading_perf_max);
                    self.post_loading_ignore = self.cfg.post_loading_ignore_frames;
                    self.gear_change_dampen_frames = 60;
                    self.post_loading_downgrade_guard = self.cfg.post_loading_downgrade_guard;
                    self.loading_reentry_cooldown = self.cfg.loading_reentry_cooldown;
                    log::info!("FAS: exit loading | P {:.2}->{:.2} | ignore {} guard {}",
                        old, self.perf_index, self.post_loading_ignore,
                        self.post_loading_downgrade_guard);
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
        if self.startup_gear_lockout > 0 { self.startup_gear_lockout -= 1; }
        if self.gear_pingpong_cooldown > 0 { self.gear_pingpong_cooldown -= 1; }

        // ── 乒乓冷却逃逸机制 ──
        // 如果在 pp-cd 期间，fps 持续大幅超出当前目标，说明乒乓是假阳性
        // （例如从加载动画的60fps进入对局的144fps场景）
        // 此时提前清除 pp-cd，让升档路径恢复工作
        if self.gear_pingpong_cooldown > 0 && self.fps_window.count() >= 20 {
            // 条件：avg > target×1.25 且 perf 较低（说明频率有大量余量）
            let pp_overshoot = avg_fps > self.current_target_fps * 1.25
                && self.perf_index < 0.45;
            if pp_overshoot {
                self.pp_overshoot_streak += 1;
            } else {
                self.pp_overshoot_streak = self.pp_overshoot_streak.saturating_sub(3);
            }
            // 连续 45 帧超出（60fps 下约 0.75 秒）→ 确认是假阳性，清除
            if self.pp_overshoot_streak >= 45 {
                log::info!("FAS: pp-cd escape! avg={:.1} target={:.0} P={:.2} \
                    (streak={}, pp-cd was {})",
                    avg_fps, self.current_target_fps, self.perf_index,
                    self.pp_overshoot_streak, self.gear_pingpong_cooldown);
                self.gear_pingpong_cooldown = 0;
                self.pp_overshoot_streak = 0;
                // 同时重置乒乓计数——既然是假阳性，之前的记录无效
                self.gear_pingpong_count = 0;
                self.probe_fail_count = 0;
            }
        } else {
            self.pp_overshoot_streak = 0;
        }

        // ── 场景过渡检测（逻辑保持不变，只替换常量引用） ──
        if self.scene_transition_guard > 0 && actual_ms > 200.0 {
            log::info!("FAS: scene guard cleared by extreme frame {:.0}ms", actual_ms);
            self.scene_transition_guard = 0;
            self.scene_transition_continuous = 0;
            self.scene_transition_low_fps_frames = 0;
            self.jank_cooldown = 0;
        }

        if self.fps_window.count() >= 20 {
            let cv = if avg_fps > 1.0 { self.fps_window.stddev() / avg_fps } else { 0.0 };
            let fps_floor = self.current_target_fps * self.cfg.scene_transition_fps_floor_ratio;
            let recent5 = self.fps_window.recent_mean(5);

            if cv > self.cfg.scene_transition_cv_threshold
                && avg_fps > fps_floor && recent5 > fps_floor
            {
                if self.scene_transition_guard == 0 {
                    self.scene_transition_continuous = 0;
                    self.scene_transition_low_fps_frames = 0;
                    log::info!("FAS: scene transition (CV={:.2} avg={:.1}) | guard {}",
                        cv, avg_fps, self.cfg.scene_transition_guard_frames);
                }
                self.scene_transition_continuous += 1;

                if self.scene_transition_continuous < self.cfg.scene_transition_max_continuous {
                    self.scene_transition_guard = self.cfg.scene_transition_guard_frames;
                } else if self.scene_transition_guard == 1 {
                    log::info!("FAS: scene transition max ({}), force clearing",
                        self.scene_transition_continuous);
                    self.scene_transition_continuous = 0;
                    self.scene_transition_low_fps_frames = 0;
                }
            }

            let recent = self.fps_window.recent_mean(15);
            if self.scene_transition_guard > 0 && recent < fps_floor {
                self.scene_transition_low_fps_frames += 1;
                if self.scene_transition_low_fps_frames >= self.cfg.scene_transition_force_exit_frames {
                    log::info!("FAS: scene guard force-exit ({:.1} < {:.0} for {}f)",
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
            if now.wrapping_sub(self.loading_cycle_first_ns)
                > self.cfg.sustained_loading_window_ns * 2
            {
                log::info!("FAS: sustained loading cleared (stable >{}s)",
                    self.cfg.sustained_loading_window_ns * 2 / 1_000_000_000);
                self.sustained_loading = false;
                self.loading_cycle_count = 0;
            }
        }

        // ── 软加载检测（逻辑保持不变，常量替换为 cfg） ──
        let soft_loading_fps_threshold = self.current_target_fps * self.cfg.soft_loading_fps_ratio;
        let effective_fps = if self.fps_window.count() >= 20 {
            let cv = if avg_fps > 1.0 { self.fps_window.stddev() / avg_fps } else { 0.0 };
            if cv > 0.3 { self.fps_window.recent_mean(20).min(avg_fps) } else { avg_fps }
        } else {
            avg_fps
        };

        if !self.is_in_soft_loading {
            // 启动锁定期间 & 降档保护期间不允许进入 soft-loading
            if self.post_loading_downgrade_guard == 0
                && effective_fps < soft_loading_fps_threshold
                && self.perf_index >= self.cfg.soft_loading_perf_threshold
                && self.fps_window.count() >= 15
                && !self.downgrade_boost_active
            {
                // 当 avg < target×0.30 且帧率稳定，即使没有精确匹配齿轮也直接降档
                let extreme_gap_ratio = 0.30;
                let is_extreme_gap = avg_fps < self.current_target_fps * extreme_gap_ratio
                    && self.fps_window.count() >= 10
                    && self.fps_window.stddev() < avg_fps.max(1.0) * 0.20;

                if is_extreme_gap {
                    // 找到 avg 附近最近的齿轮
                    if let Some(nearest_gear) = self.fps_gears.iter().copied()
                        .filter(|&g| g <= avg_fps + 5.0 && g < self.current_target_fps - 0.5)
                        .reduce(f32::max)
                    {
                        if (nearest_gear - self.native_gear_candidate).abs() < 1.0 {
                            self.native_gear_confirm_frames += 1;
                        } else {
                            self.native_gear_candidate = nearest_gear;
                            self.native_gear_confirm_frames = 1;
                        }
                        // 极端 gap 只需 4 帧确认
                        if self.native_gear_confirm_frames >= 4 {
                            log::info!("FAS: extreme gap downgrade: avg={:.1} -> gear {:.0} (target was {:.0})",
                                avg_fps, nearest_gear, self.current_target_fps);
                            self.native_gear_confirm_frames = 0;
                            self.native_gear_candidate = 0.0;
                            self.perform_soft_loading_downgrade(nearest_gear);
                            self.apply_freqs();
                            return;
                        }
                    }
                } else if let Some(native_gear) = self.detect_native_gear(avg_fps) {
                    // 原始版逻辑：native gear 检测成功就直接降档
                    // detect_native_gear 内部的 stddev < 10% 过滤已足够准确
                    if (native_gear - self.native_gear_candidate).abs() < 1.0 {
                        self.native_gear_confirm_frames += 1;
                    } else {
                        self.native_gear_candidate = native_gear;
                        self.native_gear_confirm_frames = 1;
                    }

                    let extreme_gap = avg_fps < self.current_target_fps * 0.35;
                    let confirm_needed = if extreme_gap { 6_u32 } else { 15 };

                    if self.native_gear_confirm_frames >= confirm_needed {
                        log::info!("FAS: native gear detected: avg={:.1} -> gear {:.0} ({}f)",
                            avg_fps, native_gear, self.native_gear_confirm_frames);
                        self.native_gear_confirm_frames = 0;
                        self.native_gear_candidate = 0.0;
                        self.perform_soft_loading_downgrade(native_gear);
                        self.apply_freqs();
                        return;
                    }
                } else {
                    self.native_gear_confirm_frames = 0;
                    self.native_gear_candidate = 0.0;
                }

                self.soft_loading_confirm += 1;
                if self.soft_loading_confirm >= self.cfg.soft_loading_confirm_frames {
                    self.is_in_soft_loading = true;
                    self.soft_loading_exit_confirm = 0;
                    self.soft_loading_frames_in_state = 0;
                    self.soft_loading_probe_countdown = 0;
                    self.soft_loading_gear_match_frames = 0;
                    self.soft_loading_matched_gear = 0.0;
                    let old = self.perf_index;
                    self.perf_index = self.cfg.soft_loading_perf_cap;
                    self.scene_transition_guard = 0;
                    self.scene_transition_continuous = 0;
                    self.scene_transition_low_fps_frames = 0;
                    self.jank_cooldown = 0;
                    self.downgrade_boost_active = false;
                    self.downgrade_boost_remaining = 0;
                    self.downgrade_confirm_frames = 0;
                    self.apply_freqs();
                    log::info!("FAS: enter soft loading | eff:{:.1} avg:{:.1} | P {:.2}->{:.2}",
                        effective_fps, avg_fps, old, self.perf_index);
                }
            } else {
                self.soft_loading_confirm = 0;
                self.native_gear_confirm_frames = 0;
                self.native_gear_candidate = 0.0;
            }
        } else {
            self.soft_loading_frames_in_state += 1;

            // 软加载内齿轮匹配
            if self.fps_window.count() >= 20 {
                if let Some(matched_gear) = self.find_nearest_lower_gear(avg_fps) {
                    let near = (avg_fps - matched_gear).abs()
                        < self.cfg.soft_loading_gear_match_tolerance;
                    let stable = self.fps_window.stddev() < avg_fps * 0.15;

                    // 趋势校验：如果帧率还在上升中，不应做齿轮匹配降档
                    // 防止游戏过渡期（30→60→120+）中途被锁定在中间齿轮
                    let recent5 = self.fps_window.recent_mean(5);
                    let recent20 = self.fps_window.recent_mean(20);
                    let fps_trending_up = recent5 > recent20 + 3.0;

                    if near && stable && !fps_trending_up {
                        if (matched_gear - self.soft_loading_matched_gear).abs() < 1.0 {
                            self.soft_loading_gear_match_frames += 1;
                        } else {
                            self.soft_loading_matched_gear = matched_gear;
                            self.soft_loading_gear_match_frames = 1;
                        }
                        if self.soft_loading_gear_match_frames
                            >= self.cfg.soft_loading_downgrade_check_frames
                        {
                            log::info!("FAS: soft-load gear match: avg={:.1} -> gear {:.0} ({}f)",
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
                if self.perf_index > self.cfg.soft_loading_perf_cap {
                    self.perf_index = (self.perf_index
                        - self.cfg.soft_loading_probe_fail_decay_step)
                        .max(self.cfg.soft_loading_perf_cap);
                }

                if self.soft_loading_frames_in_state % self.cfg.soft_loading_probe_interval == 0
                    && self.soft_loading_frames_in_state > 0
                {
                    self.soft_loading_probe_countdown = self.cfg.soft_loading_probe_duration;
                    self.soft_loading_probe_avg_before = avg_fps;
                    log::info!("FAS: soft probe start | baseline:{:.1} | cap->{:.2}",
                        avg_fps, self.cfg.soft_loading_probe_perf_cap);
                }
            } else {
                if self.perf_index > self.cfg.soft_loading_probe_perf_cap {
                    self.perf_index = self.cfg.soft_loading_probe_perf_cap;
                }
                self.soft_loading_probe_countdown -= 1;

                if self.soft_loading_probe_countdown == 0 {
                    let probe_avg = self.fps_window
                        .recent_mean(self.cfg.soft_loading_probe_duration as usize);
                    let probe_max = self.fps_window
                        .recent_max(self.cfg.soft_loading_probe_duration as usize);
                    let gain = if self.soft_loading_probe_avg_before > 0.1 {
                        (probe_avg - self.soft_loading_probe_avg_before)
                            / self.soft_loading_probe_avg_before
                    } else { 0.0 };

                    if gain >= self.cfg.soft_loading_probe_fps_gain_ratio {
                        self.is_in_soft_loading = false;
                        self.soft_loading_confirm = 0;
                        self.soft_loading_exit_confirm = 0;
                        self.soft_loading_frames_in_state = 0;
                        self.soft_loading_gear_match_frames = 0;
                        self.soft_loading_matched_gear = 0.0;
                        self.fps_window.clear();
                        self.downgrade_confirm_frames = 0;
                        self.ema_actual_ms = 0.0;
                        self.pid.reset();
                        self.gain_scheduler.reset();
                        self.post_loading_downgrade_guard =
                            self.cfg.post_loading_downgrade_guard / 2;
                        self.gear_change_dampen_frames = 60;
                        log::info!("FAS: exit soft loading [probe] | avg:{:.1} max:{:.1} \
                            vs baseline:{:.1} gain:{:+.0}%",
                            probe_avg, probe_max, self.soft_loading_probe_avg_before,
                            gain * 100.0);
                    } else {
                        log::info!("FAS: soft probe end | avg:{:.1} max:{:.1} \
                            vs baseline:{:.1} gain:{:+.0}% | still loading",
                            probe_avg, probe_max,
                            self.soft_loading_probe_avg_before, gain * 100.0);
                    }
                }
            }

            let is_avg_recovered = avg_fps >= self.current_target_fps * 0.7;
            let recent = self.fps_window.recent_mean(self.cfg.soft_loading_breakthrough_window);
            let is_breakthrough =
                recent >= self.current_target_fps * self.cfg.soft_loading_breakthrough_fps_ratio;

            if is_avg_recovered || is_breakthrough {
                self.soft_loading_exit_confirm += 1;
                let exit_frames_needed = if is_breakthrough {
                    self.cfg.soft_loading_exit_frames_breakthrough
                } else {
                    self.cfg.soft_loading_exit_frames
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
                    self.pid.reset();
                    self.gain_scheduler.reset();
                    self.post_loading_downgrade_guard =
                        self.cfg.post_loading_downgrade_guard / 2;
                    self.gear_change_dampen_frames = 60;
                    log::info!("FAS: exit soft loading | avg:{:.1} recent:{:.1} {}",
                        avg_fps, recent,
                        if is_breakthrough { "[breakthrough]" } else { "[avg-recovered]" });
                }
            } else {
                self.soft_loading_exit_confirm = 0;
            }

            // 软加载期间禁止降档逻辑
            self.downgrade_confirm_frames = 0;
            self.downgrade_boost_active = false;
            self.downgrade_boost_remaining = 0;
        }

        // ── 升档 ──
        if !self.is_in_soft_loading {
            let recent30 = self.fps_window.recent_mean(30);

            // 快速恢复机制（含低性能升档探测）
            // 场景1: fps 远超 target → 齿轮明显错误，多步跳升
            // 场景2: perf 极低 + fps 近 target → 有余量，尝试升一档
            if let Some(target) = next_gear {
                let overshoot_ratio = if self.current_target_fps > 1.0 {
                    avg_fps / self.current_target_fps
                } else { 1.0 };

                // 用 recent15 作为快速参考——刚从 soft-load 恢复时
                // avg 窗口内有大量旧帧数据（如30fps的旧样本），recent15 更准确
                let recent15 = self.fps_window.recent_mean(15);
                let recent_overshoot = if self.current_target_fps > 1.0 {
                    recent15 / self.current_target_fps
                } else { 1.0 };

                // 低档位（≤60fps）进一步减少窗口等待（15帧≈0.5s@30fps）
                let fast_recover_min_count = if self.current_target_fps <= 60.0 { 15 } else { 30 };

                // 场景1: 大幅超出 → 跳升到最高可达齿轮
                // 用 recent_overshoot 和 avg overshoot 中较大者判断
                // overshoot 门槛从 1.5 降到 1.35（PID 模式下难以达到 1.5x）
                let effective_overshoot = overshoot_ratio.max(recent_overshoot);
                let is_big_overshoot = effective_overshoot > 1.35
                    && self.fps_window.count() >= fast_recover_min_count
                    && recent30 > self.current_target_fps * 1.2
                    && self.perf_index < 0.45;

                // 极端 overshoot (>1.5x) 时即使在 pp-cd 内也允许 fast-recover
                // 这说明乒乓检测是假阳性（场景切换而非真实能力不足）
                let extreme_overshoot = effective_overshoot > 1.50
                    && self.fps_window.count() >= fast_recover_min_count
                    && recent30 > self.current_target_fps * 1.35
                    && self.perf_index < 0.45;

                // 场景2: fps 接近或略超 target + perf 较低 → 有频率余量，尝试升一档
                let low_perf_probe_threshold = (self.dynamic_perf_floor + 0.10).min(0.42);
                let low_perf_min_count = if self.current_target_fps <= 60.0 { 45 } else { 90 };
                let is_low_perf_probe = overshoot_ratio > 1.01
                    && self.fps_window.count() >= low_perf_min_count
                    && self.perf_index < low_perf_probe_threshold
                    && recent30 > self.current_target_fps * 0.98
                    && self.upgrade_cooldown == 0
                    && self.probe_fail_count < 2
                    && self.gear_pingpong_cooldown == 0;

                if (is_big_overshoot || is_low_perf_probe || extreme_overshoot)
                    && !self.downgrade_boost_active
                    && (self.gear_pingpong_cooldown == 0 || extreme_overshoot)
                {
                    if extreme_overshoot && self.gear_pingpong_cooldown > 0 {
                        log::info!("FAS: extreme overshoot {:.2}x bypassing pp-cd:{}",
                            effective_overshoot, self.gear_pingpong_cooldown);
                        self.gear_pingpong_cooldown = 0;
                        self.pp_overshoot_streak = 0;
                        self.gear_pingpong_count = 0;
                        self.probe_fail_count = 0;
                    }

                    let best_gear = if is_big_overshoot || extreme_overshoot {
                        // soft-load 恢复后 avg 被旧帧拖低，recent 更反映当前能力
                        let reference_fps = recent15.max(recent30).max(avg_fps);
                        self.fps_gears.iter().copied()
                            .filter(|&g| g <= reference_fps + 15.0 && g > self.current_target_fps + 0.5)
                            .reduce(f32::max)
                    } else {
                        // 单步试探
                        Some(target)
                    };

                    if let Some(jump_target) = best_gear {
                        let reason = if is_big_overshoot || extreme_overshoot { "fast-recover" } else { "low-perf-probe" };
                        log::info!("FAS: {} {:.0}->{:.0}fps (avg={:.1} recent30={:.1} P={:.2})",
                            reason, self.current_target_fps, jump_target, avg_fps, recent30, self.perf_index);
                        self.last_upgrade_to_gear = jump_target;
                        self.current_target_fps = jump_target;
                        self.upgrade_confirm_frames = 0;
                        self.downgrade_confirm_frames = 0;
                        self.ema_actual_ms = 0.0;
                        self.pid.reset();
                        self.gain_scheduler.reset();
                        self.fps_window.clear();
                        self.perf_index = if is_big_overshoot || extreme_overshoot { 0.60 } else { 0.70 };
                        let fr_gear_scale = (self.current_target_fps / 60.0).clamp(0.4, 1.0);
                        self.gear_change_dampen_frames = ((60.0 * fr_gear_scale).max(20.0)) as u32;
                        self.upgrade_cooldown = if is_big_overshoot || extreme_overshoot { 0 } else {
                            self.cfg.upgrade_cooldown_after_downgrade / 2
                        };
                        self.consecutive_downgrade_count = 0;
                        self.stable_gear_frames = 0;
                        self.downgrade_boost_active = false;
                        self.downgrade_boost_remaining = 0;
                        self.last_downgrade_perf = 0.0;
                        // fast-recover 是确信型升档（fps大幅超目标），可重置失败计数
                        // low-perf-probe 是试探型升档，失败计数必须保留，否则永远无法停止尝试
                        if is_big_overshoot || extreme_overshoot {
                            self.probe_fail_count = 0;
                        }
                        self.last_probe_gear = if is_big_overshoot || extreme_overshoot { 0.0 } else { jump_target };
                        self.native_gear_confirm_frames = 0;
                        self.native_gear_candidate = 0.0;
                        self.post_loading_downgrade_guard =
                            (self.cfg.post_loading_downgrade_guard / 3).max(15);
                        self.apply_freqs();
                        return;
                    }
                }
            }

            if let Some(target) = next_gear {
                // 30fps: scale=0.5 → 30帧确认(≈1秒), 60fps: scale=1.0 → 60帧确认(≈1秒)
                // 确保确认时间大致恒定而非帧数恒定
                let gear_scale = (self.current_target_fps / 60.0).clamp(0.4, 1.0);

                if self.upgrade_cooldown > 0 || self.gear_pingpong_cooldown > 0 {
                    self.upgrade_confirm_frames = 0;
                } else if recent30 >= target - 10.0
                    && avg_fps >= self.current_target_fps * 0.9
                    && self.fps_window.count() >= 60
                {
                    self.upgrade_confirm_frames += 1;
                    self.downgrade_confirm_frames = 0;
                    let upgrade_confirm_needed = ((45.0 * gear_scale).max(18.0)) as u32;
                    if self.upgrade_confirm_frames >= upgrade_confirm_needed {
                        log::info!("FAS: {:.0}->{:.0}fps (recent30={:.1} avg={:.1})",
                            self.current_target_fps, target, recent30, avg_fps);
                        self.last_upgrade_to_gear = target;
                        self.current_target_fps = target;
                        self.upgrade_confirm_frames = 0;
                        self.ema_actual_ms = 0.0;
                        self.pid.reset();
                        self.gain_scheduler.reset();
                        self.fps_window.clear();
                        self.gear_change_dampen_frames = ((90.0 * gear_scale).max(30.0)) as u32;
                        self.consecutive_downgrade_count = 0;
                        self.stable_gear_frames = 0;
                        self.downgrade_boost_active = false;
                        self.downgrade_boost_remaining = 0;
                        self.last_downgrade_perf = 0.0;
                        self.probe_fail_count = 0;
                        self.last_probe_gear = 0.0;
                    }
                // 稳定低 perf 升档：PID 控制下 fps 紧贴 target，不会大幅超出，
                // 但如果 perf 持续较低说明频率有余量，应尝试升档
                // 这对应原始版中 fps 自然超出 target 触发升档的场景
                } else if avg_fps >= self.current_target_fps - 5.0
                    && self.perf_index < 0.55
                    && self.fps_window.count() >= 120
                    && self.upgrade_cooldown == 0
                    && self.gear_pingpong_cooldown == 0
                    && !self.downgrade_boost_active
                    && self.probe_fail_count < 3
                    && self.gear_pingpong_count < 2
                {
                    self.upgrade_confirm_frames += 1;
                    self.downgrade_confirm_frames = 0;
                    let stable_confirm_needed = ((120.0 * gear_scale).max(45.0)) as u32;
                    if self.upgrade_confirm_frames >= stable_confirm_needed {
                        log::info!("FAS: stable-upgrade {:.0}->{:.0}fps \
                            (avg={:.1} P={:.2} {}f confirm)",
                            self.current_target_fps, target, avg_fps,
                            self.perf_index, self.upgrade_confirm_frames);
                        self.last_upgrade_to_gear = target;
                        self.last_probe_gear = target;
                        self.current_target_fps = target;
                        self.upgrade_confirm_frames = 0;
                        self.ema_actual_ms = 0.0;
                        self.pid.reset();
                        self.gain_scheduler.reset();
                        self.fps_window.clear();
                        self.perf_index = (self.perf_index + 0.15).min(0.65);
                        self.gear_change_dampen_frames = ((90.0 * gear_scale).max(30.0)) as u32;
                        self.consecutive_downgrade_count = 0;
                        self.stable_gear_frames = 0;
                        self.downgrade_boost_active = false;
                        self.downgrade_boost_remaining = 0;
                    }
                } else {
                    let probe_avg_threshold = if self.consecutive_downgrade_count >= 2 {
                        target - 10.0
                    } else {
                        target * 0.9
                    };
                    if avg_fps >= probe_avg_threshold
                        && self.perf_index < 0.50
                        && self.upgrade_cooldown == 0
                        && self.gear_pingpong_cooldown == 0
                        && self.last_downgrade_perf < 0.60
                        && self.probe_fail_count < 3
                    {
                        self.upgrade_confirm_frames += 1;
                        let probe_confirm_needed = ((90.0 * gear_scale).max(30.0)) as u32;
                        if self.upgrade_confirm_frames >= probe_confirm_needed {
                            log::info!("FAS: probe {:.0}->{:.0}fps (avg={:.1} P={:.2})",
                                self.current_target_fps, target, avg_fps, self.perf_index);
                            self.last_upgrade_to_gear = target;
                            self.last_probe_gear = target;
                            self.current_target_fps = target;
                            self.upgrade_confirm_frames = 0;
                            self.ema_actual_ms = 0.0;
                            self.pid.reset();
                            self.gain_scheduler.reset();
                            self.fps_window.clear();
                            self.perf_index = (self.perf_index + 0.20).min(0.60);
                            self.gear_change_dampen_frames = ((90.0 * gear_scale).max(30.0)) as u32;
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
                } else if self.scene_transition_guard > 0 {
                    self.downgrade_confirm_frames = 0;
                    self.downgrade_boost_active = false;
                    self.downgrade_boost_remaining = 0;
                } else if avg_fps < self.current_target_fps - 10.0 {
                    if recent30_for_dg >= self.current_target_fps - 5.0 {
                        if self.downgrade_boost_active {
                            log::info!("FAS: boost cancelled (recent30:{:.1} healthy)",
                                recent30_for_dg);
                            self.downgrade_boost_active = false;
                            self.downgrade_boost_remaining = 0;
                        }
                        self.downgrade_confirm_frames = 0;
                    } else if !self.downgrade_boost_active
                        && self.downgrade_confirm_frames == 0
                    {
                        self.downgrade_boost_active = true;
                        self.downgrade_boost_remaining = self.cfg.downgrade_boost_duration;
                        self.downgrade_boost_perf_saved = self.perf_index;
                        self.perf_index =
                            (self.perf_index + self.cfg.downgrade_boost_perf_inc).min(0.90);
                        log::info!("FAS: boost start | avg:{:.1} | P:{:.2}->{:.2}",
                            avg_fps, self.downgrade_boost_perf_saved, self.perf_index);
                    } else if self.downgrade_boost_active
                        && self.downgrade_boost_remaining > 0
                    {
                        self.downgrade_boost_remaining -= 1;
                        if self.downgrade_boost_remaining == 0 {
                            let restored_perf = self.downgrade_boost_perf_saved
                                .min(self.perf_index);
                            self.perf_index = restored_perf;
                            self.downgrade_boost_active = false;
                            self.downgrade_confirm_frames = 1;
                            log::info!("FAS: boost failed | P restored->{:.2}", self.perf_index);
                        }
                    } else {
                        self.downgrade_confirm_frames += 1;
                        if self.downgrade_confirm_frames >= self.cfg.downgrade_confirm_frames {
                            let old_fps = self.current_target_fps;

                            if (old_fps - self.last_upgrade_to_gear).abs() < 1.0
                                && self.last_upgrade_to_gear > 0.0
                            {
                                self.gear_pingpong_count += 1;
                                let backoff_pp = 1u32 << self.gear_pingpong_count.min(4);
                                self.gear_pingpong_cooldown = 300 * backoff_pp;
                                log::info!("FAS: pingpong detected {:.0}<->{:.0} (count={}, cd={})",
                                    target, old_fps, self.gear_pingpong_count,
                                    self.gear_pingpong_cooldown);

                                if self.gear_pingpong_count >= 2 {
                                    self.probe_fail_count = self.probe_fail_count.max(3);
                                    log::info!("FAS: pingpong cap: {:.0}fps marked unattainable (fail={})",
                                        old_fps, self.probe_fail_count);
                                }
                            } else {
                                self.gear_pingpong_count = 0;
                            }
                            self.last_upgrade_to_gear = 0.0;

                            self.current_target_fps = target;
                            self.downgrade_confirm_frames = 0;
                            self.downgrade_boost_active = false;
                            self.downgrade_boost_remaining = 0;
                            self.ema_actual_ms = 0.0;
                            self.pid.reset();
                            self.gain_scheduler.reset();
                            self.fps_window.clear();
                            self.upgrade_confirm_frames = 0;
                            self.gear_change_dampen_frames = 60;

                            if (old_fps - self.last_downgrade_from_fps).abs() < 1.0 {
                                self.consecutive_downgrade_count += 1;
                            } else {
                                self.consecutive_downgrade_count = 1;
                            }
                            self.last_downgrade_from_fps = old_fps;
                            self.last_downgrade_perf = self.perf_index;

                            if (old_fps - self.last_probe_gear).abs() < 1.0 {
                                self.probe_fail_count += 1;
                            } else {
                                self.probe_fail_count = 0;
                            }

                            let backoff = 1u32 << self.consecutive_downgrade_count.min(4);
                            self.upgrade_cooldown =
                                self.cfg.upgrade_cooldown_after_downgrade * backoff;
                            log::info!("FAS: {:.0}->{:.0}fps (avg={:.1}) P={:.2} cd={}",
                                old_fps, target, avg_fps, self.perf_index,
                                self.upgrade_cooldown);
                            self.stable_gear_frames = 0;
                        }
                    }
                } else {
                    if self.downgrade_boost_active {
                        log::info!("FAS: boost succeeded | avg:{:.1} recovered", avg_fps);
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
                if self.stable_gear_frames >= self.cfg.stable_forgive_frames {
                    let old_consec = self.consecutive_downgrade_count;
                    self.consecutive_downgrade_count =
                        self.consecutive_downgrade_count.saturating_sub(1);

                    if self.gear_pingpong_cooldown == 0 {
                        self.gear_pingpong_count =
                            self.gear_pingpong_count.saturating_sub(1);
                    }
                    log::info!("FAS: stable forgive | consec:{} -> {} pp:{} pp-cd:{}",
                        old_consec, self.consecutive_downgrade_count,
                        self.gear_pingpong_count, self.gear_pingpong_cooldown);
                    self.stable_gear_frames = 0;
                }
            }
        }

        let ema_budget = 1000.0 / (self.current_target_fps - self.fps_margin).max(1.0);
        let inst_budget = 1000.0 / self.current_target_fps;
        let ema_input_ms = {
            let extreme_threshold = inst_budget * 5.0;
            let spike_cap = inst_budget * 2.0;

            if actual_ms > extreme_threshold {
                // 极端 spike: 只给 budget+1ms 的微量影响，避免 EMA 跳变
                let dampened = inst_budget + 1.0;
                log::debug!("FAS: extreme spike {:.1}ms -> {:.1}ms (threshold={:.1})",
                    actual_ms, dampened, extreme_threshold);
                dampened
            } else if actual_ms > spike_cap {
                log::debug!("FAS: spike filter {:.1}ms -> {:.1}ms (cap={:.1})",
                    actual_ms, spike_cap, spike_cap);
                spike_cap
            } else {
                actual_ms
            }
        };
        if self.ema_actual_ms <= 0.0 {
            self.ema_actual_ms = ema_input_ms;
        } else {
            let a_down = if self.jank_cooldown == 0 && self.post_jank_no_decay_frames > 0 {
                if self.current_target_fps > 120.0 { 0.40 } else { 0.30 }
            } else {
                if self.current_target_fps > 120.0 { 0.30 } else { 0.20 }
            };
            let a = if ema_input_ms > self.ema_actual_ms { 0.25 } else { a_down };
            self.ema_actual_ms = self.ema_actual_ms * (1.0 - a) + ema_input_ms * a;
        }

        let ema_err = ema_budget - self.ema_actual_ms;   // 正=有余量，负=掉帧
        let inst_err = inst_budget - actual_ms;
        let old_perf = self.perf_index;
        let damped = self.gear_change_dampen_frames > 0;
        let in_scene_transition = self.scene_transition_guard > 0;

        // 帧率归一化系数：以60fps为基准
        let fps_norm = (60.0 / self.current_target_fps.max(1.0)).sqrt();

        let budget_ratio = (60.0 / self.current_target_fps.max(1.0)).sqrt().min(1.0);
        let scaled_instant_err_threshold = self.cfg.instant_error_threshold_ms * budget_ratio;

        // ── PID 计算核心输出 ──
        // 进化 2：自适应增益同步（每帧将调度器的乘子应用到 PID）
        // 高帧率下限制 Kd 乘子上限，避免过度阻尼导致 perf 无法回落
        if self.current_target_fps > 90.0 {
            let kd_cap = 1.0 + (90.0 / self.current_target_fps.max(1.0)); // 144fps → cap=1.625
            if self.gain_scheduler.kd_mult > kd_cap {
                self.gain_scheduler.kd_mult = kd_cap;
            }
            if self.gain_scheduler.kp_mult < 0.85 {
                self.gain_scheduler.kp_mult = 0.85;
            }
        }
        self.gain_scheduler.apply_to_pid(&mut self.pid);

        // pid_output > 0 → 有余量，应降低 perf
        // pid_output < 0 → 掉帧，应提升 perf
        let raw_pid = self.pid.compute(ema_err, inst_err, fps_norm);

        // ── 将 PID 输出转换为 perf_index 增量 ──
        // 注意 sign 反转：PID 正值（余量）→ 减少 perf，PID 负值（deficit）→ 增加 perf
        let act;

        // 先处理极端情况（jank 保护），PID 的线性响应不够快
        // 高帧率下 crit/heavy 增量额外衰减
        // 原因：144fps budget=6.94ms，1ms 抖动=14% 误差，但不代表需要大幅升频
        // 用 fps_norm² 让 144fps 下的增量更小（fps_norm=0.645 → fps_norm²=0.416）
        let jank_scale = if self.current_target_fps > 120.0 {
            fps_norm * fps_norm * fps_norm
        } else if self.current_target_fps > 90.0 {
            fps_norm * fps_norm
        } else {
            fps_norm
        };

        if inst_err < -scaled_instant_err_threshold {
            self.jank_streak += 1;
            let streak_mult = if self.current_target_fps > 120.0 {
                match self.jank_streak {
                    1 => 0.20,
                    2 => 0.50,
                    _ => 1.0,
                }
            } else {
                if self.jank_streak >= 2 { 1.0 } else { 0.33 }
            };
            let base_inc = if self.current_target_fps > 120.0 {
                if damped { 0.020 } else { 0.035 }
            } else {
                if damped { 0.03 } else { 0.055 }
            };
            self.perf_index += base_inc * jank_scale * streak_mult;
            act = if damped { "crit-d" } else { "crit" };
            self.consecutive_normal_frames = 0;
            self.jank_cooldown = self.cfg.jank_cooldown_frames_crit;
            self.post_jank_no_decay_frames = self.cfg.fast_decay_post_jank_suppress;
        } else if ema_err < -(ema_budget * 0.15).clamp(2.0, 4.0) {
            // 严重掉帧
            self.jank_streak += 1;
            let streak_mult = if self.current_target_fps > 120.0 {
                match self.jank_streak {
                    1 => 0.20,
                    2 => 0.50,
                    _ => 1.0,
                }
            } else {
                if self.jank_streak >= 2 { 1.0 } else { 0.33 }
            };
            let base_inc = if self.current_target_fps > 120.0 {
                if damped { 0.008 } else { 0.015 }
            } else {
                if damped { 0.012 } else { 0.025 }
            };
            self.perf_index += base_inc * jank_scale * streak_mult;
            act = if damped { "heavy-d" } else { "heavy" };
            self.consecutive_normal_frames = 0;
            self.jank_cooldown = self.jank_cooldown.max(self.cfg.jank_cooldown_frames_heavy);
            self.post_jank_no_decay_frames = self.cfg.fast_decay_post_jank_suppress / 2;
        } else {
            // 正常帧重置 jank streak
            self.jank_streak = 0;

            // ── 正常区间：完全由 PID 驱动 ──
            self.consecutive_normal_frames += 1;

            let in_jank_cooldown = self.jank_cooldown > 0;
            let in_downgrade_boost =
                self.downgrade_boost_active && self.downgrade_boost_remaining > 0;

            // 场景过渡 & jank 冷却期间衰减 PID 响应
            let scene_damp = if in_scene_transition { 0.5 } else { 1.0 };
            let jank_damp = if in_jank_cooldown { 0.4 } else { 1.0 };
            let boost_damp = if in_downgrade_boost { 0.0 } else { 1.0 };

            // 接近降档线时抑制衰减
            let proximity_threshold = self.current_target_fps * self.cfg.downgrade_proximity_ratio;
            let near_downgrade = avg_fps < proximity_threshold && avg_fps > 1.0;
            let proximity_damp = if near_downgrade { 0.2 } else { 1.0 };

            let composite_damp = scene_damp * jank_damp * boost_damp * proximity_damp;
            let stability_damp = if self.current_target_fps > 90.0
                && self.fps_window.count() >= 30
                && self.perf_index > self.dynamic_perf_floor + 0.05
                && self.perf_index < 0.85
            {
                let fps_deviation = ((avg_fps - self.current_target_fps).abs()
                    / self.current_target_fps).min(0.10);
                if fps_deviation < 0.05 {
                    // avg 在 target±5%: 强阻尼 0.35
                    0.35
                } else if fps_deviation < 0.08 {
                    // avg 在 target±8%: 中阻尼 0.55
                    0.55
                } else {
                    1.0
                }
            } else {
                1.0
            };

            // PID 输出应用（注意 sign：raw_pid 正=有余量，我们要减少 perf）
            // 修复：fps_norm 对称应用于增减两个方向，避免高帧率系统性偏高
            if raw_pid > 0.0 {
                // 有余量 → 降低 perf（省电）
                let delta = raw_pid * composite_damp * stability_damp * fps_norm;
                self.perf_index -= delta;
                act = if in_scene_transition { "pid-decay-s" } else { "pid-decay" };
            } else {
                // deficit → 提升 perf
                // damped 模式下削弱增幅
                let damp_mult = if damped { 0.5 } else { 1.0 };
                let delta = (-raw_pid) * composite_damp * stability_damp * damp_mult * fps_norm;
                self.perf_index += delta;
                act = if in_scene_transition { "pid-inc-s" } else { "pid-inc" };
            }
        }

        // ── fast_decay（连续正常帧后加速降频） ──
        {
            let init_elapsed_ms = self.init_time.elapsed().as_millis();
            let in_jank_cooldown = self.jank_cooldown > 0;
            let near_downgrade = avg_fps
                < self.current_target_fps * self.cfg.downgrade_proximity_ratio
                && avg_fps > 1.0;
            let in_downgrade_boost =
                self.downgrade_boost_active && self.downgrade_boost_remaining > 0;

            // 高帧率时降低触发阈值：144fps 下约 25 帧（≈0.17s）即可触发
            let scaled_threshold = (self.cfg.fast_decay_frame_threshold as f32 * fps_norm)
                .max(20.0) as u32;

            let adaptive_floor = self.dynamic_perf_floor;

            if self.consecutive_normal_frames >= scaled_threshold
                && self.perf_index > self.cfg.fast_decay_perf_threshold.max(adaptive_floor + 0.10)
                && !in_scene_transition && !in_jank_cooldown
                && !near_downgrade && !in_downgrade_boost
                && self.post_jank_no_decay_frames == 0
                && init_elapsed_ms > self.cfg.cold_boot_ms as u128
            {
                // fast_decay 步长不再乘 fps_norm：PID 已平衡，此处无需再削弱
                let step = ((self.perf_index - 0.50) / 0.50 * self.cfg.fast_decay_max_step)
                    .clamp(self.cfg.fast_decay_min_step, self.cfg.fast_decay_max_step);
                self.perf_index -= step;
                log::debug!("FAS: fast_decay -{:.4} after {}f (P:{:.2}->{:.2})",
                    step, self.consecutive_normal_frames,
                    self.perf_index + step, self.perf_index);
                self.consecutive_normal_frames = 0;
            }

            if self.current_target_fps > 90.0
                && avg_fps > self.current_target_fps + self.fps_margin * 1.5
                && self.perf_index > self.dynamic_perf_floor
                && !in_scene_transition && !in_jank_cooldown
                && self.fps_window.count() >= 30
            {
                let surplus_ratio = (avg_fps - self.current_target_fps) / self.current_target_fps;
                // surplus_decay 更积极
                // 系数从 0.008 增到 0.015，上限从 0.003 增到 0.005
                let surplus_decay = (surplus_ratio * 0.015).clamp(0.0, 0.005);
                if surplus_decay > 0.0005 {
                    self.perf_index -= surplus_decay;
                    log::debug!("FAS: surplus_decay -{:.4} (avg:{:.1} target:{:.0})",
                        surplus_decay, avg_fps, self.current_target_fps);
                }
            }
        }

        // ── clamp & 限速 ──
        // 进化 2：自适应增益观测（在 clamp 前喂入当前帧观测数据）
        // 同步帧率归一化系数给自适应调度器
        self.gain_scheduler.fps_norm = fps_norm;
        self.gain_scheduler.observe(inst_err, self.perf_index);

        if avg_fps > self.current_target_fps * 0.95
            && avg_fps < self.current_target_fps * 1.15
            && self.fps_window.count() >= 30
        {
            // 基准 floor：0.38
            let base_floor = 0.38_f32;
            // 如果 avg 持续超出 target，按超出比例降低 floor
            // 例: avg=127 target=120 → surplus_ratio=0.058 → reduction≈0.07
            let surplus_ratio = ((avg_fps - self.current_target_fps) / self.current_target_fps)
                .max(0.0);
            // 只有当 perf 接近 floor（余量 < 0.05）且帧率偏高时才降 floor
            let near_floor = self.perf_index < self.dynamic_perf_floor + 0.05;
            let reduction = if near_floor && surplus_ratio > 0.03 {
                // floor 最低可达 0.18（原来 0.23），给 PID 更多调节空间
                let target_reduction = (surplus_ratio * 1.5).min(0.20);
                let current_reduction = (base_floor - self.dynamic_perf_floor).max(0.0);
                let step = if self.current_target_fps > 90.0 { 0.004_f32 } else { 0.002_f32 };
                if current_reduction < target_reduction {
                    (current_reduction + step).min(target_reduction)
                } else {
                    current_reduction
                }
            } else if surplus_ratio <= 0.02 {
                // surplus 消失，floor 逐步恢复
                let current_reduction = (base_floor - self.dynamic_perf_floor).max(0.0);
                (current_reduction - 0.002).max(0.0)
            } else {
                (base_floor - self.dynamic_perf_floor).max(0.0)
            };
            self.dynamic_perf_floor = (base_floor - reduction).max(self.cfg.perf_floor);
        } else {
            // fps 不在 target 附近时，恢复到配置 floor
            self.dynamic_perf_floor = self.cfg.perf_floor;
        };

        self.perf_index = self.perf_index.clamp(self.dynamic_perf_floor, self.cfg.perf_ceil);
        let fps_inc_scale = if self.current_target_fps > 120.0 {
            (60.0 / self.current_target_fps).sqrt() * 0.9
        } else if self.current_target_fps > 90.0 {
            (60.0 / self.current_target_fps).sqrt()
        } else {
            1.0
        };
        let max_inc = if damped {
            self.cfg.max_inc_damped * fps_inc_scale
        } else {
            self.cfg.max_inc_normal * fps_inc_scale
        };
        if self.perf_index > old_perf + max_inc {
            self.perf_index = old_perf + max_inc;
        }
        if damped && self.perf_index > self.cfg.damped_perf_cap {
            self.perf_index = self.cfg.damped_perf_cap;
        }

        if self.is_in_soft_loading {
            let cap = if self.soft_loading_probe_countdown > 0 {
                self.cfg.soft_loading_probe_perf_cap
            } else {
                self.cfg.soft_loading_perf_cap
            };
            if self.perf_index > cap {
                self.perf_index = cap;
            }
        }

        if !self.is_in_loading_state && !self.is_in_soft_loading
            && avg_fps < self.current_target_fps * 0.35
            && self.fps_window.count() >= 8
        {
            let extreme_gap_cap = 0.75;
            if self.perf_index > extreme_gap_cap {
                self.perf_index = extreme_gap_cap;
                log::debug!("FAS: extreme gap perf cap {:.2} (avg={:.1} target={:.0})",
                    extreme_gap_cap, avg_fps, self.current_target_fps);
            }
        }

        // ── 心跳日志（每30帧） ──
        self.log_counter = self.log_counter.wrapping_add(1);
        if self.log_counter % 30 == 0 {
            log::info!("FAS | {:.0}fps avg:{:.1} | {:.2}ms ema:{:.2} | \
                err:{:+.2}/{:+.2} | {} | P:{:.3} F:{:.2}{}{}{}{}{}{}{}{}",
                self.current_target_fps, avg_fps, actual_ms, self.ema_actual_ms,
                ema_err, inst_err,
                act, self.perf_index,
                self.dynamic_perf_floor,
                if self.upgrade_cooldown > 0 {
                    format!(" cd:{}", self.upgrade_cooldown)
                } else { String::new() },
                if damped {
                    format!(" damp:{}", self.gear_change_dampen_frames)
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
                } else { String::new() },
                // 进化 2：自适应 PID 乘子日志
                if self.gain_scheduler.is_active() {
                    format!(" [adp Kp×{:.2} Ki×{:.2} Kd×{:.2}]",
                        self.gain_scheduler.kp_mult,
                        self.gain_scheduler.ki_mult,
                        self.gain_scheduler.kd_mult)
                } else { String::new() },
                if self.gear_pingpong_cooldown > 0 {
                    format!(" [pp-cd:{} os:{}]", self.gear_pingpong_cooldown,
                        self.pp_overshoot_streak)
                } else { String::new() });

            // ── 频率 mismatch 检测 & 补偿（逻辑与原版完全一致） ──
            if self.mismatch_probe_skip > 0 {
                self.mismatch_probe_skip -= 1;
            } else {
                let mut needs_reapply = false;
                let mut mismatch_found_this_cycle = false;
                let mut worst_ratio: f32 = 1.0;
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
                                    mismatch_found_this_cycle = true;
                                    let ratio = actual_freq as f32
                                        / p.current_freq.max(1) as f32;
                                    if ratio < worst_ratio { worst_ratio = ratio; }
                                    p.mismatch_count += 1;
                                    if p.mismatch_count >= self.cfg.mismatch_lock_threshold {
                                        p.external_lock_cooldown = 300;
                                        p.mismatch_count = 0;
                                        log::warn!("FAS[P{}] externally locked: \
                                            yielding 300f (actual={} MHz)",
                                            p.policy_id, actual_freq / 1000);
                                    } else {
                                        log::warn!("FAS[P{}] mismatch: set={} actual={} MHz [{}/{}]",
                                            p.policy_id, p.current_freq / 1000,
                                            actual_freq / 1000,
                                            p.mismatch_count, self.cfg.mismatch_lock_threshold);
                                        needs_reapply = true;
                                    }
                                } else {
                                    p.mismatch_count = p.mismatch_count.saturating_sub(1);
                                }
                            }
                        }
                    }
                }

                if mismatch_found_this_cycle {
                    self.mismatch_consecutive_cycles += 1;
                    let needed = ((1.0 / worst_ratio.max(0.3)) - 1.0).clamp(0.0, 0.35);
                    let alpha = if self.mismatch_consecutive_cycles >= 3 { 0.4 } else { 0.2 };
                    self.mismatch_compensation =
                        self.mismatch_compensation * (1.0 - alpha) + needed * alpha;
                } else {
                    self.mismatch_consecutive_cycles = 0;
                    self.mismatch_compensation *= 0.85;
                    if self.mismatch_compensation < 0.005 {
                        self.mismatch_compensation = 0.0;
                    }
                }

                if needs_reapply {
                    log::info!("FAS: mismatch, force reapply unlocked policies");
                    for p in self.policies.iter_mut() {
                        if p.external_lock_cooldown == 0 {
                            p.force_reapply();
                        }
                    }
                    self.mismatch_probe_skip = self.cfg.mismatch_reapply_skip_cycles;
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