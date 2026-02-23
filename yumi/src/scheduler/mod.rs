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

use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
pub mod config;
pub mod scheduler;
use crate::i18n::{t, load_language, t_with_args};
use crate::fluent_args; 
use crate::utils; 
use crate::common::DaemonEvent; 
use config::Config;
use scheduler::CpuScheduler;
use crate::logger;
use crate::common;

pub fn start_scheduler_thread(rx: mpsc::Receiver<DaemonEvent>) -> Result<()> {
    // 获取动态路径
    let root = common::get_module_root();
    let config_path = root.join("config/config.yaml");
    let config_dir = root.join("config"); 

    // 1. 加载配置
    let config = Config::from_file(config_path.to_str().unwrap()).unwrap_or_default();

    // 2. 初始化共享状态
    let shared_config = Arc::new(RwLock::new(config));
    let shared_mode_name = Arc::new(Mutex::new("balance".to_string())); 
    let sys_path_exist = Arc::new(utils::SysPathExist::new());
    let is_boosting = Arc::new(AtomicBool::new(false));

    // 3. 启动 AppLaunchBoost 线程
    if shared_config.read().unwrap().function.app_launch_boost {
        let config_clone = shared_config.clone();
        let mode_clone = shared_mode_name.clone();
        let sys_path_clone = sys_path_exist.clone();
        let boost_clone = is_boosting.clone();
        
        thread::Builder::new()
            .name("applaunch_boost".to_string())
            .spawn(move || {
                let scheduler = CpuScheduler::new(config_clone, mode_clone, sys_path_clone, boost_clone);
                scheduler.app_launch_boost_loop();
            })?;
        
        log::info!("{}", t("appLaunchboost-thread-created"));
    }

    // 4. 启动 Config Watcher
    let config_clone = shared_config.clone();
    let mode_clone = shared_mode_name.clone();
    let sys_path_clone = sys_path_exist.clone();
    let boost_clone = is_boosting.clone();
    
    thread::Builder::new()
        .name("config_watcher".to_string())
        .spawn(move || {
            loop {
                if let Err(e) = utils::watch_path(&config_dir) {
                    log::error!("{}", t_with_args("config-watch-error", &fluent_args!("error" => e.to_string())));
                    continue;
                }
                log::info!("{}", t("config-reloading"));

                let old_lang = config_clone.read().unwrap().meta.language.clone();
                
                match Config::from_file(config_path.to_str().unwrap()) {
                    Ok(new_config) => {
                        logger::update_level(&new_config.meta.loglevel);
                        *config_clone.write().unwrap() = new_config;
                        
                        let new_lang = config_clone.read().unwrap().meta.language.clone();
                        if old_lang != new_lang {
                            load_language(&new_lang);
                        }

                        log::info!("{}", t("config-reloaded-success"));

                        if boost_clone.load(Ordering::SeqCst) {
                            log::info!("{}", t("boost-active-defer-config-apply"));
                            continue; 
                        }
                        
                        let scheduler = CpuScheduler::new(config_clone.clone(), mode_clone.clone(), sys_path_clone.clone(), boost_clone.clone());
                        if let Err(e) = scheduler.apply_all_settings() {
                            log::error!("{}", t_with_args("config-apply-mode-failed", &fluent_args!("error" => e.to_string())));
                        }
                        if let Err(e) = scheduler.apply_system_tweaks() {
                            log::error!("{}", t_with_args("config-apply-tweaks-failed", &fluent_args!("error" => e.to_string())));
                        }
                    }
                    Err(load_err) => {
                        log::error!("{}", t_with_args("config-reload-fail", &fluent_args!("error" => load_err.to_string())));
                    }
                }
            }
        })?;
    
    log::info!("{}", t("main-config-watch-thread-create"));

    // 5. 启动 IPC 监听线程 (Scheduler 核心循环)
    let config_clone = shared_config.clone();
    let mode_clone = shared_mode_name.clone();
    let sys_path_clone = sys_path_exist.clone();
    let boost_clone = is_boosting.clone();

    thread::Builder::new()
        .name("scheduler_ipc".to_string())
        .spawn(move || {
            log::info!("{}", t("scheduler-ipc-started"));
            
            let root = common::get_module_root();
            let mode_file_path = root.join("current_mode.txt");
            
            // 初始化 FAS 控制器 (现在是空的，等待动态加载)
            let mut fas_controller = crate::fas::fas::FasController::new();
            
            for msg in rx {
                match msg {
                    crate::common::DaemonEvent::ModeChange { package_name, mode, temperature } => {
                        let mut current_mode_lock = mode_clone.lock().unwrap();
                        let old_mode = current_mode_lock.clone();
                        
                        if old_mode != mode {
                            log::info!("{}", t_with_args("scheduler-mode-change-request", &fluent_args!(
                                "old" => old_mode.clone(), "new" => mode.as_str(), "pkg" => package_name.as_str(), "temp" => temperature
                            )));
                            
                            *current_mode_lock = mode.clone();
                            drop(current_mode_lock); 

                            if let Err(e) = utils::try_write_file(&mode_file_path, mode.as_bytes()) {
                                 log::error!("Failed to update mode.txt: {}", e);
                            }

                            // 1. 优先处理 FAS 模式的初始化，不被 Boost 拦截
                            if mode == "fas" {
                                let config_lock = config_clone.read().unwrap();
                                fas_controller.load_policies(&config_lock);
                                log::info!("Entered FAS mode, FAS controller is now taking over CPU frequencies.");
                            } else {
                                // 2. 清理 FAS 策略
                                fas_controller.policies.clear();
                                
                                // 3. 对于常规的静态模式，遇到 Boost 才跳过频率应用
                                if boost_clone.load(Ordering::SeqCst) {
                                    log::info!("{}", t("scheduler-boost-active-ignore"));
                                    continue;
                                }

                                // 常规模式，走静态调度
                                let scheduler = CpuScheduler::new(
                                    config_clone.clone(), 
                                    mode_clone.clone(), 
                                    sys_path_clone.clone(), 
                                    boost_clone.clone()
                                );
                                
                                if let Err(e) = scheduler.apply_all_settings() {
                                    log::error!("{}", t_with_args("scheduler-apply-failed", &fluent_args!("error" => e.to_string())));
                                }
                            }
                        }
                    },
                    crate::common::DaemonEvent::FrameUpdate { package_name, fps, frame_delta_ns } => {
                        //log::debug!("App: {}, Instant FPS: {:.1}", package_name, fps);
                        let current_mode = mode_clone.lock().unwrap().clone();
                        if current_mode == "fas" {
                            fas_controller.update_frame(frame_delta_ns);
                        }
                    }
                }
            }
            log::warn!("{}", t("scheduler-channel-closed"));
        })?;

    Ok(())
}