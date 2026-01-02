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

use std::error::Error;
use std::thread;
use std::time::Duration;
use chrono::Utc;
use log::{error, info, warn, debug}; 
use std::sync::{Arc, Mutex};

use super::app_detect;
use super::db;
use super::config::RulesConfig;

use crate::i18n::{t, t_with_args};
use crate::fluent_args;
use crate::utils;

const VOLTAGE_NOW_PATH: &str = "/sys/class/power_supply/battery/voltage_now";
const CURRENT_NOW_PATH: &str = "/sys/class/power_supply/battery/current_now";
const CAPACITY_PATH: &str = "/sys/class/power_supply/battery/capacity";
const STATUS_PATH: &str = "/sys/class/power_supply/battery/status";

pub fn power_monitoring_loop(
    screen_state_arc: Arc<Mutex<bool>>, 
    config_arc: Arc<Mutex<RulesConfig>>
) -> Result<(), Box<dyn Error>> {
    info!("{}", t("power-loop-started"));
    
    let temp_path = match utils::find_cpu_temp_path() {
        Ok(path) => path,
        Err(e) => {
            error!("{}", t_with_args("power-cpu-temp-not-found", &fluent_args!("error" => e.to_string())));
            "/invalid/temp/path".to_string()
        }
    };

    // 在循环外部建立数据库连接
    let db_conn = match db::open_db_connection() {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to open database connection for power monitor: {}", e);
            return Err(Box::new(e));
        }
    };

    let mut session_id = Utc::now().timestamp_millis();
    let mut last_charging = false;

    loop {
        thread::sleep(Duration::from_millis(2500));
        let is_screen_on = { *screen_state_arc.lock().unwrap() };
        if !is_screen_on {
            // 息屏时不需要高频轮询，但因为这里 sleep 在 loop 开头，所以是安全的
            continue;
        }

        match utils::read_file_content(STATUS_PATH) {
            Ok(status_str) => {
                let is_charging = status_str == "Charging" || status_str == "Full";
                if last_charging && !is_charging {
                    info!("{}", t("power-charging-stopped"));
                    let limit = { config_arc.lock().unwrap().session_log_limit };
                    
                    // 复用连接
                    if let Err(e) = db::trim_old_sessions(&db_conn, limit) {
                        warn!("{}", t_with_args("power-trim-failed", &fluent_args!("error" => e.to_string())));
                    }
                    session_id = Utc::now().timestamp_millis();
                    info!("{}", t_with_args("power-new-session", &fluent_args!("id" => session_id)));
                }
                last_charging = is_charging;

                if !is_charging {
                    match (utils::read_f64_from_file(VOLTAGE_NOW_PATH), utils::read_f64_from_file(CURRENT_NOW_PATH)) {
                        (Ok(voltage_uv), Ok(current_ua)) => {
                            let temp_c = utils::read_f64_from_file(&temp_path).unwrap_or(0.0) / 1000.0;
                            let battery_pct: i32 = utils::read_f64_from_file(CAPACITY_PATH).unwrap_or(0.0) as i32;
                            
                            let current_pkg = app_detect::get_current_package();
                            
                            // 复用连接
                            if let Err(e) = db::insert_power_log(
                                &db_conn, session_id, &current_pkg, voltage_uv, current_ua, temp_c, battery_pct,
                            ) {
                                warn!("{}", t_with_args("power-db-write-failed", &fluent_args!("error" => e.to_string())));
                            }
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            warn!("{}", t_with_args("power-read-failed", &fluent_args!("error" => e.to_string())));
                        }
                    }
                }
            }
            Err(e) => {
                warn!("{}", t_with_args("power-status-read-failed", &fluent_args!("error" => e.to_string())));
            }
        }
    }
}