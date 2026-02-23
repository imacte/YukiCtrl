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

use crate::scheduler::config::{Config, FreqSettings};
use std::fs::{self, File, OpenOptions};
use std::io::{Write, Seek, SeekFrom};
use std::path::Path;
use log::info;
use serde::Deserialize;

// 用于内部独立解析 rules.yaml 中的 FAS 规则
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
    /// 降档确认延迟，单位毫秒，默认 2000ms
    /// 游戏场景可适当调大（如 3000），桌面/视频可调小（如 500）
    #[serde(default = "default_downgrade_delay_ms")]
    downgrade_delay_ms: String,
}

impl Default for FasRules {
    fn default() -> Self {
        Self {
            fps_gears: default_gears(),
            fps_margin: default_margin(),
            downgrade_delay_ms: default_downgrade_delay_ms(),
        }
    }
}

fn default_gears() -> Vec<f32> { vec![30.0, 60.0, 90.0, 120.0, 144.0] }
fn default_margin() -> String { "3".to_string() }
fn default_downgrade_delay_ms() -> String { "3000".to_string() }

/// 极速文件写入器，保持文件描述符常开，内置写入去重
pub struct FastWriter {
    file: Option<File>,
    /// 缓存上次写入的值，相同值直接跳过 syscall
    last_value: Option<u32>,
}

impl FastWriter {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let file = OpenOptions::new().write(true).open(path).ok();
        Self { file, last_value: None }
    }

    pub fn write_value(&mut self, value: u32) {
        // 去重：与上次写入值相同则直接返回，避免无意义的内核 sysfs 写入
        if self.last_value == Some(value) {
            return;
        }

        if let Some(file) = &mut self.file {
            let mut buf = itoa::Buffer::new();
            let val_str = buf.format(value);

            let _ = file.seek(SeekFrom::Start(0));
            let _ = file.write_all(val_str.as_bytes());
            let _ = file.set_len(val_str.len() as u64);

            self.last_value = Some(value);
        }
    }
}

/// 单个 CPU 簇的控制器 (支持阶梯频率)
pub struct PolicyController {
    pub max_writer: FastWriter,
    pub min_writer: FastWriter,
    pub available_freqs: Vec<u32>, // 从低到高排序的可用频率表
    pub current_freq: u32,
}

impl PolicyController {
    /// 安全地应用目标频率，防止内核报 OS error 22
    pub fn apply_freq_safe(&mut self, target_freq: u32) {
        if target_freq == self.current_freq {
            return;
        }

        if target_freq > self.current_freq {
            // 升频：先抬高天花板 (Max)，再抬高地板 (Min)
            self.max_writer.write_value(target_freq);
            self.min_writer.write_value(target_freq);
        } else {
            // 降频：先降低地板 (Min)，再降低天花板 (Max)
            self.min_writer.write_value(target_freq);
            self.max_writer.write_value(target_freq);
        }

        self.current_freq = target_freq;
    }
}

/// 定长环形缓冲区，用于计算最近 N 帧的滑动平均 FPS
struct FpsWindow {
    buf: [f32; 5],
    pos: usize,
    filled: bool,
}

impl FpsWindow {
    fn new() -> Self {
        Self { buf: [0.0; 5], pos: 0, filled: false }
    }

    fn push(&mut self, fps: f32) {
        self.buf[self.pos] = fps;
        self.pos = (self.pos + 1) % 5;
        if self.pos == 0 { self.filled = true; }
    }

    /// 返回已填充帧的均值；未满 5 帧时只对已有帧求均值
    fn mean(&self) -> f32 {
        let len = if self.filled { 5 } else { self.pos.max(1) };
        self.buf[..len].iter().sum::<f32>() / len as f32
    }
}

pub struct FasController {
    kp: f32,
    ki: f32,
    kd: f32,
    integral: f32,
    /// 经过 EMA 平滑后的误差，用于计算微分项，抑制帧时间抖动引入的噪声
    smoothed_error_ms: f32,
    target_frame_ns: u64,
    fps_gears: Vec<f32>,
    fps_margin: f32,
    current_target_fps: f32,
    time_since_last_high_fps_ns: u64,
    /// 降档确认延迟（纳秒），从 rules.yaml 的 downgrade_delay_ms 转换而来
    downgrade_delay_ns: u64,
    upgrade_confirm_frames: u32,
    /// 最近 5 帧的 FPS 滑动窗口，用于平滑计算当前档位
    fps_window: FpsWindow,
    virtual_freq: u32,
    global_max_freq: u32,
    global_min_freq: u32,
    pub policies: Vec<PolicyController>,
}

impl FasController {
    pub fn new() -> Self {
        Self {
            kp: 0.5,
            ki: 0.05,
            kd: 0.1,
            integral: 0.0,
            smoothed_error_ms: 0.0,
            target_frame_ns: 16_666_666,
            fps_gears: default_gears(),
            fps_margin: 3.0,
            current_target_fps: 60.0,
            time_since_last_high_fps_ns: 0,
            downgrade_delay_ns: 2_000_000_000, // 默认 2 秒
            upgrade_confirm_frames: 0,
            fps_window: FpsWindow::new(),
            virtual_freq: 9999999,
            global_max_freq: 9999999,
            global_min_freq: 0,
            policies: Vec::new(),
        }
    }

    /// 核心功能：读取 rules.yaml，解析频率表并接管核心
    pub fn load_policies(&mut self, config: &Config) {
        self.policies.clear();

        // 0. 准备 FAS 运行环境：直接干掉系统的 FEAS / FPSGO 干扰
        let _ = crate::utils::try_write_file("/sys/module/perfmgr/parameters/perfmgr_enable", "0");
        let _ = crate::utils::try_write_file("/sys/module/mtk_fpsgo/parameters/perfmgr_enable", "0");
        info!("FAS environment setup: Disabled system FEAS/FPSGO mechanisms.");

        // 1. 读取 rules.yaml 获取自定义的 FPS 档位和降档延迟
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
                // 将毫秒转换为纳秒存储
                self.downgrade_delay_ns = rules.fas_rules.downgrade_delay_ms.parse::<u64>().unwrap_or(2000) * 1_000_000;
                info!(
                    "FAS loaded custom rules: gears={:?}, margin={}, downgrade_delay={}ms",
                    self.fps_gears, self.fps_margin, rules.fas_rules.downgrade_delay_ms
                );
            }
        }

        // 2. 加载核心策略：直接读取系统硬件支持的真实上下限
        let core_info = &config.core_framework;
        
        // 我们不再需要借用 performance 模式的配置，只需知道有哪些核心簇即可
        let clusters = vec![
            core_info.small_core_path,
            core_info.medium_core_path,
            core_info.big_core_path,
            core_info.super_big_core_path,
        ];

        let mut global_max = 0u32;
        let mut global_min = u32::MAX;

        for policy_id in clusters {
            if policy_id != -1 {
                // 切换调度器到 performance
                let gov_path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_governor", policy_id);
                if let Err(e) = crate::utils::try_write_file(&gov_path, "performance") {
                    log::warn!("FAS Error: Failed to set policy{} to performance: {}", policy_id, e);
                } else {
                    log::debug!("FAS: Locked policy{} to performance governor.", policy_id);
                }

                // 读取硬件支持的真实频率表
                let avail_path = format!(
                    "/sys/devices/system/cpu/cpufreq/policy{}/scaling_available_frequencies",
                    policy_id
                );
                
                let mut avail_freqs: Vec<u32> = fs::read_to_string(&avail_path)
                    .unwrap_or_default()
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                // 防御性编程：如果因为某些奇怪的内核原因读不到可用频率，直接跳过该核心
                if avail_freqs.is_empty() {
                    log::error!("FAS: Failed to read available frequencies for policy{}. Skipping.", policy_id);
                    continue; 
                } else {
                    avail_freqs.sort_unstable();
                    avail_freqs.dedup();
                }

                // 直接从硬件物理频率表中获取真实上下限
                let min_f = *avail_freqs.first().unwrap();
                let max_f = *avail_freqs.last().unwrap();
                let init_freq = max_f;

                let max_path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_max_freq", policy_id);
                let min_path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_min_freq", policy_id);

                let mut max_writer = FastWriter::new(&max_path);
                let mut min_writer = FastWriter::new(&min_path);

                // FAS 刚接管时，先保守地将频率拉满，防止掉帧
                max_writer.write_value(init_freq);
                min_writer.write_value(init_freq);

                self.policies.push(PolicyController {
                    max_writer,
                    min_writer,
                    available_freqs: avail_freqs,
                    current_freq: init_freq,
                });

                // 更新全局上下限，供 PID 计算算力使用
                if max_f > global_max { global_max = max_f; }
                if min_f < global_min { global_min = min_f; }
            }
        }

        self.global_max_freq = global_max;
        // 健壮性 fallback：无任何有效 policy 时 global_min 保持 u32::MAX，重置为 0 防止 clamp 异常
        self.global_max_freq = if global_max == 0 { 9999999 } else { global_max };
        self.global_min_freq = if global_min == u32::MAX { 0 } else { global_min };
        self.virtual_freq = self.global_max_freq;
        
        self.current_target_fps = *self.fps_gears.first().unwrap_or(&60.0);
        self.time_since_last_high_fps_ns = 0;
        info!("FAS dynamically loaded {} CPU policies.", self.policies.len());

        // 重置所有控制器的状态
        self.integral = 0.0;
        self.smoothed_error_ms = 0.0;
        self.upgrade_confirm_frames = 0;
        self.fps_window = FpsWindow::new();
    }

    pub fn update_frame(&mut self, frame_delta_ns: u64) {
        if frame_delta_ns == 0 { return; }
        if self.policies.is_empty() { return; }

        let current_fps = 1_000_000_000.0 / frame_delta_ns as f32;

        // [步骤 A] 将当前帧 FPS 推入滑动窗口，用 5 帧均值计算所属档位
        //          避免单帧瞬时抖动（GC、中断等）影响档位判断
        self.fps_window.push(current_fps);
        let avg_fps = self.fps_window.mean();

        let inst_gear = self.fps_gears.iter()
            .min_by(|&&a, &&b| (a - avg_fps).abs().partial_cmp(&(b - avg_fps).abs()).unwrap())
            .copied()
            .unwrap_or(60.0);

        // 高水位与延迟降档逻辑
        if inst_gear >= self.current_target_fps {
            // 连续 3 帧均值都达到更高档位，才确认升档
            if inst_gear > self.current_target_fps {
                self.upgrade_confirm_frames += 1;
                if self.upgrade_confirm_frames >= 3 {
                    self.current_target_fps = inst_gear;
                    self.upgrade_confirm_frames = 0;
                }
            } else {
                self.upgrade_confirm_frames = 0;
            }
            self.time_since_last_high_fps_ns = 0;
        } else {
            // 瞬时均值表现变差，开始累计时间
            self.time_since_last_high_fps_ns += frame_delta_ns;

            // 超过可配置的降档延迟后，才确认降档
            if self.time_since_last_high_fps_ns > self.downgrade_delay_ns {
                self.current_target_fps = inst_gear;
                self.time_since_last_high_fps_ns = 0;
            }
        }

        let acceptable_fps = (self.current_target_fps - self.fps_margin).max(1.0);
        self.target_frame_ns = (1_000_000_000.0 / acceptable_fps) as u64;

        let margin_ns = self.target_frame_ns as i64 - frame_delta_ns as i64;
        let error_ms = margin_ns as f32 / 1_000_000.0;

        // 积分逻辑：双边限幅，正负方向均可累积
        self.integral = (self.integral + error_ms * self.ki).clamp(-100.0, 100.0);

        // [步骤 B] 微分项：对误差做 EMA 低通滤波，再计算导数
        //          α=0.2：新值权重 20%，旧值权重 80%，平滑帧时间随机抖动
        const D_ALPHA: f32 = 0.2;
        let prev_smoothed = self.smoothed_error_ms;
        self.smoothed_error_ms = prev_smoothed * (1.0 - D_ALPHA) + error_ms * D_ALPHA;
        let derivative = self.smoothed_error_ms - prev_smoothed;

        // 频率缩放系数：将毫秒级误差映射到 Hz 量级的频率变化
        const FREQ_SCALE: f32 = 100_000.0;

        // PID 各项输出
        let p_out = error_ms * self.kp * FREQ_SCALE;
        let i_out = self.integral * 10_000.0;
        let d_out = derivative * self.kd * FREQ_SCALE;

        // [步骤 C] 总算力变化 = P + I + D
        let delta_freq = p_out + i_out + d_out;

        let mut new_virtual_freq = (self.virtual_freq as f32 - delta_freq).max(0.0) as u32;

        if error_ms < -3.0 {
            new_virtual_freq = self.global_max_freq;
        }

        new_virtual_freq = new_virtual_freq.clamp(self.global_min_freq, self.global_max_freq);

        if new_virtual_freq.abs_diff(self.virtual_freq) > 50_000 {
            self.virtual_freq = new_virtual_freq;

            for policy in &mut self.policies {
                let target_freq = policy.available_freqs.iter()
                    .find(|&&f| f >= new_virtual_freq)
                    .copied()
                    .unwrap_or_else(|| *policy.available_freqs.last().unwrap());

                policy.apply_freq_safe(target_freq);
            }
        }
    }
}