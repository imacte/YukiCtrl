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

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct FasRulesConfig {
    #[serde(default = "default_fps_gears")]
    pub fps_gears: Vec<f32>,
    #[serde(default = "default_fps_margin")]
    pub fps_margin: String,
    #[serde(default = "default_heavy_frame_ms")]
    pub heavy_frame_threshold_ms: f32,
    #[serde(default = "default_loading_cumulative_ms")]
    pub loading_cumulative_ms: f32,
    #[serde(default = "default_post_loading_ignore")]
    pub post_loading_ignore_frames: u32,
    #[serde(default = "default_post_loading_perf_min")]
    pub post_loading_perf_min: f32,
    #[serde(default = "default_post_loading_perf_max")]
    pub post_loading_perf_max: f32,
    #[serde(default = "default_instant_error_threshold")]
    pub instant_error_threshold_ms: f32,
    #[serde(default = "default_perf_floor")]
    pub perf_floor: f32,
    #[serde(default = "default_hysteresis")]
    pub freq_hysteresis: f32,
    #[serde(default = "default_perf_ceil")]
    pub perf_ceil: f32,
}

pub fn default_fps_gears() -> Vec<f32> { vec![30.0, 60.0, 90.0, 120.0, 144.0] }
pub fn default_fps_margin() -> String { "3".to_string() }
pub fn default_heavy_frame_ms() -> f32 { 150.0 }
pub fn default_loading_cumulative_ms() -> f32 { 2500.0 }
pub fn default_post_loading_ignore() -> u32 { 5 }
pub fn default_post_loading_perf_min() -> f32 { 500.0 }
pub fn default_post_loading_perf_max() -> f32 { 800.0 }
pub fn default_instant_error_threshold() -> f32 { 4.0 }
pub fn default_perf_floor() -> f32 { 150.0 }
pub fn default_hysteresis() -> f32 { 0.015 }
pub fn default_perf_ceil() -> f32 { 850.0 }


impl Default for FasRulesConfig {
    fn default() -> Self {
        Self {
            fps_gears: default_fps_gears(),
            fps_margin: default_fps_margin(),
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