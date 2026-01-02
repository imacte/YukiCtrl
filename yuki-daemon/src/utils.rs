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

use anyhow::{Result};
use log;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use nix::unistd::{access, AccessFlags};

/// 向文件写入内容，并处理可能的错误
pub fn write_to_file<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> Result<()> {
    let path = path.as_ref();

    // 尝试修改权限以便写入
    if path.exists() {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o664));
    }

    fs::write(path, content)?;
    
    // 写完后设为只读
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o444));
    Ok(())
}

pub fn write_to_file_no_perm_change<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> Result<()> {
    fs::write(path.as_ref(), content)?;
    Ok(())
}

// 尝试写入内容 (不抛出错误，只记录警告)
pub fn try_write_file<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> Result<()> {
    if let Err(e) = write_to_file(path.as_ref(), content) {
        log::warn!("Failed to write to {}: {}.", path.as_ref().display(), e);
    }
    Ok(())
}

pub fn try_write_file_no_perm<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> Result<()> {
    if let Err(e) = write_to_file_no_perm_change(path.as_ref(), content) {
        log::warn!("Failed to write to {}: {}.", path.as_ref().display(), e);
    }
    Ok(())
}

pub fn enable_perm <P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o664))?;
    }
    Ok(())
}

/// 监控指定路径的文件/目录事件
pub fn watch_path<P: AsRef<Path>>(path_to_watch: P) -> Result<()> {
    use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
    
    let inotify = Inotify::init(InitFlags::empty())?;
    inotify.add_watch(path_to_watch.as_ref(), AddWatchFlags::IN_CLOSE_WRITE)?;
    
    let _buffer = [0u8; 1024];
    let _events = inotify.read_events()?;
    
    if !_events.is_empty() {
        log::debug!("Detected change in {:?}, re-evaluating...", path_to_watch.as_ref());
    }
    Ok(())
}

// 通用的读取文件为 f64 的函数
pub fn read_f64_from_file(path: &str) -> Result<f64> {
    let mut content = String::new();
    File::open(path)?.read_to_string(&mut content)?;
    let val: f64 = content.trim().parse()?;
    Ok(val)
}

// 辅助函数：读取文件内容为 String
pub fn read_file_content(path: &str) -> Result<String> {
    let mut content = String::new();
    File::open(path)?.read_to_string(&mut content)?;
    Ok(content.trim().to_string())
}

// 查找 CPU 温度路径的逻辑
pub fn find_cpu_temp_path() -> Result<String> {
    let thermal_path = "/sys/class/thermal";
    let thermal_dir = Path::new(thermal_path);
    
    if !thermal_dir.exists() {
         return Err(anyhow::anyhow!("Thermal directory not found"));
    }

    for entry in fs::read_dir(thermal_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
                if dir_name.starts_with("thermal_zone") {
                    let type_path = path.join("type");
                    // 修复 E0532 模式匹配错误: 直接使用 if let Ok(...)
                    if let Ok(type_content) = read_file_content(type_path.to_str().unwrap_or_default()) {
                        if type_content.contains("soc_max") 
                           || type_content.contains("mtktscpu") 
                           || type_content.contains("cpu-1-") 
                           || type_content.contains("cpu-0-0-usr") {
                            
                            let temp_path = path.join("temp");
                            if temp_path.exists() {
                                return Ok(temp_path.to_str().unwrap().to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    Err(anyhow::anyhow!("Valid CPU thermal zone not found"))
}

// --- SysPathExist 结构体 ---
pub struct SysPathExist {
    pub qcom_feas_exist: bool,
    pub mtk_feas_exist: bool,
    pub walt_exist: bool,
    pub stune_exist: bool,
    pub hi6220_ufs_exist: bool,
    pub cpuctl_top_app_exist: bool,
    pub cpuctl_foreground_exist: bool,
    pub cpuctl_background_exist: bool,
    pub cpuset_top_app_exist: bool,
    pub cpuset_foreground_exist: bool,
    pub cpuset_background_exist: bool,
    pub cpuset_system_background_exist: bool,
    pub cpuset_restricted_exist: bool,
    pub cpuset_root_exist: bool,
    pub cpuidle_governor_exist: bool,
    pub sda_scheduler_exist: bool,
}

impl SysPathExist {
    pub fn new() -> Self {
        Self {
            qcom_feas_exist: Self::path_exists("/sys/module/perfmgr/parameters/perfmgr_enable"),
            mtk_feas_exist: Self::path_exists("/sys/module/mtk_fpsgo/parameters/perfmgr_enable"),
            walt_exist: Self::path_exists("/proc/sys/walt"),
            stune_exist: Self::path_exists("/dev/stune"),
            hi6220_ufs_exist: Self::path_exists("/sys/bus/platform/devices/hi6220-ufs/ufs_clk_gate_disable"),
            cpuctl_top_app_exist: Self::path_exists("/dev/cpuctl/top-app"),
            cpuctl_foreground_exist: Self::path_exists("/dev/cpuctl/foreground"),
            cpuctl_background_exist: Self::path_exists("/dev/cpuctl/background"),
            cpuset_top_app_exist: Self::path_exists("/dev/cpuset/top-app"),
            cpuset_foreground_exist: Self::path_exists("/dev/cpuset/foreground"),
            cpuset_background_exist: Self::path_exists("/dev/cpuset/background"),
            cpuset_system_background_exist: Self::path_exists("/dev/cpuset/system-background"),
            cpuset_restricted_exist: Self::path_exists("/dev/cpuset/restricted"),
            cpuset_root_exist: Self::path_exists("/dev/cpuset"),
            cpuidle_governor_exist: Self::path_exists("/sys/devices/system/cpu/cpuidle/current_governor"),
            sda_scheduler_exist: Self::path_exists("/sys/block/sda/queue/scheduler"),
        }
    }

    fn path_exists(path: &str) -> bool {
        access(path, AccessFlags::F_OK).is_ok()
    }
}