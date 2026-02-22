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
    #[serde(default)]
    pub latency_threshold: String,
    #[serde(default)]
    pub poll_interval_ms: String,
}

fn default_fps_gears() -> Vec<f32> { vec![30.0, 60.0, 90.0, 120.0, 144.0] }
fn default_fps_margin() -> String { "3".to_string() }

impl Default for FasRulesConfig {
    fn default() -> Self {
        Self {
            fps_gears: default_fps_gears(),
            fps_margin: default_fps_margin(),
            latency_threshold: "".to_string(),
            poll_interval_ms: "".to_string(),
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