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
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use log::{error, info};

// 声明子模块
pub mod db;
pub mod boot;
pub mod power;
pub mod config;
pub mod app_detect;
pub mod screen_detect;

use crate::common::SchedulerMessage;
use crate::i18n::t;

// 启动函数
pub fn start_monitor(tx: Sender<SchedulerMessage>) -> Result<(), Box<dyn Error>> {
    info!("{}", t("monitor-starting"));

    // 1. 初始化数据库
    if let Err(e) = db::init_db() {
        error!("Failed to initialize database: {}", e);
    }

    // 2. 执行开机脚本
    if let Err(e) = boot::run_boot_scripts() {
        error!("Failed to run boot scripts: {}", e);
    }

    // --- 初始化共享配置 ---
    let rules_path = config::get_rules_path();
    
    // --- 初始化配置 ---
    let initial_config = config::read_config(&rules_path) 
                            .unwrap_or_else(|e| {
                                log::warn!("[Main] Failed to read initial config: {}. Using default.", e);
                                app_detect::get_default_rules()
                            });

    let config_arc = Arc::new(Mutex::new(initial_config));
    let config_arc_clone_for_watcher = Arc::clone(&config_arc);
    let config_arc_clone_for_power = Arc::clone(&config_arc);

    // --- 初始化共享的屏幕状态 ---
    let screen_state_arc = Arc::new(Mutex::new(true));
    let screen_state_clone_for_watcher = Arc::clone(&screen_state_arc);
    let screen_state_clone_for_power = Arc::clone(&screen_state_arc);
    let screen_state_clone_for_app_detect = Arc::clone(&screen_state_arc);

    // 初始化共享的强制刷新标志
    let force_refresh_arc = Arc::new(AtomicBool::new(false));
    let force_refresh_clone_for_watcher = Arc::clone(&force_refresh_arc);

    // 3. 启动功耗监控线程
    thread::Builder::new()
        .name("power_monitor".to_string())
        .spawn(move || {
            if let Err(e) = power::power_monitoring_loop(screen_state_clone_for_power, config_arc_clone_for_power) {
                error!("Power monitoring loop failed: {}", e);
            }
        })?;

    // 3.5. 启动屏幕状态监控线程
    thread::Builder::new()
        .name("screen_watcher".to_string())
        .spawn(move || {
            if let Err(e) = screen_detect::monitor_screen_state_uevent(screen_state_clone_for_watcher) {
                error!("[Main] Screen state watcher thread failed: {}", e);
            }
        })?;

    // 4. 启动配置监控线程
    thread::Builder::new()
        .name("config_watcher".to_string())
        .spawn(move || {
            if let Err(e) = app_detect::watch_config_file(
                config_arc_clone_for_watcher,
                force_refresh_clone_for_watcher
            ) {
                error!("[Main] Config watcher thread failed: {}", e);
            }
        })?;

    // 5. 启动应用检测主循环 (阻塞)
    app_detect::app_detection_loop(
        config_arc,
        screen_state_clone_for_app_detect,
        force_refresh_arc,
        tx
    )?;

    Ok(())
}