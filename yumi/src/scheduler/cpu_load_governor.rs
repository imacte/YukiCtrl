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

//! CPU Load Governor — 全局负载感知升降频
//!
//! 基于 eBPF 采集的每核心利用率，实时调节各 cluster 频率。
//! 与 FAS（帧感知调度）互斥运行：FAS 激活时本模块自动让位。
//! 与 AppLaunchBoost 互斥：Boost 期间暂停写 sysfs，避免频率互相覆盖。

use crate::scheduler::config::Config;
use crate::monitor::config::CpuLoadGovernorConfig;
use super::fas::FastWriter;
use log::{info, debug, warn};
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// ════════════════════════════════════════════════════════════════
//  ClusterState — 单 cluster 运行时状态
// ════════════════════════════════════════════════════════════════

struct ClusterState {
    policy_id: i32,
    /// 属于此 cluster 的 CPU 核心编号 (用于从 core_utils 取最大利用率)
    affected_cpus: Vec<usize>,
    /// 排序后的可用频率表
    available_freqs: Vec<u32>,
    /// 对应 available_freqs 的归一化比例缓存 (0.0~1.0)
    cached_ratios: Vec<f32>,
    _freq_min: f32,
    _freq_max: f32,
    max_writer: FastWriter,
    min_writer: FastWriter,
    /// 当前 perf 指数 (0.0~1.0)，平滑后的值
    current_perf: f32,
    /// 当前写入的频率值
    current_freq: u32,
    /// 降频等待计数器
    down_wait: u32,
}

impl ClusterState {
    fn find_nearest_freq(&self, target_ratio: f32) -> u32 {
        let idx = self.cached_ratios.partition_point(|&r| r < target_ratio);
        if idx == 0 {
            self.available_freqs[0]
        } else if idx >= self.available_freqs.len() {
            *self.available_freqs.last().unwrap()
        } else {
            let lo = idx - 1;
            let hi = idx;
            if (self.cached_ratios[hi] - target_ratio).abs()
                < (self.cached_ratios[lo] - target_ratio).abs()
            { self.available_freqs[hi] } else { self.available_freqs[lo] }
        }
    }

    /// 写入频率 (min=max 锁频模式)
    fn write_freq(&mut self, freq: u32) {
        if freq == self.current_freq { return; }
        if freq >= self.current_freq {
            // 升频：先拉 max 再拉 min
            self.max_writer.write_value_force(freq);
            self.min_writer.write_value_force(freq);
        } else {
            // 降频：先降 min 再降 max
            self.min_writer.write_value_force(freq);
            self.max_writer.write_value_force(freq);
        }
        self.current_freq = freq;
    }

    /// 计算此 cluster 涉及核心的最大利用率
    fn max_util(&self, core_utils: &[f32]) -> f32 {
        self.affected_cpus.iter()
            .filter_map(|&cpu| core_utils.get(cpu))
            .copied()
            .fold(0.0_f32, f32::max)
    }
}

// ════════════════════════════════════════════════════════════════
//  CpuLoadGovernor — 主控制器
// ════════════════════════════════════════════════════════════════

pub struct CpuLoadGovernor {
    clusters: Vec<ClusterState>,
    cfg: CpuLoadGovernorConfig,
    active: bool,
    /// 统计计数器，每 N 个 tick 输出一次日志
    log_counter: u32,
    /// 引用 AppLaunchBoost 的全局标志，Boost 期间暂停写频率
    is_boosting: Option<Arc<AtomicBool>>,
}

impl CpuLoadGovernor {
    pub fn new() -> Self {
        Self {
            clusters: Vec::new(),
            cfg: CpuLoadGovernorConfig::default(),
            active: false,
            log_counter: 0,
            is_boosting: None,
        }
    }

    /// 设置 boost 标志引用，在 init 前由调度器调用
    pub fn set_boost_flag(&mut self, flag: Arc<AtomicBool>) {
        self.is_boosting = Some(flag);
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    // ────────────────────────────────────────────────────────────
    //  初始化 / 释放
    // ────────────────────────────────────────────────────────────

    /// 扫描 CPU 拓扑并接管频率控制
    /// 调用时机：进入非 FAS 模式 且 governor 已在 rules.yaml 中启用
    pub fn init_policies(&mut self, config: &Config, gov_cfg: &CpuLoadGovernorConfig) {
        self.release();
        self.cfg = gov_cfg.clone();

        let ci = &config.core_framework;
        let clusters = [ci.small_core_path, ci.medium_core_path,
                        ci.big_core_path, ci.super_big_core_path];

        for &pid in &clusters {
            if pid == -1 { continue; }

            // 1. 设置 scaling_governor 为 performance，夺取频率控制权
            let gov_path = format!(
                "/sys/devices/system/cpu/cpufreq/policy{}/scaling_governor", pid);
            let _ = crate::utils::try_write_file(&gov_path, "performance");

            // 2. 读取可用频率表
            let freq_path = format!(
                "/sys/devices/system/cpu/cpufreq/policy{}/scaling_available_frequencies", pid);
            let mut freqs: Vec<u32> = fs::read_to_string(&freq_path)
                .unwrap_or_default()
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if freqs.is_empty() { continue; }
            freqs.sort_unstable();
            freqs.dedup();

            // 3. 读取 affected_cpus
            let affected = Self::read_affected_cpus(pid);
            if affected.is_empty() { continue; }

            // 4. 构建 ratio 缓存
            let fmin = *freqs.first().unwrap() as f32;
            let fmax = *freqs.last().unwrap() as f32;
            let range = (fmax - fmin).max(1.0);
            let cached_ratios: Vec<f32> = freqs.iter()
                .map(|&f| (f as f32 - fmin) / range)
                .collect();

            // 5. 创建 sysfs 写入器
            let max_writer = FastWriter::new(format!(
                "/sys/devices/system/cpu/cpufreq/policy{}/scaling_max_freq", pid));
            let min_writer = FastWriter::new(format!(
                "/sys/devices/system/cpu/cpufreq/policy{}/scaling_min_freq", pid));

            // 6. 初始频率设置为中间值 (perf_init)
            let init_perf = self.cfg.perf_init.clamp(self.cfg.perf_floor, self.cfg.perf_ceil);
            let mut cluster = ClusterState {
                policy_id: pid,
                affected_cpus: affected.clone(),
                available_freqs: freqs,
                cached_ratios,
                _freq_min: fmin,
                _freq_max: fmax,
                max_writer,
                min_writer,
                current_perf: init_perf,
                current_freq: 0,
                down_wait: 0,
            };

            let init_freq = cluster.find_nearest_freq(init_perf);
            cluster.max_writer.write_value_force(init_freq);
            cluster.min_writer.write_value_force(init_freq);
            cluster.current_freq = init_freq;

            info!("CLG[P{}] init | cpus={:?} | freqs={}-{} MHz | P={:.2} -> {} kHz",
                pid, affected,
                (fmin / 1000.0) as u32, (fmax / 1000.0) as u32,
                init_perf, init_freq / 1000);

            self.clusters.push(cluster);
        }

        self.active = !self.clusters.is_empty();
        if self.active {
            info!("CPU Load Governor activated with {} cluster(s)", self.clusters.len());
        } else {
            warn!("CPU Load Governor: no valid clusters found, staying inactive");
        }
    }

    /// 释放频率控制权 (不重置频率——调用方自行处理后续)
    pub fn release(&mut self) {
        if self.active {
            info!("CPU Load Governor deactivated");
        }
        self.clusters.clear();
        self.active = false;
        self.log_counter = 0;
    }

    /// 热重载配置参数 (不重建 cluster，不中断运行)
    pub fn reload_config(&mut self, gov_cfg: &CpuLoadGovernorConfig) {
        self.cfg = gov_cfg.clone();
        debug!("CLG: config hot-reloaded | up={:.2} down={:.2} floor={:.2} ceil={:.2}",
            self.cfg.up_threshold, self.cfg.down_threshold,
            self.cfg.perf_floor, self.cfg.perf_ceil);
    }

    // ────────────────────────────────────────────────────────────
    //  主 tick：接收 SystemLoadUpdate 事件
    // ────────────────────────────────────────────────────────────

    /// 每个 eBPF 采样周期调用一次 (约 200ms)
    pub fn on_load_update(&mut self, core_utils: &[f32]) {
        if !self.active { return; }

        // Boost 期间暂停写频率，避免和 AppLaunchBoost 互相覆盖 sysfs
        // 但仍然更新内部 perf 指数，这样 Boost 结束后能立刻用正确的 perf 值恢复
        let boosting = self.is_boosting.as_ref()
            .map_or(false, |flag| flag.load(Ordering::Relaxed));

        for cluster in &mut self.clusters {
            let util = cluster.max_util(core_utils);

            // 计算目标 perf：util * headroom，钳位到 [floor, ceil]
            let target_perf = (util * self.cfg.headroom_factor)
                .clamp(self.cfg.perf_floor, self.cfg.perf_ceil);

            let old_perf = cluster.current_perf;

            if target_perf > old_perf {
                // 升频：快速响应
                if util >= self.cfg.up_threshold || target_perf > old_perf + 0.05 {
                    cluster.current_perf += (target_perf - old_perf) * self.cfg.smoothing_up;
                    cluster.down_wait = 0;
                }
            } else {
                // 降频：带 rate limit 的渐进衰减
                cluster.down_wait += 1;
                if cluster.down_wait >= self.cfg.down_rate_limit_ticks
                    && util < self.cfg.down_threshold
                {
                    cluster.current_perf += (target_perf - old_perf) * self.cfg.smoothing_down;
                }
            }

            cluster.current_perf = cluster.current_perf
                .clamp(self.cfg.perf_floor, self.cfg.perf_ceil);

            // Boost 期间只更新 perf 指数，不写 sysfs
            if !boosting {
                let target_freq = cluster.find_nearest_freq(cluster.current_perf);
                cluster.write_freq(target_freq);
            }
        }

        // 每 25 个 tick (~5秒) 输出一次摘要日志
        self.log_counter += 1;
        if self.log_counter % 25 == 0 {
            for c in &self.clusters {
                let util = c.max_util(core_utils);
                debug!("CLG[P{}] util={:.0}% perf={:.2} freq={}kHz{}",
                    c.policy_id, util * 100.0, c.current_perf, c.current_freq / 1000,
                    if boosting { " [BOOST-paused]" } else { "" });
            }
        }
    }

    // ────────────────────────────────────────────────────────────
    //  Boost 结束后重新同步频率
    // ────────────────────────────────────────────────────────────

    /// AppLaunchBoost 结束后调用，将内部 perf 指数立刻写入 sysfs
    /// 避免 Boost 恢复静态频率后、CLG 下一个 tick 之前的"空窗期"
    pub fn resync_after_boost(&mut self) {
        if !self.active { return; }
        for cluster in &mut self.clusters {
            let target_freq = cluster.find_nearest_freq(cluster.current_perf);
            // 强制使 last_value 失效，确保下次一定会写入
            cluster.max_writer.invalidate();
            cluster.min_writer.invalidate();
            cluster.write_freq(target_freq);
            debug!("CLG[P{}] resync after boost: perf={:.2} -> freq={}kHz",
                cluster.policy_id, cluster.current_perf, target_freq / 1000);
        }
    }

    // ────────────────────────────────────────────────────────────
    //  辅助
    // ────────────────────────────────────────────────────────────

    fn read_affected_cpus(policy_id: i32) -> Vec<usize> {
        let path = format!(
            "/sys/devices/system/cpu/cpufreq/policy{}/affected_cpus", policy_id);
        fs::read_to_string(&path)
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|s| s.parse::<usize>().ok())
            .collect()
    }
}