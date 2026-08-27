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
use std::fs;
use anyhow::Result;

pub mod config;
pub mod scheduler;
pub mod fas;
pub mod cpu_load_governor;
pub mod hotplug;
// Phase 2 / ticket-07: App 规则引擎 (按前台包名施加调度偏置)
pub mod app_rule;
// 全模块亮/息屏双套配置的应用层 (modules.*)
pub mod modules_ctrl;
// 任务 #5 / ticket-09: sense snapshot 写盘器 (供 WebUI 轮询)
// 注意: sensor 是顶层模块 (src/sensor/), 这里 use 而非 pub mod.
use crate::sensor::start_sense_snapshot_thread;

use crate::i18n::{t, load_language, t_with_args};
use crate::fluent_args; 
use crate::utils; 
use crate::common::DaemonEvent; 
use config::Config;
use scheduler::CpuScheduler;
use crate::logger;
use crate::common;

/// CPU 频率策略簇信息
pub struct CpuPolicy {
    pub id: i32,
    /// boost 频率列表（单位 kHz），有的簇没有此文件则为空
    pub boost_frequencies: Vec<u32>,
}

// 动态获取系统中实际可用的 CPU Policy，并读取 boost 频率
pub fn get_cpu_policies() -> Vec<CpuPolicy> {
    let mut policies = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/devices/system/cpu/cpufreq") {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("policy") {
                    if let Ok(pid) = name["policy".len()..].parse::<i32>() {
                        let boost_freqs = read_boost_frequencies(pid);
                        policies.push(CpuPolicy {
                            id: pid,
                            boost_frequencies: boost_freqs,
                        });
                    }
                }
            }
        }
    }
    policies.sort_unstable_by_key(|p| p.id);
    policies
}

fn read_boost_frequencies(pid: i32) -> Vec<u32> {
    let path = format!(
        "/sys/devices/system/cpu/cpufreq/policy{}/scaling_boost_frequencies",
        pid
    );
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect()
}

/// 通过 sysfs 探测指定 policy 的 capacity 值
pub(super) fn probe_policy_capacity(policy_id: i32) -> Option<u32> {
    let related_str = fs::read_to_string(
        format!("/sys/devices/system/cpu/cpufreq/policy{}/related_cpus", policy_id))
        .or_else(|_| fs::read_to_string(
            format!("/sys/devices/system/cpu/cpufreq/policy{}/affected_cpus", policy_id)))
        .ok()?;
    let first_cpu: u32 = related_str.split_whitespace().next()?.parse().ok()?;
    fs::read_to_string(format!("/sys/devices/system/cpu/cpu{}/cpu_capacity", first_cpu))
        .ok()?.trim().parse::<u32>().ok()
}

/// 根据 CPU capacity 自动计算每个 cluster 的权重
pub(super) fn auto_compute_capacity_weights(policies: &[CpuPolicy]) -> Option<Vec<(i32, f32)>> {
    let caps: Vec<(i32, u32)> = policies.iter()
        .filter(|p| p.id != -1)
        .filter_map(|p| probe_policy_capacity(p.id).map(|c| (p.id, c)))
        .collect();
    if caps.is_empty() || caps.iter().any(|&(_, c)| c == 0) { return None; }
    let min_cap = caps.iter().map(|&(_, c)| c).min().unwrap() as f32;
    Some(caps.iter().map(|&(pid, cap)| {
        let r = cap as f32 / min_cap;
        (pid, if r <= 1.01 { 1.0 } else { 1.0 + (r - 1.0).sqrt() })
    }).collect())
}

pub fn start_scheduler_thread(rx: mpsc::Receiver<DaemonEvent>) -> Result<()> {
    let root = common::get_module_root();
    let config_path = root.join("config/config.yaml");
    let config_dir = root.join("config"); 

    let config = Config::from_file(config_path.to_str().unwrap()).unwrap_or_default();

    let shared_config = Arc::new(RwLock::new(config));
    let shared_mode_name = Arc::new(Mutex::new("balance".to_string()));
    let sys_path_exist = Arc::new(utils::SysPathExist::new());
    // 屏幕状态共享 (gpu_boost 线程 / config watcher / modules 应用层共用);
    // 初始按亮屏处理 (保守方向, 与 scheduler_ipc 的 is_screen_on 初值一致).
    let screen_shared: Arc<RwLock<bool>> = Arc::new(RwLock::new(true));

    // 全模块双套配置: 启动时应用亮屏套 (含触摸配置推送全局快照)
    {
        let cfg_lock = shared_config.read().unwrap();
        modules_ctrl::apply_screen_scoped(&cfg_lock.modules, true);
        modules_ctrl::update_touch_global(&cfg_lock, true);
    }
    // GPU 加速线程 (boost_util_pct 驱动; 关闭时静默空转)
    modules_ctrl::spawn_gpu_boost_thread(shared_config.clone(), screen_shared.clone());

    // ==========================================
    // Config Watcher 线程
    // ==========================================
    let config_clone = shared_config.clone();
    let sys_path_clone = sys_path_exist.clone();
    let screen_for_watcher = screen_shared.clone();
    
    thread::Builder::new()
        .name("config_watcher".to_string())
        .spawn(move || {
            loop {
                if let Err(e) = utils::watch_path(&config_dir) {
                    log::error!("{}", t_with_args("config-watch-error", &fluent_args!("error" => e.to_string())));
                    // 退避后再重试，避免持续错误时忙循环刷 CPU
                    thread::sleep(std::time::Duration::from_secs(2));
                    continue;
                }
                log::info!("{}", t("config-reloading"));

                let old_lang = config_clone.read().unwrap().meta.language.clone();
                
                match Config::from_file(config_path.to_str().unwrap()) {
                    Ok(new_config) => {
                        logger::update_level(&new_config.meta.loglevel);
                        *config_clone.write().unwrap() = new_config;

                        let new_lang = config_clone.read().unwrap().meta.language.clone();
                        if old_lang != new_lang { load_language(&new_lang); }

                        log::info!("{}", t("config-reloaded-success"));

                        // 全模块双套配置热重载: 按当前屏幕状态重新应用
                        {
                            let screen_on = *screen_for_watcher.read().unwrap();
                            let cfg_lock = config_clone.read().unwrap();
                            modules_ctrl::apply_screen_scoped(&cfg_lock.modules, screen_on);
                            modules_ctrl::update_touch_global(&cfg_lock, screen_on);
                        }

                        let scheduler = CpuScheduler::new(config_clone.clone(), sys_path_clone.clone());
                        if let Err(e) = scheduler.apply_system_tweaks() {
                            log::error!("{}", t_with_args("config-apply-tweaks-failed", &fluent_args!("error" => e.to_string())));
                        }
                    }
                    Err(load_err) => log::error!("{}", t_with_args("config-reload-fail", &fluent_args!("error" => load_err.to_string()))),
                }
            }
        })?;
    
    log::info!("{}", t("main-config-watch-thread-create"));

    // ============================================================
    // Hotplug 主循环 (200ms tick, 独立线程)
    // ============================================================
    if let Err(e) = hotplug::init_state_file() {
        log::warn!("hotplug init_state_file failed: {}", e);
    }
    // Phase 2 / ticket-07: 把当前 config.app_rules 注入 hotplug 全局引擎,
    // 这样 hotplug 200ms tick 才能根据前台包名调整 off/on 阈值 (漏洞 3).
    // 注意: OnceLock 不可覆盖, 后续若 config 重载需要新的注入机制 (本任务范围外).
    {
        let cfg_lock = shared_config.read().unwrap();
        hotplug::set_global_app_rule_engine(
            app_rule::AppRuleEngine::new(cfg_lock.app_rules.clone())
        );
    }
    if let Err(e) = hotplug::start_hotplug_thread() {
        log::error!("hotplug thread start failed: {}", e);
    } else {
        log::info!("hotplug thread started");
    }

    // 任务 #5 / ticket-09: sense snapshot 写盘线程 (供 WebUI SensePanel 轮询)
    if let Err(e) = start_sense_snapshot_thread() {
        log::error!("sense snapshot thread start failed: {}", e);
    } else {
        log::info!("sense snapshot thread started");
    }

    // ==========================================
    // IPC 监听主线程 (负责所有的状态机流转与调度干预)
    // ==========================================
    let config_clone = shared_config.clone();
    let mode_clone = shared_mode_name.clone();
    let screen_for_ipc = screen_shared.clone();

    thread::Builder::new()
        .name("scheduler_ipc".to_string())
        .spawn(move || {
            log::info!("{}", t("scheduler-ipc-started"));
            
            let root = common::get_module_root();
            let mode_file_path = root.join("current_mode.txt");
            
            let mut fas_controller = crate::scheduler::fas::FasController::new();
            let mut cpu_governor = crate::scheduler::cpu_load_governor::CpuLoadGovernor::new();

            let rules_path = crate::monitor::config::get_rules_path();
            let mut current_rules = crate::utils::read_config::<crate::monitor::config::RulesConfig, _>(&rules_path).unwrap_or_default();

            // 状态机变量
            let mut fas_suspended_at: Option<Instant> = None;
            let mut fas_suspended_package = String::new();
            const FAS_SUSPEND_GRACE_SECS: u64 = 5;
            
            let mut is_screen_on = true; // 屏幕状态标记

            let temp_sensor_path = crate::utils::find_cpu_temp_path().unwrap_or_default();
            let mut last_temp_update = Instant::now();

            let get_clg_cfg = |config: &Config, mode: &str| -> crate::scheduler::config::CpuLoadGovernorConfig {
                config.get_mode(mode).map(|m| m.cpu_load_governor.clone()).unwrap_or_else(|| {
                    // 未知/空模式名：不意外启用 CLG，避免用默认参数接管 CPU
                    let mut cfg = crate::scheduler::config::CpuLoadGovernorConfig::default();
                    cfg.enabled = false;
                    cfg
                })
            };

            // 启动时初始化
            {
                let current_mode = mode_clone.lock().unwrap().clone();
                if current_mode != "fas" {
                    let config_lock = config_clone.read().unwrap();
                    let clg_cfg = get_clg_cfg(&config_lock, &current_mode);
                    if clg_cfg.enabled {
                        cpu_governor.init_policies(&clg_cfg);
                        log::info!("{}", t_with_args("scheduler-clg-init", &fluent_args!("mode" => current_mode.clone())));
                    }
                }
                // 需求: 启动默认亮屏 — 应用亮屏套帧参数
                let config_lock = config_clone.read().unwrap();
                let f = config_lock.modules.frame.pick(true);
                fas_controller.set_frame_params(f.jank_margin_ms, f.boost_enabled, f.boost_strength);
            }

            // Phase 2 / ticket-07-fix: 共享的 "应用 App 规则偏置" 流程.
            // 入口: (a) ModeChange 事件处理 (mode 变了, 顺便刷偏置);
            //       (b) AppRuleRefresh 事件处理 (mode 没变, 只刷偏置).
            // 注: hotplug 的偏置由 hotplug 自己的 run_one_tick 每次读前台包名算,
            //     此函数只负责 FAS 侧 (target_pressure). 这样 hotplug 与 FAS 解耦.
            let apply_app_rule_bias = |fas_ctl: &mut fas::controller::FasController,
                                       cfg: &Arc<RwLock<Config>>,
                                       current_mode: &str,
                                       package_name: &str,
                                       base_target: f32| {
                let bias_offset = {
                    let cfg_lock = cfg.read().unwrap();
                    app_rule::AppRuleEngine::new(cfg_lock.app_rules.clone())
                        .match_rule(package_name)
                        .map(|r| app_rule::AppRuleBias::from_rule(Some(r)))
                        .map(|b| b.target_util_offset)
                        .unwrap_or(0)
                };
                fas_ctl.set_target_pressure_with_app_bias(base_target, bias_offset);
                if bias_offset != 0 {
                    log::info!(
                        "[app_rule] mode={} pkg={} target={:.1} bias={:+}",
                        current_mode, package_name, base_target, bias_offset
                    );
                }
            };
            
            // 事件循环包在 catch_unwind 中：任何 panic 都被捕获并记录，
            // 避免调度线程静默死亡（进程存活但频率停在最后状态）
            let loop_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // 任务 #6 reliability: scheduler 主循环心跳. 每次成功处理一个事件
            // 都 tick 一次. 如果 app_detect 线程卡住但 scheduler 仍在处理事件
            // (例如收到 DaemonEvent), watchdog 仍然能收到心跳, 不会误杀.
            for msg in rx {
                crate::watchdog::heartbeat_tick();
                match msg {
                    // --- 1. 屏幕状态事件 (息屏深度睡眠) ---
                    DaemonEvent::ScreenStateChange(screen_on) => {
                        is_screen_on = screen_on;

                        // 全模块亮/息屏双套配置: 切换到对应套 (gpu/swap/io 写 sysfs;
                        // frame 由 FAS set_frame_params 消费; touch 推送全局快照)
                        //
                        // 锁序约束 (防死锁): 全进程统一 "先 config 后 screen".
                        // 此处必须先做 config 读锁内的 apply, 再更新 screen 写锁 —
                        // 若反过来 (screen 写锁内等 config 读锁), 会与 config watcher
                        // (config 写锁 → screen 读锁) 形成环, 真机观测整进程僵死.
                        {
                            let cfg_lock = config_clone.read().unwrap();
                            modules_ctrl::apply_screen_scoped(&cfg_lock.modules, screen_on);
                            modules_ctrl::update_touch_global(&cfg_lock, screen_on);
                            let f = cfg_lock.modules.frame.pick(screen_on);
                            fas_controller.set_frame_params(
                                f.jank_margin_ms, f.boost_enabled, f.boost_strength);
                        }
                        *screen_for_ipc.write().unwrap() = screen_on;

                        let current_mode = mode_clone.lock().unwrap().clone();

                        if !is_screen_on {
                            log::info!("{}", t("scheduler-doze-enable"));
                            
                            // 息屏立刻剥夺 FAS 的频率控制权
                            if current_mode == "fas" {
                                fas_controller.reset_all_freqs();
                                fas_controller.clear_game();
                                fas_controller.policies.clear();
                                fas_suspended_at = None;
                                fas_suspended_package.clear();
                            }

                            // 强行让 CLG 接管，并动态生成一个极致省电配置
                            let config_lock = config_clone.read().unwrap();
                            let mut doze_cfg = get_clg_cfg(&config_lock, "powersave"); 
                            doze_cfg.enabled = true;
                            doze_cfg.perf_floor = 0.0;
                            doze_cfg.perf_ceil = doze_cfg.perf_ceil.min(0.40); // 锁死天花板最高只给 40% 性能
                            doze_cfg.smoothing_up = 0.10;           // 升频极其迟钝
                            doze_cfg.smoothing_down = 1.0;          // 瞬间降频
                            
                            cpu_governor.init_policies(&doze_cfg);
                        } else {
                            log::info!("{}", t("scheduler-doze-restore"));
                            
                            let config_lock = config_clone.read().unwrap();
                            let clg_cfg = get_clg_cfg(&config_lock, &current_mode);
                            
                            if current_mode != "fas" {
                                if clg_cfg.enabled {
                                    // 息屏 doze 期间 CLG 仍持有 writer，热切换配置即可
                                    if cpu_governor.is_active() { cpu_governor.reload_config(&clg_cfg); } 
                                    else { cpu_governor.init_policies(&clg_cfg); }
                                } 
                                else { cpu_governor.release(); }
                            } else {
                                cpu_governor.release(); 
                                *mode_clone.lock().unwrap() = String::new();
                            }
                        }
                    },

                    // --- 2. 前台模式切换事件 ---
                    DaemonEvent::ModeChange { package_name, pid, mode, temperature } => {
                        let mut current_mode_lock = mode_clone.lock().unwrap();
                        let old_mode = current_mode_lock.clone();
                        
                        if old_mode != mode {
                            log::info!("{}", t_with_args("scheduler-mode-change-request", &fluent_args!(
                                "old" => old_mode.clone(), "new" => mode.as_str(), "pkg" => package_name.as_str(), "temp" => temperature
                            )));

                            *current_mode_lock = mode.clone();
                            drop(current_mode_lock);

                            let _ = utils::try_write_file(&mode_file_path, mode.as_bytes());

                            // Phase 2 / ticket-06: 模式切换 → 更新 fas_controller target_pressure.
                            // 幂等, FAS 接管前先用 mode 的默认值, 避免 reset_runtime 后跑空.
                            //
                            // Phase 2 / ticket-07: 若触发 ModeChange 的就是当前前台包,
                            // 叠加 App 规则偏置 (Restrict → 降低 target, Boost → 提高).
                            // ModeChange 事件的 package_name 就是触发源, 直接拿来匹配.
                            //
                            // 需求: 目标负载配置化 — config {mode}.target_load 优先,
                            // 未配置回落 mode_target_pressure() 硬编码默认.
                            let target = {
                                let cfg_lock = config_clone.read().unwrap();
                                cfg_lock.target_load_of(&mode)
                            };
                            apply_app_rule_bias(
                                &mut fas_controller,
                                &config_clone,
                                &mode,
                                &package_name,
                                target,
                            );

                            if mode == "fas" {
                                // 进游戏：释放 CLG 控制权，激活 FAS
                                cpu_governor.release();

                                let can_resume = fas_suspended_at.map_or(false, |at| {
                                    at.elapsed().as_secs() < FAS_SUSPEND_GRACE_SECS && fas_suspended_package == package_name && !fas_controller.policies.is_empty()
                                });

                                if can_resume {
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                    for policy in &mut fas_controller.policies { policy.force_reapply(); }
                                } else {
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                    fas_controller.load_policies(&current_rules.fas_rules);
                                }
                                fas_controller.set_game(pid, &package_name);
                                fas_controller.set_temperature(temperature);
                                fas_controller.set_temp_threshold(current_rules.fas_rules.core_temp_threshold);
                                // 需求: 进游戏接管时应用当前屏幕状态的帧参数套
                                if is_screen_on {
                                    let config_lock = config_clone.read().unwrap();
                                    let f = config_lock.modules.frame.pick(true);
                                    fas_controller.set_frame_params(f.jank_margin_ms, f.boost_enabled, f.boost_strength);
                                }
                            } else {
                                // 退游戏：尝试挂起 FAS，并激活普通模式
                                if fas_suspended_at.is_some() {
                                    fas_controller.reset_all_freqs();
                                    fas_controller.clear_game();
                                    fas_controller.policies.clear();
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                }

                                if old_mode == "fas" && !fas_controller.policies.is_empty() {
                                    fas_suspended_at = Some(Instant::now());
                                    fas_suspended_package = package_name.clone();
                                } else if old_mode == "fas" {
                                    fas_controller.clear_game();
                                    fas_controller.policies.clear();
                                    fas_suspended_at = None;
                                    fas_suspended_package.clear();
                                }

                                // 仅在亮屏时处理 CLG。如果息屏，Doze 配置仍在生效，这里不能覆盖它
                                if is_screen_on {
                                    let config_lock = config_clone.read().unwrap();
                                    let clg_cfg = get_clg_cfg(&config_lock, &mode);
                                    if clg_cfg.enabled {
                                        // CLG 已激活时热切换配置，避免同模式反复切换全量重建
                                        if cpu_governor.is_active() { cpu_governor.reload_config(&clg_cfg); }
                                        else { cpu_governor.init_policies(&clg_cfg); }
                                    } else {
                                        cpu_governor.release();
                                    }
                                }
                            }
                        } else if mode == "fas" {
                            fas_controller.set_temperature(temperature);
                        }
                    },

                    // --- 2.5 Phase 2 / ticket-07-fix: 前台包名变化 (与 ModeChange 解耦) ---
                    // 触发场景: 用户切换前台应用, 但 app_detect 判定模式没变 (例如两个
                    // 应用都映射到 balance 模式). ModeChange 不会发, 但 AppRule 偏置需要更新.
                    DaemonEvent::AppRuleRefresh { package_name } => {
                        if !is_screen_on { continue; } // 息屏期间不施加 App 规则
                        let current_mode = mode_clone.lock().unwrap().clone();
                        // 只对 FAS 模式应用 (非 FAS 模式靠 mode 自带的 target, App 规则
                        // 只在 FAS 调度下有意义; 非 FAS 用 CLG 完全不同的决策路径).
                        if current_mode == "fas" {
                            // 需求: 目标负载配置化 (同 ModeChange 分支)
                            let target = {
                                let cfg_lock = config_clone.read().unwrap();
                                cfg_lock.target_load_of(&current_mode)
                            };
                            apply_app_rule_bias(
                                &mut fas_controller,
                                &config_clone,
                                &current_mode,
                                &package_name,
                                target,
                            );
                        }
                        // 注: hotplug 不在此处触发 — hotplug::run_one_tick 每个 tick 自动
                        // 读前台包名重算偏置, 频率足够高, 不需要事件驱动. 事件驱动 FAS 是
                        // 因为 FAS 调频成本高, 必须精确触发才划算.
                    },

                    // --- 3. CPU 负载事件 (eBPF 驱动) ---
                    DaemonEvent::SystemLoadUpdate { core_utils, foreground_max_util } => {
                        let current_mode = mode_clone.lock().unwrap().clone();
                        // 仅当亮屏且在 FAS 模式且未挂起时，投喂 FAS
                        if is_screen_on && current_mode == "fas" && fas_suspended_at.is_none() {
                            fas_controller.update_cpu_util(foreground_max_util);
                            fas_controller.update_core_utils(&core_utils);
                            // Phase 2 / ticket-06: 顺手喂一次综合压力指数 (frame_drop_active=false,
                            // 因为这是 CPU 负载事件, 不知道 frame 状态; 真正的 frame_drop 由
                            // frame_pipeline 单独发事件喂入).
                            fas_controller.tick_pressure_index(false);
                        }
                        // 如果 CLG 处于活动状态（包含日常模式或息屏 Doze 模式），全权投喂
                        if cpu_governor.is_active() {
                            // 需求: 频率护栏 — 每 tick 按当前屏幕状态从 config.freq_limits
                            // 取值注入 (幂等). 放在这里而非 4 个激活点, config 热重载与
                            // 屏幕切换都自动覆盖, 无需额外事件.
                            let (fl_lo, fl_hi) = {
                                let cfg_lock = config_clone.read().unwrap();
                                cfg_lock.freq_limits.limits_for(is_screen_on)
                            };
                            cpu_governor.set_freq_limits(fl_lo, fl_hi);
                            cpu_governor.on_load_update(&core_utils);
                        }
                    },

                    // --- 4. 帧率事件 (eBPF 驱动) ---
                    DaemonEvent::FrameUpdate { frame_delta_ns } => {
                        if !is_screen_on { continue; } // 息屏不处理渲染帧

                        let current_mode = mode_clone.lock().unwrap().clone();
                        if current_mode == "fas" {
                            if !temp_sensor_path.is_empty() && last_temp_update.elapsed().as_secs() >= 3 {
                                if let Ok(raw_temp) = crate::utils::read_f64_from_file(&temp_sensor_path) { 
                                    fas_controller.set_temperature(raw_temp / 1000.0); 
                                }
                                last_temp_update = Instant::now();
                            }
                            fas_controller.update_frame(frame_delta_ns);
                        }
                    }

                    // --- 5. 热重载配置事件 ---
                    DaemonEvent::ConfigReload(new_rules) => {
                        current_rules = new_rules;
                        let current_mode = mode_clone.lock().unwrap().clone();
                        
                        if current_mode == "fas" {
                            if fas_controller.policies.is_empty() {
                                fas_controller.load_policies(&current_rules.fas_rules);
                            } else {
                                fas_controller.reload_rules(&current_rules.fas_rules);
                            }
                        } else if is_screen_on { // 息屏时不要用新配置覆盖 Doze
                            let config_lock = config_clone.read().unwrap();
                            let clg_cfg = get_clg_cfg(&config_lock, &current_mode);
                            if clg_cfg.enabled {
                                if cpu_governor.is_active() { cpu_governor.reload_config(&clg_cfg); } 
                                else { cpu_governor.init_policies(&clg_cfg); }
                            } else if cpu_governor.is_active() {
                                cpu_governor.release();
                            }
                        }
                    }
                }

                // 定期检查 FAS 挂起状态是否超时
                if let Some(suspended_at) = fas_suspended_at {
                    if suspended_at.elapsed().as_secs() >= FAS_SUSPEND_GRACE_SECS {
                        fas_controller.reset_all_freqs();
                        fas_controller.clear_game();
                        fas_controller.policies.clear();
                        fas_suspended_at = None;
                        fas_suspended_package.clear();
                    }
                }
            }
            }));
            if loop_result.is_err() {
                log::error!("{}", t("scheduler-ipc-panic"));
            }
            log::warn!("{}", t("scheduler-channel-closed"));
            // 收尾：无论 channel 关闭还是 panic，都恢复 CPU 控制状态，避免频率/governor 残留
            cpu_governor.release();
            fas_controller.reset_all_freqs();
            fas_controller.clear_game();
        })?;

    Ok(())
}