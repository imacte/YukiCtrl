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

use super::config::{Config, Mode};
use super::utils::{self, SysPathExist};
use anyhow::Result;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::process::Command;
use std::sync::{Arc, Mutex, RwLock};
use glob::glob;

use crate::i18n::{t, t_with_args};
use crate::fluent_args; 
use std::sync::atomic::{AtomicBool, Ordering};

pub struct CpuScheduler {
    config: Arc<RwLock<Config>>,
    current_mode_name: Arc<Mutex<String>>,
    sys_path_exist: Arc<SysPathExist>,
    is_boosting: Arc<AtomicBool>,
}

impl CpuScheduler {
    pub fn new(
        config: Arc<RwLock<Config>>,
        initial_mode: Arc<Mutex<String>>,
        sys_path_exist: Arc<SysPathExist>,
        is_boosting: Arc<AtomicBool>,
    ) -> Self {
        Self {
            config,
            current_mode_name: initial_mode,
            sys_path_exist,
            is_boosting,
        }
    }

    /// 获取当前激活的 Mode 配置的克隆
    fn get_current_mode(&self) -> Result<Mode> {
        let config_lock = self.config.read().unwrap();
        let mode_name_lock = self.current_mode_name.lock().unwrap();
        config_lock
            .get_mode(&mode_name_lock)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Mode '{}' not found!", *mode_name_lock))
    }

    /// 应用所有与当前性能模式相关的设置
    pub fn apply_all_settings(&self) -> Result<()> {
        if self.is_boosting.load(Ordering::SeqCst) {
            log::info!("{}", t("boost-active-skipping-apply-all-settings"));
            return Ok(());
        }

        let mode_name = self.current_mode_name.lock().unwrap().clone();

        // 在函数开头判断是否为特殊的 "fas" 模式
        if mode_name == "fas" {
            log::info!("{}", t("fas-detected"));
            return Ok(());
        }
        
        let current_mode = self.get_current_mode()?;

        log::info!("{}", t_with_args(
            "apply-settings-for-mode",
            &fluent_args!{"mode" => mode_name.as_str()}
        ));
            
        self.disable_feas()?;
        // 将获取到的 current_mode 作为参数传递下去
        self.apply_uclamp(&current_mode)?;
        self.apply_governor(&current_mode)?;
        self.apply_frequencies(&current_mode)?;
        self.apply_bus_dcvs(&current_mode)?;

            // 正确地从 current_mode 中访问 `other`
        if self.sys_path_exist.hi6220_ufs_exist {
            let _ = utils::try_write_file(
                "/sys/bus/platform/devices/hi6220-ufs/ufs_clk_gate_disable",
                current_mode.other.ufs_clk_gate.to_string(),
            );
        }

        if mode_name == "fast" {
            self.enable_feas()?;
        }

        log::info!("{}", t_with_args(
            "settings-applied-success",
            &fluent_args!{"mode" => mode_name.as_str()}
        ));
        Ok(())
    }

    /// 应用所有一次性的、与模式无关的系统调整
    pub fn apply_system_tweaks(&self) -> Result<()> {
        self.load_balancing()?;
        self.apply_cpuset()?;
        self.apply_cpu_idle_governor()?;
        self.apply_io_settings()?;
        self.apply_cfs_scheduler()?;
        self.apply_eas_scheduler()?;
        self.thread_core_allocation()?;
        Ok(())
    }

    fn enable_feas(&self) -> Result<()> {
        let config = self.config.read().unwrap();
        if self.sys_path_exist.qcom_feas_exist && config.function.enable_feas {
            let _ = utils::try_write_file("/sys/module/perfmgr/parameters/perfmgr_enable", "1");
        }
        if self.sys_path_exist.mtk_feas_exist && config.function.enable_feas {
            let _ = utils::try_write_file("/sys/module/mtk_fpsgo/parameters/perfmgr_enable", "1");
        }
        Ok(())
    }

    fn disable_feas(&self) -> Result<()> {
        if self.sys_path_exist.qcom_feas_exist {
            let _ = utils::try_write_file("/sys/module/perfmgr/parameters/perfmgr_enable", "0");
        }
        if self.sys_path_exist.mtk_feas_exist {
            let _ = utils::try_write_file("/sys/module/mtk_fpsgo/parameters/perfmgr_enable", "0");
        }
        Ok(())
    }

    fn apply_bus_dcvs(&self, current_mode: &Mode) -> Result<()> {
        let bus_paths = &self.config.read().unwrap().bus_dcvs_path;
        let bus_config = &current_mode.bus_dcvs;

        let settings_to_apply = [
            (&bus_paths.cpullccmin_path, &bus_config.cpullccmin),
            (&bus_paths.cpullccmax_path, &bus_config.cpullccmax),
            (&bus_paths.cpuddrmin_path, &bus_config.cpuddrmin),
            (&bus_paths.cpuddrmax_path, &bus_config.cpuddrmax),
        ];

        for (path, value) in settings_to_apply {
            if !path.is_empty() && !value.is_empty() {
                let _ = utils::try_write_file(path, value);
            }
        }
        Ok(())
    }

    fn apply_uclamp(&self, current_mode: &Mode) -> Result<()> {
        let uclamp = &current_mode.uclamp;
        if self.sys_path_exist.cpuctl_top_app_exist {
            let _ = utils::try_write_file("/dev/cpuctl/top-app/cpu.uclamp.min", &uclamp.uclamp_top_app_min);
            let _ = utils::try_write_file("/dev/cpuctl/top-app/cpu.uclamp.max", &uclamp.uclamp_top_app_max);
            let _ = utils::try_write_file("/dev/cpuctl/top-app/cpu.uclamp.latency_sensitive", &uclamp.uclamp_top_app_latency_sensitive);
        }
        if self.sys_path_exist.cpuctl_foreground_exist {
            let _ = utils::try_write_file("/dev/cpuctl/foreground/cpu.uclamp.min", &uclamp.uclamp_fore_ground_min);
            let _ = utils::try_write_file("/dev/cpuctl/foreground/cpu.uclamp.max", &uclamp.uclamp_fore_ground_max);
        }
        if self.sys_path_exist.cpuctl_background_exist {
            let _ = utils::try_write_file("/dev/cpuctl/background/cpu.uclamp.min", &uclamp.uclamp_back_ground_min);
            let _ = utils::try_write_file("/dev/cpuctl/background/cpu.uclamp.max", &uclamp.uclamp_back_ground_max);
        }
        Ok(())
    }

    fn apply_governor(&self, current_mode: &Mode) -> Result<()> {
        // 注意：gov_settings 来自参数 current_mode，config 来自 self.config
        let gov_settings = &current_mode.governor;
        let config = self.config.read().unwrap();
        let core_info = &config.core_framework;

        let cores_to_process = [
            (core_info.small_core_path, if !gov_settings.small_core.is_empty() { &gov_settings.small_core } else { &gov_settings.global }, "SmallCore"),
            (core_info.medium_core_path, if !gov_settings.medium_core.is_empty() { &gov_settings.medium_core } else { &gov_settings.global }, "MediumCore"),
            (core_info.big_core_path, if !gov_settings.big_core.is_empty() { &gov_settings.big_core } else { &gov_settings.global }, "BigCore"),
            (core_info.super_big_core_path, if !gov_settings.super_big_core.is_empty() { &gov_settings.super_big_core } else { &gov_settings.global }, "SuperBigCore"),
        ];

        for (core_path, governor, core_name) in cores_to_process {
            if core_path == -1 { continue; }
            let path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_governor", core_path);
            let _ = utils::try_write_file(path, governor);
            self.apply_gov_sets(current_mode, core_path, core_name, governor)?;
        }
        Ok(())
    }
    
    fn apply_gov_sets(
        &self,
        current_mode: &Mode,
        core_policy_id: i32,
        core_type_str: &str,
        governor_name: &str,
    ) -> Result<()> {
        let config = self.config.read().unwrap();
        let Some(settings_for_this_gov) = current_mode.govsets.get(governor_name) else {
            return Ok(());
        };
        let Some(paths_for_this_gov) = config.p_gov_path.get(governor_name) else {
            return Ok(());
        };
        for (path_alias, core_map) in settings_for_this_gov {
            let Some(value_to_set) = core_map.get(core_type_str) else {
                continue;
            };
            let Some(filename) = paths_for_this_gov.get(path_alias) else {
                continue;
            };
            if value_to_set.is_empty() || filename.is_empty() {
                continue;
            }
            let final_path = format!(
                "/sys/devices/system/cpu/cpufreq/policy{}/{}/{}",
                core_policy_id, governor_name, filename
            );

            if std::path::Path::new(&final_path).exists() {
                let _ = utils::try_write_file(&final_path, value_to_set);
            }
        }

        Ok(())
    }

  fn apply_frequencies(&self, current_mode: &Mode) -> Result<()> {
        if self.is_boosting.load(Ordering::SeqCst) {
            log::info!("{}", t("boost-active-skipping-apply-frequencies"));
            return Ok(()); // 跳过，因为加速循环会写入频率
        }

        let freq_settings = &current_mode.freq;
        let core_info = &self.config.read().unwrap().core_framework;

        let set_frequency = |core_path_id: i32, min_freq: u32, max_freq: u32, _core_name: &str| -> Result<()> {
            if core_path_id == -1 { return Ok(()); }
            let final_min_freq = std::cmp::min(min_freq, max_freq);
            
            let min_path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_min_freq", core_path_id);
            let max_path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_max_freq", core_path_id);
            
            let _ = utils::try_write_file(max_path, max_freq.to_string());
            let _ = utils::try_write_file(min_path, final_min_freq.to_string());
            Ok(())
        };

        set_frequency(core_info.small_core_path, freq_settings.small_core_min_freq, freq_settings.small_core_max_freq, "Small")?;
        set_frequency(core_info.medium_core_path, freq_settings.medium_core_min_freq, freq_settings.medium_core_max_freq, "Medium")?;
        set_frequency(core_info.big_core_path, freq_settings.big_core_min_freq, freq_settings.big_core_max_freq, "Big")?;
        set_frequency(core_info.super_big_core_path, freq_settings.super_big_core_min_freq, freq_settings.super_big_core_max_freq, "SuperBig")?;
        Ok(())
    }

    pub fn app_launch_boost_loop(&self) -> ! {
        loop {
            if let Err(e) = utils::watch_path("/dev/cpuset/top-app/cgroup.procs") {
                log::error!("{}", t_with_args("app-launch-watch-failed", &fluent_args!("error" => e.to_string())));
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
            log::info!("{}", t("applaunch-detected-boosting-frequencies"));

            // 1. 在开启加速状态前，先记录当前的模式名称
            let mode_name_before = self.current_mode_name.lock().unwrap().clone();

            if mode_name_before == "fas" {
                continue;
            }

            // 2. 设置加速状态
            self.is_boosting.store(true, Ordering::SeqCst);

            match self.get_current_mode() { 
                Ok(current_mode_before_boost) => {
                    let config_lock = self.config.read().unwrap();
                    let boost_multiplier = config_lock.app_launch_boost_settings.freq_multi;
                    
                    // 3. 计算并应用加速频率
                    let boosted_small_max = (current_mode_before_boost.freq.small_core_max_freq as f32 * boost_multiplier) as u32;
                    let boosted_medium_max = (current_mode_before_boost.freq.medium_core_max_freq as f32 * boost_multiplier) as u32;
                    let boosted_big_max = (current_mode_before_boost.freq.big_core_max_freq as f32 * boost_multiplier) as u32;
                    let boosted_super_big_max = (current_mode_before_boost.freq.super_big_core_max_freq as f32 * boost_multiplier) as u32;

                    if let Err(e) = self.set_max_cpu_freq_boost(boosted_small_max, boosted_medium_max, boosted_big_max, boosted_super_big_max) {
                        log::error!("{}", t_with_args("boost-apply-failed", &fluent_args!("error" => e.to_string())));
                    }

                    // 4. 等待加速
                    let boost_duration = config_lock.app_launch_boost_settings.boost_rate_ms;
                    drop(config_lock); 
                    
                    std::thread::sleep(std::time::Duration::from_millis(boost_duration));
                    log::info!("{}", t("boost-finished-restoring-settings"));
                    
                    // 5. 清除加速状态
                    self.is_boosting.store(false, Ordering::SeqCst);

                    // 6. 关键修改：获取当前的模式名称，并与加速前的进行对比
                    let mode_name_after = self.current_mode_name.lock().unwrap().clone();

                    if mode_name_before == mode_name_after {
                        // 情况 A: 模式没有改变
                        if let Err(e) = self.apply_frequencies(&current_mode_before_boost) {
                            log::error!("{}", t_with_args("boost-restore-freq-failed", &fluent_args!("error" => e.to_string())));
                        }
                    } else {
                        // 情况 B: 模式在加速期间发生了改变
                        log::info!("{}", t_with_args("boost-mode-changed", &fluent_args!(
                            "old" => mode_name_before.clone(), "new" => mode_name_after.as_str()
                        )));
                        if let Err(e) = self.apply_all_settings() {
                            log::error!("{}", t_with_args("boost-mode-apply-failed", &fluent_args!("error" => e.to_string())));
                        }
                    }
                }
                Err(e) => {
                    log::error!("{}", t_with_args("boost-get-mode-failed", &fluent_args!("error" => e.to_string())));
                    self.is_boosting.store(false, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
    }

    fn set_max_cpu_freq_boost(&self, small_freq: u32, medium_freq: u32, big_freq: u32, super_big_freq: u32) -> Result<()> {
        let core_info = &self.config.read().unwrap().core_framework;
        let cores_to_boost = [
            (core_info.small_core_path, small_freq),
            (core_info.medium_core_path, medium_freq),
            (core_info.big_core_path, big_freq),
            (core_info.super_big_core_path, super_big_freq),
        ];

        for (core_path, freq) in cores_to_boost {
            if core_path != -1 {
                let path = format!("/sys/devices/system/cpu/cpufreq/policy{}/scaling_max_freq", core_path);
                let _ = utils::try_write_file(path, freq.to_string());
            }
        }
        Ok(())
    }

    // 注意：以下所有函数都修改为返回 Result<()>
    fn load_balancing(&self) -> Result<()> {
        let config = self.config.read().unwrap();
        if config.function.load_balancing {
            if self.sys_path_exist.cpuset_top_app_exist {
                let _ = utils::try_write_file("/dev/cpuset/top-app/sched_relax_domain_level", "0");
                let _ = utils::try_write_file("/dev/cpuset/top-app/sched_load_balance", "0");
                //let _ = utils::try_write_file("/dev/cpuset/top-app/memory_migrate", "1");
                let _ = utils::try_write_file("/dev/cpuset/top-app/memory_migrate", "0");
            }
            if self.sys_path_exist.cpuset_foreground_exist {
                let _ = utils::try_write_file("/dev/cpuset/foreground/sched_relax_domain_level", "1");
                let _ = utils::try_write_file("/dev/cpuset/foreground/sched_load_balance", "1");
                let _ = utils::try_write_file("/dev/cpuset/foreground/memory_migrate", "0");
            }
            if self.sys_path_exist.cpuset_root_exist { // /dev/cpuset/
                let _ = utils::try_write_file("/dev/cpuset/sched_relax_domain_level", "1");
                let _ = utils::try_write_file("/dev/cpuset/sched_load_balance", "1");
                let _ = utils::try_write_file("/dev/cpuset/memory_migrate", "1");
            }
            if self.sys_path_exist.cpuset_background_exist {
                let _ = utils::try_write_file("/dev/cpuset/background/sched_relax_domain_level", "1");
                let _ = utils::try_write_file("/dev/cpuset/background/sched_load_balance", "1");
                let _ = utils::try_write_file("/dev/cpuset/background/memory_migrate", "0");
            }
            if self.sys_path_exist.cpuset_system_background_exist {
                let _ = utils::try_write_file("/dev/cpuset/system-background/sched_relax_domain_level", "1");
                let _ = utils::try_write_file("/dev/cpuset/system-background/sched_load_balance", "1");
                let _ = utils::try_write_file("/dev/cpuset/system-background/memory_migrate", "0");
            }
        }
        log::info!("{}", t("load-balancing-start"));
        Ok(())
    }

    fn apply_cpuset(&self) -> Result<()> {
        let config = self.config.read().unwrap();
        if config.function.cpuset {
            if self.sys_path_exist.cpuset_top_app_exist {
                let _ = utils::try_write_file("/dev/cpuset/top-app/cpus", &config.cpu_set.top_app);
            }
            if self.sys_path_exist.cpuset_foreground_exist {
                let _ = utils::try_write_file("/dev/cpuset/foreground/cpus", &config.cpu_set.foreground);
            }
            if self.sys_path_exist.cpuset_background_exist {
                let _ = utils::try_write_file("/dev/cpuset/background/cpus", &config.cpu_set.background);
            }
            if self.sys_path_exist.cpuset_system_background_exist {
                let _ = utils::try_write_file("/dev/cpuset/system-background/cpus", &config.cpu_set.system_background);
            }
            if self.sys_path_exist.cpuset_restricted_exist {
                let _ = utils::try_write_file("/dev/cpuset/restricted/cpus", &config.cpu_set.restricted);
            }
        }
        log::info!("{}", t("apply-cpuset-start"));
        Ok(())
    }

    fn apply_cpu_idle_governor(&self) -> Result<()> {
        let config = self.config.read().unwrap();
        if config.function.cpu_idle_scaling_governor && !config.cpu_idle.current_governor.is_empty() {
            if self.sys_path_exist.cpuidle_governor_exist {
                let _ = utils::try_write_file("/sys/devices/system/cpu/cpuidle/current_governor", &config.cpu_idle.current_governor);
            }
        }
        log::info!("{}",t("apply-cpu-idle-governor-start"));
        Ok(())
    }

    fn apply_glob_setting(pattern: &str, value: &str) {
        // 使用 expect 在这里是可以接受的，因为模式是硬编码的，如果出错说明程序本身有问题
        for entry in glob(pattern).expect("Failed to read glob pattern") {
            if let Ok(path) = entry {
                let _ = utils::try_write_file(&path, value);
            }
        }
    }

    fn apply_io_settings(&self) -> Result<()> {
        let config = self.config.read().unwrap();
        if config.function.adj_i_o_scheduler && !config.io_settings.scheduler.is_empty() {
            let _ = utils::try_write_file("/sys/block/sda/queue/scheduler", &config.io_settings.scheduler);
        }
        
        if config.io_settings.io_optimization {
            Self::apply_glob_setting("/sys/block/*/queue/iostats", "0");
            Self::apply_glob_setting("/sys/block/*/queue/nomerges", "0");
        }
        log::info!("{}", t("apply-io-settings-start"));
        Ok(())
    }

    fn apply_cfs_scheduler(&self) -> Result<()> {
        let config = self.config.read().unwrap();
        let cfs = &config.completely_fair_scheduler_value;

        // 1. 只有当 sched_child_runs_first 不为空时才写入
        if !cfs.sched_child_runs_first.is_empty() {
            let _ = utils::try_write_file_no_perm("/proc/sys/kernel/sched_child_runs_first", &cfs.sched_child_runs_first);
        }

        // 2. 只有当 sched_rt_period_us 不为空时才写入
        if !cfs.sched_rt_period_us.is_empty() {
            let _ = utils::try_write_file_no_perm("/proc/sys/kernel/sched_rt_period_us", &cfs.sched_rt_period_us);
        }

        // 3. 只有当 sched_rt_runtime_us 不为空时才写入
        if !cfs.sched_rt_runtime_us.is_empty() {
            let _ = utils::try_write_file_no_perm("/proc/sys/kernel/sched_rt_runtime_us", &cfs.sched_rt_runtime_us);
        }

        Ok(())
    }

    fn apply_eas_scheduler(&self) -> Result<()> {
        let config = self.config.read().unwrap();

        match config.function.eas_scheduler {
            true => {
                let _ = utils::try_write_file_no_perm("/proc/sys/kernel/sched_energy_aware", "1");
                log::info!("{}", t("attempted-to-enable-eas-scheduler-settings"));
            },
            false => {
                let _ = utils::try_write_file_no_perm("/proc/sys/kernel/sched_energy_aware", "0");
                log::info!("{}", t("attempted-to-disable-eas-scheduler"));
            },
        }
        Ok(())
    }

    fn mount_cpuset_and_cpuctl(&self) -> Result<()> {
        let config = self.config.read().unwrap();
        fs::DirBuilder::new().mode(0o666).recursive(true).create("/dev/cpuset/top-app/yuki")?;
        utils::write_to_file("/dev/cpuset/top-app/yuki/cpus", &config.core_allocation.cpu_set_core)?;
        utils::write_to_file("/dev/cpuset/top-app/yuki/mems", "0")?;

        fs::DirBuilder::new().mode(0o666).recursive(true).create("/dev/cpuset/Rubbish")?;
        utils::write_to_file("/dev/cpuset/Rubbish/cpus", "1-2")?;
        utils::write_to_file("/dev/cpuset/Rubbish/mems", "0")?;
        
        fs::DirBuilder::new().mode(0o666).recursive(true).create("/dev/cpuctl/yuki")?;
        utils::write_to_file("/dev/cpuctl/yuki/cpu.uclamp.min", "0")?;
        utils::write_to_file("/dev/cpuctl/yuki/cpu.uclamp.max", "max")?;

        Ok(())
    }

    // 辅助函数，用于执行 pidof 并返回 Option<Vec<u8>>
    fn get_pid_for_process(process_name: &str) -> Result<Option<Vec<u8>>> {
        let output = match Command::new("pidof").arg(process_name).output() {
            Ok(out) => out,
            Err(e) => {
                log::error!("{}", t_with_args("pidof-failed", &fluent_args!("name" => process_name, "error" => e.to_string())));
                return Err(e.into());
            }
        };

        if output.status.success() && !output.stdout.is_empty() {
            Ok(Some(output.stdout))
        } else {
            log::debug!("{}", t_with_args("process-not-found", &fluent_args!("name" => process_name)));
            Ok(None)
        }
    }

    fn adj_system_process_cpuctl() -> Result<()> {
        const PROCESS_NAMES: &[&str] = &["surfaceflinger", "system_server", "android:ui", "providers.media"]; //去掉"com.android.systemui"防止hyperOS3重启问题 
        for &process_name in PROCESS_NAMES {
            if let Ok(Some(pid_bytes)) = Self::get_pid_for_process(process_name) {
                if let Err(e) = utils::write_to_file("/dev/cpuset/top-app/yuki/cgroup.procs", &pid_bytes) {
                    log::warn!("{}", t_with_args("cpuset-write-failed", &fluent_args!("name" => process_name, "error" => e.to_string())));
                }
                if let Err(e) = utils::write_to_file("/dev/cpuctl/yuki/cgroup.procs", &pid_bytes) {
                    log::warn!("{}", t_with_args("cpuctl-write-failed", &fluent_args!("name" => process_name, "error" => e.to_string())));
                }
            }

        }
        Ok(())
    }

    fn rubbish_process() -> Result<()> {
        const PROCESS_NAMES: &[&str] = &["kswapd0", "kcompactd0", "init", "logcat", "mdnsd", "magiskd", "zygiskd"];
        for &process_name in PROCESS_NAMES {
            if let Ok(Some(pid_bytes)) = Self::get_pid_for_process(process_name) {
                if let Err(e) = utils::write_to_file("/dev/cpuset/Rubbish/cgroup.procs", &pid_bytes) {
                    log::warn!("{}", t_with_args("cpuset-write-failed", &fluent_args!("name" => process_name, "error" => e.to_string())));
                }
            }
        }
        Ok(())
    }

    fn thread_core_allocation(&self) -> Result<()> {
        let config = self.config.read().unwrap();
        if config.function.affinity_setter {
            self.mount_cpuset_and_cpuctl()?;
            Self::adj_system_process_cpuctl()?;
            Self::rubbish_process()?;
        }
        log::info!("{}", t("thread-core-allocation-log"));
        Ok(())
    }
}