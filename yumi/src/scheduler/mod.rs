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
pub mod fas;
pub mod cpu_load_governor;
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
            
            // 初始化 FAS 控制器
            let mut fas_controller = crate::scheduler::fas::FasController::new();
            // 初始化 CPU 负载调频器
            let mut cpu_governor = crate::scheduler::cpu_load_governor::CpuLoadGovernor::new();
            // 将 is_boosting 标志传给 CLG，使其在 Boost 期间暂停写 sysfs
            cpu_governor.set_boost_flag(boost_clone.clone());

            let rules_path = crate::monitor::config::get_rules_path();
            let mut current_rules = crate::monitor::config::read_config::<crate::monitor::config::RulesConfig, _>(&rules_path).unwrap_or_default();

            let mut fas_suspended_at: Option<Instant> = None;
            let mut fas_suspended_package = String::new();
            const FAS_SUSPEND_GRACE_SECS: u64 = 5;

            // 温度传感器路径和定时器，用于 FAS 运行时定期更新温度
            let temp_sensor_path = crate::utils::find_cpu_temp_path().unwrap_or_default();
            let mut last_temp_update = Instant::now();
            let mut was_boosting = false;

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

            // 启动时：如果负载调频器已启用且当前不是 FAS 模式，立即初始化
            // 修复：shared_mode_name 初始值为 "balance"，首次 ModeChange 也是 "balance"，
            // 导致 old_mode != mode 判断为 false，init_policies() 永远不会被调用
            {
                let current_mode = mode_clone.lock().unwrap().clone();
                if current_mode != "fas" && current_rules.cpu_load_governor.enabled {
                    let config_lock = config_clone.read().unwrap();
                    cpu_governor.init_policies(&config_lock, &current_rules.cpu_load_governor);
                    log::info!("CPU Load Governor: initialized at startup (mode={})", current_mode);
                }
            }
            
            for msg in rx {
                match msg {
                    // ModeChange 现在携带 pid 字段
                    DaemonEvent::ModeChange { package_name, pid, mode, temperature } => {
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
                                // FAS 接管频率控制，先释放负载调频器
                                cpu_governor.release();

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
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                    fas_suspended_clone.store(false, Ordering::SeqCst);
                                    for policy in &mut fas_controller.policies {
                                        policy.force_reapply();
                                    }
                                    // 恢复时也要更新 game 和温度信息
                                    fas_controller.set_game(pid, &package_name);
                                    fas_controller.set_temperature(temperature);
                                    fas_controller.set_temp_threshold(current_rules.fas_rules.core_temp_threshold);
                                    log::info!("FAS: resumed from suspend (pkg={}, pid={}, policies intact, sysfs reapplied)",
                                        package_name, pid);
                                } else {
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                    fas_suspended_clone.store(false, Ordering::SeqCst);
                                    let config_lock = config_clone.read().unwrap();
                                    fas_controller.load_policies(&config_lock, &current_rules.fas_rules);
                                    // 初始化时必须设置 game、温度信息，否则 ProcessMonitor 和温度降频无法工作
                                    fas_controller.set_game(pid, &package_name);
                                    fas_controller.set_temperature(temperature);
                                    fas_controller.set_temp_threshold(current_rules.fas_rules.core_temp_threshold);
                                    log::info!("Entered FAS mode (pkg={}, pid={}), FAS controller is now taking over CPU frequencies.",
                                        package_name, pid);
                                }
                            }
                            // ===== 离开 FAS 模式，进入静态模式 =====
                            else {
                                if fas_suspended_at.is_some() {
                                    log::info!("FAS: clearing stale suspend state before applying static mode");
                                    fas_controller.reset_all_freqs();
                                    fas_controller.clear_game();
                                    fas_controller.policies.clear();
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                    fas_suspended_clone.store(false, Ordering::SeqCst);
                                }

                                if old_mode == "fas" && !fas_controller.policies.is_empty() {
                                    fas_suspended_at = Some(Instant::now());
                                    fas_suspended_package = package_name.clone();
                                    fas_suspended_clone.store(true, Ordering::SeqCst);
                                    log::info!("FAS: suspended (pkg={}, grace={}s, in-memory state preserved)",
                                        package_name, FAS_SUSPEND_GRACE_SECS);
                                } else if old_mode == "fas" {
                                    fas_controller.clear_game();
                                    fas_controller.policies.clear();
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                    fas_suspended_clone.store(false, Ordering::SeqCst);
                                }

                                if boost_clone.load(Ordering::SeqCst) {
                                    log::info!("{}", t("scheduler-boost-active-ignore"));
                                    continue;
                                }

                                apply_static_mode(
                                    &config_clone,
                                    &mode_clone,
                                    &sys_path_clone,
                                    &boost_clone,
                                    &fas_suspended_clone,
                                );

                                // 静态模式应用完毕后，如果负载调频器已启用则接管频率
                                if current_rules.cpu_load_governor.enabled {
                                    let config_lock = config_clone.read().unwrap();
                                    cpu_governor.init_policies(&config_lock, &current_rules.cpu_load_governor);
                                } else {
                                    cpu_governor.release();
                                }
                            }
                        } else {
                            // 即使模式没变，也更新温度（温度可能变化了）
                            if mode == "fas" {
                                fas_controller.set_temperature(temperature);
                            }
                        }
                    },
                    // 接住 CPU 负载事件
                    DaemonEvent::SystemLoadUpdate { core_utils, foreground_max_util } => {
                        // 1. 如果你在打游戏（FAS 开启状态），把最重线程的利用率喂给 FAS 算法
                        if !fas_suspended_clone.load(std::sync::atomic::Ordering::Relaxed) {
                            fas_controller.update_cpu_util(foreground_max_util);
                            // [Fix] Drive scene detection + populate core_utils for FAS
                            fas_controller.update_core_utils(&core_utils);
                        }
                        // [Fix] Detect boost end -> resync CLG frequencies
                        let boosting_now = boost_clone.load(std::sync::atomic::Ordering::Relaxed);
                        if was_boosting && !boosting_now {
                            cpu_governor.resync_after_boost();
                            log::info!("CLG: resync after app-launch-boost ended");
                        }
                        was_boosting = boosting_now;
                        // 2. 如果负载调频器处于激活状态，把各核心利用率喂给它
                        if cpu_governor.is_active() {
                            cpu_governor.on_load_update(&core_utils);
                        }
                    },
                    // FrameUpdate 不再携带 package_name
                    DaemonEvent::FrameUpdate { fps: _, frame_delta_ns } => {
                        let current_mode = mode_clone.lock().unwrap().clone();
                        if current_mode == "fas" {
                            // 每 3 秒更新一次温度（低开销，仅读 sysfs 文件）
                            if !temp_sensor_path.is_empty() && last_temp_update.elapsed().as_secs() >= 3 {
                                if let Ok(raw_temp) = crate::utils::read_f64_from_file(&temp_sensor_path) {
                                    fas_controller.set_temperature(raw_temp / 1000.0);
                                }
                                last_temp_update = Instant::now();
                            }
                            fas_controller.update_frame(frame_delta_ns);
                        }
                    }
                    // 热重载使用 reload_rules，不重建 policies，不重置运行时状态
                    DaemonEvent::ConfigReload(new_rules) => {
                        log::info!("Scheduler received config reload event. Updating in-memory rules...");
                        current_rules = new_rules;
                        
                        let current_mode = mode_clone.lock().unwrap().clone();
                        if current_mode == "fas" {
                            if fas_controller.policies.is_empty() {
                                // policies 尚未初始化，做全量加载
                                let config_lock = config_clone.read().unwrap();
                                fas_controller.load_policies(&config_lock, &current_rules.fas_rules);
                                log::info!("FAS: full policy init on config reload (was empty).");
                            } else {
                                // policies 已存在，热重载规则参数，不中断状态
                                fas_controller.reload_rules(&current_rules.fas_rules);
                                log::info!("FAS: rules hot-reloaded without resetting runtime state.");
                            }
                        } else {
                            // 非 FAS 模式：热重载负载调频器配置
                            if current_rules.cpu_load_governor.enabled {
                                if cpu_governor.is_active() {
                                    cpu_governor.reload_config(&current_rules.cpu_load_governor);
                                } else {
                                    // 刚从禁用切到启用，需全量初始化
                                    let config_lock = config_clone.read().unwrap();
                                    cpu_governor.init_policies(&config_lock, &current_rules.cpu_load_governor);
                                }
                            } else if cpu_governor.is_active() {
                                // 刚从启用切到禁用，释放并恢复静态频率
                                cpu_governor.release();
                                apply_static_mode(
                                    &config_clone, &mode_clone, &sys_path_clone,
                                    &boost_clone, &fas_suspended_clone,
                                );
                            }
                        }
                    }
                }

                if let Some(suspended_at) = fas_suspended_at {
                    if suspended_at.elapsed().as_secs() >= FAS_SUSPEND_GRACE_SECS {
                        log::info!("FAS: suspend grace expired, clearing FAS in-memory state");
                        fas_controller.reset_all_freqs();
                        fas_controller.clear_game();
                        fas_controller.policies.clear();
                        fas_suspended_at = None;
                        fas_suspended_package.clear();
                        fas_suspended_clone.store(false, Ordering::SeqCst);
                    }
                }
            }
            log::warn!("{}", t("scheduler-channel-closed"));
        })?;

    Ok(())
}