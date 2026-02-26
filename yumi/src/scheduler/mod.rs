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
use std::time::Instant;
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
    // FAS 挂起标志：让 boost 线程感知 FAS 暂停状态
    let fas_suspended = Arc::new(AtomicBool::new(false));

    // 3. 启动 AppLaunchBoost 线程
    if shared_config.read().unwrap().function.app_launch_boost {
        let config_clone = shared_config.clone();
        let mode_clone = shared_mode_name.clone();
        let sys_path_clone = sys_path_exist.clone();
        let boost_clone = is_boosting.clone();
        let fas_suspended_clone = fas_suspended.clone();
        
        thread::Builder::new()
            .name("applaunch_boost".to_string())
            .spawn(move || {
                let scheduler = CpuScheduler::new(config_clone, mode_clone, sys_path_clone, boost_clone, fas_suspended_clone);
                scheduler.app_launch_boost_loop();
            })?;
        
        log::info!("{}", t("appLaunchboost-thread-created"));
    }

    // 4. 启动 Config Watcher
    let config_clone = shared_config.clone();
    let mode_clone = shared_mode_name.clone();
    let sys_path_clone = sys_path_exist.clone();
    let boost_clone = is_boosting.clone();
    let fas_suspended_clone = fas_suspended.clone();
    
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
                        
                        let scheduler = CpuScheduler::new(config_clone.clone(), mode_clone.clone(), sys_path_clone.clone(), boost_clone.clone(), fas_suspended_clone.clone());
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
    let fas_suspended_clone = fas_suspended.clone();

    thread::Builder::new()
        .name("scheduler_ipc".to_string())
        .spawn(move || {
            log::info!("{}", t("scheduler-ipc-started"));
            
            let root = common::get_module_root();
            let mode_file_path = root.join("current_mode.txt");
            
            // 初始化 FAS 控制器 (现在是空的，等待动态加载)
            let mut fas_controller = crate::fas::fas::FasController::new();

            let mut fas_suspended_at: Option<Instant> = None;
            let mut fas_suspended_package = String::new();
            const FAS_SUSPEND_GRACE_SECS: u64 = 5;

            let apply_static_mode = |config: &Arc<RwLock<Config>>,
                                      mode: &Arc<Mutex<String>>,
                                      sys_path: &Arc<utils::SysPathExist>,
                                      boost: &Arc<AtomicBool>,
                                      fas_sus: &Arc<AtomicBool>| {
                let scheduler = CpuScheduler::new(
                    config.clone(),
                    mode.clone(),
                    sys_path.clone(),
                    boost.clone(),
                    fas_sus.clone(),
                );
                if let Err(e) = scheduler.apply_all_settings() {
                    log::error!("{}", t_with_args("scheduler-apply-failed", &fluent_args!("error" => e.to_string())));
                }
            };
            
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

                            // ===== 进入 FAS 模式 =====
                            if mode == "fas" {
                                // 检查是否可以从挂起状态恢复
                                let can_resume = if let Some(suspended_at) = fas_suspended_at {
                                    let elapsed = suspended_at.elapsed().as_secs();
                                    let same_pkg = fas_suspended_package == package_name;
                                    let within_grace = elapsed < FAS_SUSPEND_GRACE_SECS;
                                    let has_policies = !fas_controller.policies.is_empty();
                                    same_pkg && within_grace && has_policies
                                } else {
                                    false
                                };

                                if can_resume {
                                    // 恢复路径：FAS 控制器保留了所有内存状态
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                    fas_suspended_clone.store(false, Ordering::SeqCst);
                                    for policy in &mut fas_controller.policies {
                                        policy.force_reapply();
                                    }
                                    log::info!("FAS: ♻️ resumed from suspend (pkg={}, policies intact, sysfs reapplied)",
                                        package_name);
                                } else {
                                    // 全量初始化路径：首次进入或宽限期已过
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                    fas_suspended_clone.store(false, Ordering::SeqCst);
                                    let config_lock = config_clone.read().unwrap();
                                    fas_controller.load_policies(&config_lock);
                                    log::info!("Entered FAS mode, FAS controller is now taking over CPU frequencies.");
                                }
                            }
                            // ===== 离开 FAS 模式，进入静态模式 =====
                            else {
                                if fas_suspended_at.is_some() {
                                    // 之前已在 suspend 状态，现在又收到新的非 FAS 模式切换
                                    // 说明不再需要保留 FAS 状态，直接清理
                                    log::info!("FAS: clearing stale suspend state before applying static mode");
                                    fas_controller.policies.clear();
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                    fas_suspended_clone.store(false, Ordering::SeqCst);
                                }

                                if old_mode == "fas" && !fas_controller.policies.is_empty() {
                                    // [FIX] 刚离开 FAS：挂起内存状态以便快速恢复，
                                    // 但不再 continue —— 必须继续往下走应用静态模式！
                                    fas_suspended_at = Some(Instant::now());
                                    fas_suspended_package = package_name.clone();
                                    fas_suspended_clone.store(true, Ordering::SeqCst);
                                    log::info!("FAS: ⏸️ suspended (pkg={}, grace={}s, in-memory state preserved)",
                                        package_name, FAS_SUSPEND_GRACE_SECS);
                                    // 注意：这里不再 continue，而是继续往下执行静态调度！
                                } else if old_mode == "fas" {
                                    // old_mode == "fas" 但 policies 已经为空
                                    fas_controller.policies.clear();
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                    fas_suspended_clone.store(false, Ordering::SeqCst);
                                }

                                if boost_clone.load(Ordering::SeqCst) {
                                    log::info!("{}", t("scheduler-boost-active-ignore"));
                                    continue;
                                }

                                // 常规模式，走静态调度
                                apply_static_mode(
                                    &config_clone,
                                    &mode_clone,
                                    &sys_path_clone,
                                    &boost_clone,
                                    &fas_suspended_clone,
                                );
                            }
                        }
                    },
                    crate::common::DaemonEvent::FrameUpdate { package_name: _, fps: _, frame_delta_ns } => {
                        let current_mode = mode_clone.lock().unwrap().clone();
                        if current_mode == "fas" {
                            fas_controller.update_frame(frame_delta_ns);
                        }
                    }
                }

                if let Some(suspended_at) = fas_suspended_at {
                    if suspended_at.elapsed().as_secs() >= FAS_SUSPEND_GRACE_SECS {
                        log::info!("FAS: ⏹️ suspend grace expired, clearing FAS in-memory state");
                        fas_controller.policies.clear();
                        fas_suspended_at = None;
                        fas_suspended_package.clear();
                        fas_suspended_clone.store(false, Ordering::SeqCst);
                        // 注意：此时 sysfs 已经在 suspend 开始时被静态调度覆盖过了，无需再次应用
                    }
                }
            }
            log::warn!("{}", t("scheduler-channel-closed"));
        })?;

    Ok(())
}