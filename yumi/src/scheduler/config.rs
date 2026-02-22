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

use serde::Deserialize;
use std::collections::HashMap;
use serde::Deserializer;
use std::fmt;

// de_util 模块保持不变
mod de_util {
    use super::*;
    use serde::de::{self, Visitor};

    pub fn deserialize_freq<'de, D>(deserializer: D) -> Result<u32, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(FreqVisitor)
    }

    struct FreqVisitor;

    impl<'de> Visitor<'de> for FreqVisitor {
        type Value = u32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string 'min', 'max', or an integer")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match value {
                "min" => Ok(0),
                "max" => Ok(9999999),
                _ => {
                    Err(de::Error::unknown_variant(value, &["min", "max"]))
                }
            }
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            u32::try_from(value).map_err(|_| {
                de::Error::invalid_value(de::Unexpected::Unsigned(value), &self)
            })
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct Meta {
    // 同时支持 "loglevel" 和 "Loglevel"
    #[serde(default = "default_loglevel", alias = "Loglevel")]
    pub loglevel: String,
    
    // 同时支持 "language" 和 "Language"
    #[serde(default = "default_language", alias = "Language")]
    pub language: String,
}

fn default_loglevel() -> String {
    "INFO".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct GovernorSettings {
    #[serde(default = "default_governor")]
    pub global: String,
    pub small_core: String,
    pub medium_core: String,
    pub big_core: String,
    pub super_big_core: String,
}

fn default_governor() -> String {
    "schedutil".to_string()
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct FreqSettings {
    #[serde(deserialize_with = "de_util::deserialize_freq")]
    pub small_core_min_freq: u32,
    #[serde(deserialize_with = "de_util::deserialize_freq")]
    pub small_core_max_freq: u32,
    #[serde(deserialize_with = "de_util::deserialize_freq")]
    pub medium_core_min_freq: u32,
    #[serde(deserialize_with = "de_util::deserialize_freq")]
    pub medium_core_max_freq: u32,
    #[serde(deserialize_with = "de_util::deserialize_freq")]
    pub big_core_min_freq: u32,
    #[serde(deserialize_with = "de_util::deserialize_freq")]
    pub big_core_max_freq: u32,
    #[serde(deserialize_with = "de_util::deserialize_freq")]
    pub super_big_core_min_freq: u32,
    #[serde(deserialize_with = "de_util::deserialize_freq")]
    pub super_big_core_max_freq: u32,
}

impl Default for FreqSettings {
    fn default() -> Self {
        Self {
            small_core_min_freq: 0,
            small_core_max_freq: 10_000_000,
            medium_core_min_freq: 0,
            medium_core_max_freq: 10_000_000,
            big_core_min_freq: 0,
            big_core_max_freq: 10_000_000,
            super_big_core_min_freq: 0,
            super_big_core_max_freq: 10_000_000,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct UclampSettings {
    pub uclamp_top_app_min: String,
    pub uclamp_top_app_max: String,
    #[serde(rename = "UclampTopApplatency_sensitive")]
    pub uclamp_top_app_latency_sensitive: String,
    pub uclamp_fore_ground_min: String,
    pub uclamp_fore_ground_max: String,
    pub uclamp_back_ground_min: String,
    pub uclamp_back_ground_max: String,
}

impl Default for UclampSettings {
    fn default() -> Self {
        Self {
            uclamp_top_app_min: "0".to_string(),
            uclamp_top_app_max: "100".to_string(),
            uclamp_top_app_latency_sensitive: "0".to_string(),
            uclamp_fore_ground_min: "0".to_string(),
            uclamp_fore_ground_max: "70".to_string(),
            uclamp_back_ground_min: "0".to_string(),
            uclamp_back_ground_max: "50".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct BusDcvs {
    #[serde(rename = "CPUllccmin")]
    pub cpullccmin: String,
    #[serde(rename = "CPUllccmax")]
    pub cpullccmax: String,
    #[serde(rename = "CPUddrmin")]
    pub cpuddrmin: String,
    #[serde(rename = "CPUddrmax")]
    pub cpuddrmax: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Other {
    #[serde(rename = "ufsClkGate")]
    pub ufs_clk_gate: bool,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Mode {
    #[serde(default)]
    pub governor: GovernorSettings,
    #[serde(default)]
    pub freq: FreqSettings,
    #[serde(default)]
    pub uclamp: UclampSettings,
    #[serde(default, rename = "Bus_dcvs")]
    pub bus_dcvs: BusDcvs,
    #[serde(default)]
    pub govsets: HashMap<String, HashMap<String, HashMap<String, String>>>,
    #[serde(default)]
    pub other: Other,
}

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default, alias = "Meta")]
    pub meta: Meta,
    #[serde(default)]
    pub function: FunctionToggles,
    #[serde(default, rename = "AppLaunchBoostSettings")]
    pub app_launch_boost_settings: AppLaunchBoostSettings,
    #[serde(default, rename = "CoreAllocation")]
    pub core_allocation: CoreAllocation,
    #[serde(default, rename = "CoreFramework")]
    pub core_framework: CoreFramework,
    #[serde(default, rename = "IO_Settings")]
    pub io_settings: IOSettings,
    #[serde(default, rename = "CompletelyFairSchedulerValue")]
    pub completely_fair_scheduler_value: CompletelyFairSchedulerValue,
    #[serde(default, rename = "CpuIdle")]
    pub cpu_idle: CpuIdle,
    #[serde(default, rename = "Cpuset")]
    pub cpu_set: Cpuset,
    #[serde(default, rename = "Bus_dcvs_Path")]
    pub bus_dcvs_path: BusDcvsPath,
    #[serde(default, rename = "pGovPath")]
    pub p_gov_path: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub powersave: Mode,
    #[serde(default)]
    pub balance: Mode,
    #[serde(default)]
    pub performance: Mode,
    #[serde(default)]
    pub fast: Mode,
}

#[derive(Debug, Deserialize, Default)]
pub struct FunctionToggles {
    #[serde(rename = "AffinitySetter")]
    pub affinity_setter: bool,
    #[serde(rename = "CpuIdleScalingGovernor")]
    pub cpu_idle_scaling_governor: bool,
    #[serde(rename = "EasScheduler")]
    pub eas_scheduler: bool,
    pub cpuset: bool,
    #[serde(rename = "LoadBalancing")]
    pub load_balancing: bool,
    #[serde(rename = "EnableFeas")]
    pub enable_feas: bool,
    #[serde(rename = "AdjIOScheduler")]
    pub adj_i_o_scheduler: bool,
    #[serde(rename = "AppLaunchBoost")]
    pub app_launch_boost: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct AppLaunchBoostSettings {
    #[serde(default = "default_freq_multi")]
    pub freq_multi: f32,
    #[serde(default = "default_boost_rate")]
    pub boost_rate_ms: u64,
}
fn default_freq_multi() -> f32 { 1.2 }
fn default_boost_rate() -> u64 { 200 }

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct CoreAllocation {
    pub cpu_set_core: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct CoreFramework {
    pub small_core_path: i32,
    pub medium_core_path: i32,
    pub big_core_path: i32,
    pub super_big_core_path: i32,
}

#[derive(Debug, Deserialize, Default)]
pub struct IOSettings {
    #[serde(rename = "Scheduler")]
    pub scheduler: String,
    #[serde(rename = "IO_optimization")]
    pub io_optimization: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct CompletelyFairSchedulerValue {
    #[serde(rename = "sched_child_runs_first")]
    pub sched_child_runs_first: String,
    #[serde(rename = "sched_rt_period_us")]
    pub sched_rt_period_us: String,
    #[serde(rename = "sched_rt_runtime_us")]
    pub sched_rt_runtime_us: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CpuIdle {
    pub current_governor: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct Cpuset {
  pub top_app: String,
  pub foreground: String,
  pub restricted: String,
  pub system_background: String,
  pub background: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct BusDcvsPath {
  #[serde(rename = "CPUllccminPath")]
  pub cpullccmin_path: String,
  #[serde(rename = "CPUllccmaxPath")]
  pub cpullccmax_path: String,
  #[serde(rename = "CPUddrminPath")]
  pub cpuddrmin_path: String,
  #[serde(rename = "CPUddrmaxPath")]
  pub cpuddrmax_path: String,
}

impl Config {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn get_mode(&self, mode_name: &str) -> Option<&Mode> {
        match mode_name {
            "powersave" => Some(&self.powersave),
            "balance" => Some(&self.balance),
            "performance" => Some(&self.performance),
            "fast" => Some(&self.fast),
            _ => None,
        }
    }
}