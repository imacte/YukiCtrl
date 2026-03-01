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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use serde::de::DeserializeOwned;
use log::warn;
use std::path::PathBuf;
use crate::common;

pub fn get_rules_path() -> PathBuf {
    common::get_module_root().join("rules.yaml")
}

pub fn get_boot_scripts_path() -> PathBuf {
    common::get_module_root().join("boot_scripts.yaml")
}

pub fn get_scripts_dir() -> PathBuf {
    common::get_module_root().join("scripts")
}

/// PID 控制器的三个核心参数。
/// - `kp`：比例项（Proportional）— 响应瞬时误差（inst_err）
/// - `ki`：积分项（Integral）    — 修正长期趋势偏差（ema_err 的累积）
/// - `kd`：微分项（Derivative）  — 阻尼 / 防过冲（ema_err 的变化率）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PidCoefficients {
    #[serde(default = "default_kp")]
    pub kp: f32,
    #[serde(default = "default_ki")]
    pub ki: f32,
    #[serde(default = "default_kd")]
    pub kd: f32,
}

fn default_kp() -> f32 { 0.045 }
fn default_ki() -> f32 { 0.012 }
fn default_kd() -> f32 { 0.008 }

impl Default for PidCoefficients {
    fn default() -> Self {
        Self {
            kp: default_kp(),
            ki: default_ki(),
            kd: default_kd(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClusterProfile {
    /// IPC 容量权重 / 频率曲率指数（小核=1.0 为基准）
    #[serde(default = "default_capacity_weight")]
    pub capacity_weight: f32,
}

fn default_capacity_weight() -> f32 { 1.0 }

impl Default for ClusterProfile {
    fn default() -> Self {
        Self {
            capacity_weight: 1.0,
        }
    }
}

/// 预设的集群配置模板。
/// 索引 0=小核, 1=中核, 2=大核, 3=超大核。
pub fn default_cluster_profiles() -> Vec<ClusterProfile> {
    vec![
        // 小核 (e.g. Cortex-A510): 线性跟随
        ClusterProfile { capacity_weight: 1.0 },
        // 中核 (e.g. Cortex-A715): 略保守
        ClusterProfile { capacity_weight: 1.5 },
        // 大核 (e.g. Cortex-A720): 明显保守
        ClusterProfile { capacity_weight: 2.5 },
        // 超大核 (e.g. Cortex-X4): 极度保守
        ClusterProfile { capacity_weight: 3.5 },
    ]
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdaptivePidConfig {
    /// 是否启用自适应增益调度
    #[serde(default = "default_adaptive_pid_enabled")]
    pub enabled: bool,

    /// 每隔多少帧执行一次增益评估
    #[serde(default = "default_adaptive_eval_interval")]
    pub eval_interval: u32,

    // ── deficit streak → boost Kp ──

    /// 连续 deficit 帧达到此阈值时触发 Kp 提升
    #[serde(default = "default_deficit_streak_threshold")]
    pub deficit_streak_threshold: u32,

    /// deficit 触发时 Kp 乘子的增量（叠加到 kp_gain_mult 上）
    #[serde(default = "default_kp_boost_step")]
    pub kp_boost_step: f32,

    /// surplus 恢复时 Kp 乘子每次评估的衰减步长
    #[serde(default = "default_kp_decay_step")]
    pub kp_decay_step: f32,

    // ── oscillation → boost Kd, reduce Kp ──

    /// 频率翻转（升→降→升）次数达到此阈值视为振荡
    #[serde(default = "default_oscillation_threshold")]
    pub oscillation_threshold: u32,

    /// 振荡检测窗口帧数
    #[serde(default = "default_oscillation_window")]
    pub oscillation_window: u32,

    /// 振荡时 Kd 乘子增量
    #[serde(default = "default_kd_boost_step")]
    pub kd_boost_step: f32,

    /// 振荡时 Kp 乘子削减步长
    #[serde(default = "default_kp_osc_reduce_step")]
    pub kp_osc_reduce_step: f32,

    /// 无振荡时 Kd 乘子衰减步长
    #[serde(default = "default_kd_decay_step")]
    pub kd_decay_step: f32,

    // ── 乘子边界 ──

    /// 增益乘子下限
    #[serde(default = "default_min_gain_mult")]
    pub min_gain_mult: f32,

    /// 增益乘子上限
    #[serde(default = "default_max_gain_mult")]
    pub max_gain_mult: f32,
}

fn default_adaptive_pid_enabled() -> bool { true }
fn default_adaptive_eval_interval() -> u32 { 60 }
fn default_deficit_streak_threshold() -> u32 { 30 }
fn default_kp_boost_step() -> f32 { 0.15 }
fn default_kp_decay_step() -> f32 { 0.03 }
fn default_oscillation_threshold() -> u32 { 6 }
fn default_oscillation_window() -> u32 { 60 }
fn default_kd_boost_step() -> f32 { 0.20 }
fn default_kp_osc_reduce_step() -> f32 { 0.08 }
fn default_kd_decay_step() -> f32 { 0.05 }
fn default_min_gain_mult() -> f32 { 0.5 }
fn default_max_gain_mult() -> f32 { 2.5 }

impl Default for AdaptivePidConfig {
    fn default() -> Self {
        Self {
            enabled: default_adaptive_pid_enabled(),
            eval_interval: default_adaptive_eval_interval(),
            deficit_streak_threshold: default_deficit_streak_threshold(),
            kp_boost_step: default_kp_boost_step(),
            kp_decay_step: default_kp_decay_step(),
            oscillation_threshold: default_oscillation_threshold(),
            oscillation_window: default_oscillation_window(),
            kd_boost_step: default_kd_boost_step(),
            kp_osc_reduce_step: default_kp_osc_reduce_step(),
            kd_decay_step: default_kd_decay_step(),
            min_gain_mult: default_min_gain_mult(),
            max_gain_mult: default_max_gain_mult(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FasRulesConfig {
    // ── 基础 ──
    #[serde(default = "default_fps_gears")]
    pub fps_gears: Vec<f32>,
    #[serde(default = "default_fps_margin")]
    pub fps_margin: String,

    // ── PID 控制器 ──
    #[serde(default)]
    pub pid: PidCoefficients,

    // ── 集群配置 ──
    #[serde(default = "default_cluster_profiles")]
    pub cluster_profiles: Vec<ClusterProfile>,

    /// 是否自动从内核 cpu_capacity 节点探测算力分布并计算 capacity_weight。
    /// 开启后忽略 cluster_profiles 中的手动 weight 值。
    #[serde(default = "default_auto_capacity_weight")]
    pub auto_capacity_weight: bool,

    // ── 自适应 PID ──
    #[serde(default)]
    pub adaptive_pid: AdaptivePidConfig,

    // ── 归一化 perf 范围（逻辑上 0.0~1.0，这里是初始值/下限/上限） ──
    /// perf 归一化下限（不再是 150.0 这种绝对值，而是 0.0~1.0）
    #[serde(default = "default_perf_floor_norm")]
    pub perf_floor: f32,
    /// perf 归一化上限
    #[serde(default = "default_perf_ceil_norm")]
    pub perf_ceil: f32,
    /// 控制器初始化时的 perf
    #[serde(default = "default_perf_init")]
    pub perf_init: f32,
    /// 冷启动期间的 perf
    #[serde(default = "default_perf_cold_boot")]
    pub perf_cold_boot: f32,

    // ── 频率迟滞 ──
    #[serde(default = "default_hysteresis")]
    pub freq_hysteresis: f32,

    // ── 重帧 / 硬加载 ──
    #[serde(default = "default_heavy_frame_ms")]
    pub heavy_frame_threshold_ms: f32,
    #[serde(default = "default_loading_cumulative_ms")]
    pub loading_cumulative_ms: f32,
    #[serde(default = "default_loading_normal_tolerance")]
    pub loading_normal_tolerance: u32,
    #[serde(default = "default_loading_perf_floor")]
    pub loading_perf_floor: f32,
    #[serde(default = "default_loading_perf_ceiling")]
    pub loading_perf_ceiling: f32,
    #[serde(default = "default_loading_reentry_cooldown")]
    pub loading_reentry_cooldown: u32,

    // ── 加载退出后 ──
    #[serde(default = "default_post_loading_ignore")]
    pub post_loading_ignore_frames: u32,
    #[serde(default = "default_post_loading_perf_min")]
    pub post_loading_perf_min: f32,
    #[serde(default = "default_post_loading_perf_max")]
    pub post_loading_perf_max: f32,
    #[serde(default = "default_post_loading_downgrade_guard")]
    pub post_loading_downgrade_guard: u32,

    // ── 持续加载 ──
    #[serde(default = "default_sustained_loading_cycle_threshold")]
    pub sustained_loading_cycle_threshold: u32,
    #[serde(default = "default_sustained_loading_window_ns")]
    pub sustained_loading_window_ns: u64,
    #[serde(default = "default_sustained_post_loading_ignore")]
    pub sustained_post_loading_ignore: u32,

    // ── 瞬时误差 ──
    #[serde(default = "default_instant_error_threshold")]
    pub instant_error_threshold_ms: f32,

    // ── 软加载 ──
    #[serde(default = "default_soft_loading_fps_ratio")]
    pub soft_loading_fps_ratio: f32,
    #[serde(default = "default_soft_loading_perf_threshold")]
    pub soft_loading_perf_threshold: f32,
    #[serde(default = "default_soft_loading_confirm_frames")]
    pub soft_loading_confirm_frames: u32,
    #[serde(default = "default_soft_loading_perf_cap")]
    pub soft_loading_perf_cap: f32,
    #[serde(default = "default_soft_loading_exit_frames")]
    pub soft_loading_exit_frames: u32,
    #[serde(default = "default_soft_loading_breakthrough_fps_ratio")]
    pub soft_loading_breakthrough_fps_ratio: f32,
    #[serde(default = "default_soft_loading_breakthrough_window")]
    pub soft_loading_breakthrough_window: usize,
    #[serde(default = "default_soft_loading_exit_frames_breakthrough")]
    pub soft_loading_exit_frames_breakthrough: u32,
    #[serde(default = "default_soft_loading_probe_interval")]
    pub soft_loading_probe_interval: u32,
    #[serde(default = "default_soft_loading_probe_duration")]
    pub soft_loading_probe_duration: u32,
    #[serde(default = "default_soft_loading_probe_perf_cap")]
    pub soft_loading_probe_perf_cap: f32,
    #[serde(default = "default_soft_loading_probe_fps_gain_ratio")]
    pub soft_loading_probe_fps_gain_ratio: f32,
    #[serde(default = "default_soft_loading_probe_fail_decay_step")]
    pub soft_loading_probe_fail_decay_step: f32,
    #[serde(default = "default_soft_loading_downgrade_check_frames")]
    pub soft_loading_downgrade_check_frames: u32,
    #[serde(default = "default_soft_loading_gear_match_tolerance")]
    pub soft_loading_gear_match_tolerance: f32,

    // ── 应用切换 ──
    #[serde(default = "default_app_switch_gap_ms")]
    pub app_switch_gap_ms: f32,
    #[serde(default = "default_app_switch_resume_perf")]
    pub app_switch_resume_perf: f32,
    #[serde(default = "default_app_switch_ignore_frames")]
    pub app_switch_ignore_frames: u32,

    // ── 场景过渡 ──
    #[serde(default = "default_scene_transition_cv_threshold")]
    pub scene_transition_cv_threshold: f32,
    #[serde(default = "default_scene_transition_guard_frames")]
    pub scene_transition_guard_frames: u32,
    #[serde(default = "default_scene_transition_max_continuous")]
    pub scene_transition_max_continuous: u32,
    #[serde(default = "default_scene_transition_fps_floor_ratio")]
    pub scene_transition_fps_floor_ratio: f32,
    #[serde(default = "default_scene_transition_force_exit_frames")]
    pub scene_transition_force_exit_frames: u32,

    // ── Jank 冷却 ──
    #[serde(default = "default_jank_cooldown_crit")]
    pub jank_cooldown_frames_crit: u32,
    #[serde(default = "default_jank_cooldown_heavy")]
    pub jank_cooldown_frames_heavy: u32,

    // ── 降档 ──
    #[serde(default = "default_downgrade_confirm_frames")]
    pub downgrade_confirm_frames: u32,
    #[serde(default = "default_downgrade_boost_perf_inc")]
    pub downgrade_boost_perf_inc: f32,
    #[serde(default = "default_downgrade_boost_duration")]
    pub downgrade_boost_duration: u32,
    #[serde(default = "default_downgrade_proximity_ratio")]
    pub downgrade_proximity_ratio: f32,
    #[serde(default = "default_upgrade_cooldown_after_downgrade")]
    pub upgrade_cooldown_after_downgrade: u32,
    #[serde(default = "default_stable_forgive_frames")]
    pub stable_forgive_frames: u32,

    // ── 快速衰减 ──
    #[serde(default = "default_fast_decay_frame_threshold")]
    pub fast_decay_frame_threshold: u32,
    #[serde(default = "default_fast_decay_perf_threshold")]
    pub fast_decay_perf_threshold: f32,
    #[serde(default = "default_fast_decay_max_step")]
    pub fast_decay_max_step: f32,
    #[serde(default = "default_fast_decay_min_step")]
    pub fast_decay_min_step: f32,
    #[serde(default = "default_fast_decay_post_jank_suppress")]
    pub fast_decay_post_jank_suppress: u32,

    // ── deficit 抑制 ──
    #[serde(default = "default_deficit_suppress_ms")]
    pub deficit_suppress_ms: f32,

    // ── Mismatch ──
    #[serde(default = "default_mismatch_lock_threshold")]
    pub mismatch_lock_threshold: u32,
    #[serde(default = "default_mismatch_reapply_skip_cycles")]
    pub mismatch_reapply_skip_cycles: u32,

    // ── PID 输出限幅（归一化） ──
    /// 单帧最大 perf 增量（damped 模式）
    #[serde(default = "default_max_inc_damped")]
    pub max_inc_damped: f32,
    /// 单帧最大 perf 增量（正常模式）
    #[serde(default = "default_max_inc_normal")]
    pub max_inc_normal: f32,
    /// damped 模式下 perf 天花板
    #[serde(default = "default_damped_perf_cap")]
    pub damped_perf_cap: f32,

    // ── 频率重应用 ──
    #[serde(default = "default_freq_force_reapply_interval")]
    pub freq_force_reapply_interval: u32,
    #[serde(default = "default_fixed_max_frame_ms")]
    pub fixed_max_frame_ms: f32,

    // ── 冷启动 ──
    #[serde(default = "default_cold_boot_ms")]
    pub cold_boot_ms: u64,
}


// ── 所有默认值函数 ──

pub fn default_fps_gears() -> Vec<f32> { vec![30.0, 60.0, 90.0, 120.0, 144.0] }
pub fn default_fps_margin() -> String { "3".to_string() }

fn default_perf_floor_norm() -> f32 { 0.15 }
fn default_perf_ceil_norm() -> f32 { 1.0 }
fn default_perf_init() -> f32 { 0.40 }
fn default_perf_cold_boot() -> f32 { 0.85 }
pub fn default_hysteresis() -> f32 { 0.015 }
fn default_auto_capacity_weight() -> bool { true }

pub fn default_heavy_frame_ms() -> f32 { 150.0 }
pub fn default_loading_cumulative_ms() -> f32 { 2500.0 }
fn default_loading_normal_tolerance() -> u32 { 3 }
fn default_loading_perf_floor() -> f32 { 0.60 }
fn default_loading_perf_ceiling() -> f32 { 0.70 }
fn default_loading_reentry_cooldown() -> u32 { 60 }

pub fn default_post_loading_ignore() -> u32 { 5 }
pub fn default_post_loading_perf_min() -> f32 { 0.50 }
pub fn default_post_loading_perf_max() -> f32 { 0.80 }
fn default_post_loading_downgrade_guard() -> u32 { 90 }

fn default_sustained_loading_cycle_threshold() -> u32 { 3 }
fn default_sustained_loading_window_ns() -> u64 { 10_000_000_000 }
fn default_sustained_post_loading_ignore() -> u32 { 30 }

pub fn default_instant_error_threshold() -> f32 { 4.0 }

fn default_soft_loading_fps_ratio() -> f32 { 0.5 }
fn default_soft_loading_perf_threshold() -> f32 { 0.70 }
fn default_soft_loading_confirm_frames() -> u32 { 30 }
fn default_soft_loading_perf_cap() -> f32 { 0.40 }
fn default_soft_loading_exit_frames() -> u32 { 45 }
fn default_soft_loading_breakthrough_fps_ratio() -> f32 { 0.65 }
fn default_soft_loading_breakthrough_window() -> usize { 15 }
fn default_soft_loading_exit_frames_breakthrough() -> u32 { 20 }
fn default_soft_loading_probe_interval() -> u32 { 120 }
fn default_soft_loading_probe_duration() -> u32 { 15 }
fn default_soft_loading_probe_perf_cap() -> f32 { 0.70 }
fn default_soft_loading_probe_fps_gain_ratio() -> f32 { 0.3 }
fn default_soft_loading_probe_fail_decay_step() -> f32 { 0.10 }
fn default_soft_loading_downgrade_check_frames() -> u32 { 45 }
fn default_soft_loading_gear_match_tolerance() -> f32 { 8.0 }

fn default_app_switch_gap_ms() -> f32 { 3000.0 }
fn default_app_switch_resume_perf() -> f32 { 0.60 }
fn default_app_switch_ignore_frames() -> u32 { 8 }

fn default_scene_transition_cv_threshold() -> f32 { 0.45 }
fn default_scene_transition_guard_frames() -> u32 { 40 }
fn default_scene_transition_max_continuous() -> u32 { 120 }
fn default_scene_transition_fps_floor_ratio() -> f32 { 0.3 }
fn default_scene_transition_force_exit_frames() -> u32 { 15 }

fn default_jank_cooldown_crit() -> u32 { 10 }
fn default_jank_cooldown_heavy() -> u32 { 5 }

fn default_downgrade_confirm_frames() -> u32 { 90 }
fn default_downgrade_boost_perf_inc() -> f32 { 0.15 }
fn default_downgrade_boost_duration() -> u32 { 45 }
fn default_downgrade_proximity_ratio() -> f32 { 0.92 }
fn default_upgrade_cooldown_after_downgrade() -> u32 { 90 }
fn default_stable_forgive_frames() -> u32 { 900 }

fn default_fast_decay_frame_threshold() -> u32 { 60 }
fn default_fast_decay_perf_threshold() -> f32 { 0.65 }
fn default_fast_decay_max_step() -> f32 { 0.030 }
fn default_fast_decay_min_step() -> f32 { 0.005 }
fn default_fast_decay_post_jank_suppress() -> u32 { 90 }

fn default_deficit_suppress_ms() -> f32 { 0.3 }

fn default_mismatch_lock_threshold() -> u32 { 8 }
fn default_mismatch_reapply_skip_cycles() -> u32 { 3 }

fn default_max_inc_damped() -> f32 { 0.04 }
fn default_max_inc_normal() -> f32 { 0.07 }
fn default_damped_perf_cap() -> f32 { 0.90 }

fn default_freq_force_reapply_interval() -> u32 { 30 }
fn default_fixed_max_frame_ms() -> f32 { 500.0 }
fn default_cold_boot_ms() -> u64 { 3500 }


impl Default for FasRulesConfig {
    fn default() -> Self {
        Self {
            fps_gears: default_fps_gears(),
            fps_margin: default_fps_margin(),
            pid: PidCoefficients::default(),
            cluster_profiles: default_cluster_profiles(),
            auto_capacity_weight: default_auto_capacity_weight(),
            adaptive_pid: AdaptivePidConfig::default(),

            perf_floor: default_perf_floor_norm(),
            perf_ceil: default_perf_ceil_norm(),
            perf_init: default_perf_init(),
            perf_cold_boot: default_perf_cold_boot(),
            freq_hysteresis: default_hysteresis(),

            heavy_frame_threshold_ms: default_heavy_frame_ms(),
            loading_cumulative_ms: default_loading_cumulative_ms(),
            loading_normal_tolerance: default_loading_normal_tolerance(),
            loading_perf_floor: default_loading_perf_floor(),
            loading_perf_ceiling: default_loading_perf_ceiling(),
            loading_reentry_cooldown: default_loading_reentry_cooldown(),

            post_loading_ignore_frames: default_post_loading_ignore(),
            post_loading_perf_min: default_post_loading_perf_min(),
            post_loading_perf_max: default_post_loading_perf_max(),
            post_loading_downgrade_guard: default_post_loading_downgrade_guard(),

            sustained_loading_cycle_threshold: default_sustained_loading_cycle_threshold(),
            sustained_loading_window_ns: default_sustained_loading_window_ns(),
            sustained_post_loading_ignore: default_sustained_post_loading_ignore(),

            instant_error_threshold_ms: default_instant_error_threshold(),

            soft_loading_fps_ratio: default_soft_loading_fps_ratio(),
            soft_loading_perf_threshold: default_soft_loading_perf_threshold(),
            soft_loading_confirm_frames: default_soft_loading_confirm_frames(),
            soft_loading_perf_cap: default_soft_loading_perf_cap(),
            soft_loading_exit_frames: default_soft_loading_exit_frames(),
            soft_loading_breakthrough_fps_ratio: default_soft_loading_breakthrough_fps_ratio(),
            soft_loading_breakthrough_window: default_soft_loading_breakthrough_window(),
            soft_loading_exit_frames_breakthrough: default_soft_loading_exit_frames_breakthrough(),
            soft_loading_probe_interval: default_soft_loading_probe_interval(),
            soft_loading_probe_duration: default_soft_loading_probe_duration(),
            soft_loading_probe_perf_cap: default_soft_loading_probe_perf_cap(),
            soft_loading_probe_fps_gain_ratio: default_soft_loading_probe_fps_gain_ratio(),
            soft_loading_probe_fail_decay_step: default_soft_loading_probe_fail_decay_step(),
            soft_loading_downgrade_check_frames: default_soft_loading_downgrade_check_frames(),
            soft_loading_gear_match_tolerance: default_soft_loading_gear_match_tolerance(),

            app_switch_gap_ms: default_app_switch_gap_ms(),
            app_switch_resume_perf: default_app_switch_resume_perf(),
            app_switch_ignore_frames: default_app_switch_ignore_frames(),

            scene_transition_cv_threshold: default_scene_transition_cv_threshold(),
            scene_transition_guard_frames: default_scene_transition_guard_frames(),
            scene_transition_max_continuous: default_scene_transition_max_continuous(),
            scene_transition_fps_floor_ratio: default_scene_transition_fps_floor_ratio(),
            scene_transition_force_exit_frames: default_scene_transition_force_exit_frames(),

            jank_cooldown_frames_crit: default_jank_cooldown_crit(),
            jank_cooldown_frames_heavy: default_jank_cooldown_heavy(),

            downgrade_confirm_frames: default_downgrade_confirm_frames(),
            downgrade_boost_perf_inc: default_downgrade_boost_perf_inc(),
            downgrade_boost_duration: default_downgrade_boost_duration(),
            downgrade_proximity_ratio: default_downgrade_proximity_ratio(),
            upgrade_cooldown_after_downgrade: default_upgrade_cooldown_after_downgrade(),
            stable_forgive_frames: default_stable_forgive_frames(),

            fast_decay_frame_threshold: default_fast_decay_frame_threshold(),
            fast_decay_perf_threshold: default_fast_decay_perf_threshold(),
            fast_decay_max_step: default_fast_decay_max_step(),
            fast_decay_min_step: default_fast_decay_min_step(),
            fast_decay_post_jank_suppress: default_fast_decay_post_jank_suppress(),

            deficit_suppress_ms: default_deficit_suppress_ms(),

            mismatch_lock_threshold: default_mismatch_lock_threshold(),
            mismatch_reapply_skip_cycles: default_mismatch_reapply_skip_cycles(),

            max_inc_damped: default_max_inc_damped(),
            max_inc_normal: default_max_inc_normal(),
            damped_perf_cap: default_damped_perf_cap(),

            freq_force_reapply_interval: default_freq_force_reapply_interval(),
            fixed_max_frame_ms: default_fixed_max_frame_ms(),
            cold_boot_ms: default_cold_boot_ms(),
        }
    }
}


// 辅助函数
fn default_true() -> bool { true }

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RulesConfig {
    #[serde(default = "default_true")]
    pub yumi_scheduler: bool,
    pub dynamic_enabled: bool,
    pub global_mode: String,
    pub app_modes: HashMap<String, String>,
    #[serde(default)]
    pub ignored_apps: Vec<String>,
    #[serde(default)]
    pub fas_rules: FasRulesConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BootScriptsConfig {
    pub scripts: HashMap<String, bool>,
}

pub fn read_config<T, P>(path: P) -> Result<T, Box<dyn Error>>
where
    T: DeserializeOwned + Default,
    P: AsRef<std::path::Path>,
{
    let mut file_content = String::new();
    let path_ref = path.as_ref();

    match File::open(path_ref) {
        Ok(mut file) => {
            file.read_to_string(&mut file_content)?;
            match serde_yaml::from_str(&file_content) {
                Ok(config) => Ok(config),
                Err(e) => {
                    warn!("[Config] Failed to parse YAML at {}: {}. Using default.", path_ref.display(), e);
                    Ok(T::default())
                }
            }
        }
        Err(_) => {
            warn!("[Config] Config file not found at {}. Using default.", path_ref.display());
            Ok(T::default())
        }
    }
}