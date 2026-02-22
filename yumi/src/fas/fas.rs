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
}

impl Default for FasRules {
    fn default() -> Self {
        Self { fps_gears: default_gears(), fps_margin: default_margin() }
    }
}

fn default_gears() -> Vec<f32> { vec![30.0, 60.0, 90.0, 120.0, 144.0] }
fn default_margin() -> String { "3".to_string() }

/// 极速文件写入器，保持文件描述符常开
pub struct FastWriter {
    file: Option<File>,
}

impl FastWriter {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let file = OpenOptions::new().write(true).open(path).ok();
        Self { file }
    }

    pub fn write_value(&mut self, value: u32) {
        if let Some(file) = &mut self.file {
            let mut buf = itoa::Buffer::new();
            let val_str = buf.format(value);
            
            let _ = file.seek(SeekFrom::Start(0));
            let _ = file.write_all(val_str.as_bytes());
            let _ = file.set_len(val_str.len() as u64); 
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

pub struct FasController {
    kp: f32, 
    ki: f32,
    kd: f32,
    integral: f32,
    last_error_ms: f32,
    target_frame_ns: u64, 
    fps_gears: Vec<f32>,
    fps_margin: f32,
    current_target_fps: f32,
    time_since_last_high_fps_ns: u64,
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
            last_error_ms: 0.0,
            target_frame_ns: 16_666_666, 
            fps_gears: default_gears(),
            fps_margin: 3.0, // 默认 3 FPS 的裕度
            current_target_fps: 60.0,
            time_since_last_high_fps_ns: 0,
            virtual_freq: 9999999,
            global_max_freq: 9999999,
            global_min_freq: 0,
            policies: Vec::new(),
        }
    }

    /// 核心功能：读取 rules.yaml，解析频率表并接管核心
    pub fn load_policies(&mut self, config: &Config) {
        self.policies.clear();
        
        // 1. 读取 rules.yaml 获取自定义的 FPS 档位
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
                info!("FAS loaded custom rules: gears={:?}, margin={}", self.fps_gears, self.fps_margin);
            }
        }

        // 2. 加载核心策略
        let core_info = &config.core_framework;
        let default_freq = FreqSettings::default();
        let freq_bounds = config.get_mode("performance").map(|m| &m.freq).unwrap_or(&default_freq);

        let clusters = vec![
            (core_info.small_core_path, freq_bounds.small_core_min_freq, freq_bounds.small_core_max_freq),
            (core_info.medium_core_path, freq_bounds.medium_core_min_freq, freq_bounds.medium_core_max_freq),
            (core_info.big_core_path, freq_bounds.big_core_min_freq, freq_bounds.big_core_max_freq),
            (core_info.super_big_core_path, freq_bounds.super_big_core_min_freq, freq_bounds.super_big_core_max_freq),
        ];

        let mut global_max = 0;
        let mut global_min = 9999999;

        for (policy_id, min_f, max_f) in clusters {
            if policy_id != -1 {
                let avail_path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_available_frequencies", policy_id);
                let mut avail_freqs: Vec<u32> = fs::read_to_string(&avail_path)
                    .unwrap_or_default()
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .filter(|&f| f >= min_f && f <= max_f)
                    .collect();

                if avail_freqs.is_empty() {
                    avail_freqs = vec![min_f, max_f];
                } else {
                    avail_freqs.sort_unstable();
                    avail_freqs.dedup(); 
                }

                let max_path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_max_freq", policy_id);
                let min_path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_min_freq", policy_id);
                let init_freq = *avail_freqs.last().unwrap_or(&max_f);

                let mut max_writer = FastWriter::new(&max_path);
                let mut min_writer = FastWriter::new(&min_path);

                max_writer.write_value(init_freq);
                min_writer.write_value(init_freq);

                self.policies.push(PolicyController {
                    max_writer,
                    min_writer,
                    available_freqs: avail_freqs,
                    current_freq: init_freq, 
                });
                
                if max_f > global_max { global_max = max_f; }
                if min_f < global_min { global_min = min_f; }
            }
        }
        
        self.global_max_freq = global_max;
        self.global_min_freq = global_min;
        self.virtual_freq = global_max; 
        self.current_target_fps = *self.fps_gears.first().unwrap_or(&60.0);
        self.time_since_last_high_fps_ns = 0;
        info!("FAS dynamically loaded {} CPU policies.", self.policies.len());
    }

    pub fn update_frame(&mut self, frame_delta_ns: u64) {
        if self.policies.is_empty() { return; }

        let current_fps = 1_000_000_000.0 / frame_delta_ns as f32;
        
        // 1. 寻找这一帧瞬时表现最接近的档位
        let inst_gear = self.fps_gears.iter()
            .min_by(|&&a, &&b| (a - current_fps).abs().partial_cmp(&(b - current_fps).abs()).unwrap())
            .copied()
            .unwrap_or(60.0);

        // 高水位与延迟降档逻辑
        if inst_gear >= self.current_target_fps {
            // 只要达到或超越当前目标，立刻升档，并重置降档计时器
            self.current_target_fps = inst_gear;
            self.time_since_last_high_fps_ns = 0;
        } else {
            // 如果瞬时表现变差（可能掉帧，或切回菜单），开始累计时间
            self.time_since_last_high_fps_ns += frame_delta_ns;

            // 如果连续 2 秒 (2_000_000_000 纳秒) 都没有摸到目标档位，才确认是真切场景了，允许降档
            if self.time_since_last_high_fps_ns > 2_000_000_000 {
                self.current_target_fps = inst_gear;
                self.time_since_last_high_fps_ns = 0;
                // debug!("FAS safely downgraded to {} FPS gear", inst_gear);
            }
        }

        let acceptable_fps = (self.current_target_fps - self.fps_margin).max(1.0);
        self.target_frame_ns = (1_000_000_000.0 / acceptable_fps) as u64;

        let margin_ns = self.target_frame_ns as i64 - frame_delta_ns as i64;
        let error_ms = margin_ns as f32 / 1_000_000.0;

        // 1. I (积分) 逻辑，顺便带上刚才修复的防积分饱和
        if error_ms > 0.0 {
            self.integral = (self.integral + error_ms * self.ki).min(100.0);
        } else {
            self.integral = 0.0; 
        }

        // 2. D (微分) 逻辑：计算误差变化率
        let derivative = error_ms - self.last_error_ms;
        self.last_error_ms = error_ms; // 记录本次误差供下一帧使用

        // 3. 计算 PID 各项输出
        let p_out = error_ms * self.kp * 100_000.0; 
        let i_out = self.integral * 10_000.0;
        let d_out = derivative * self.kd * 100_000.0; // 新增：微分算力输出

        // 4. 总算力变化 = P + I + D
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