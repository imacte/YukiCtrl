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
use std::path::PathBuf;
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerMessage {
    pub package_name: String,
    pub mode: String,
    pub temperature: f64,
    // pub enable_scheduler: bool,
}

/// 获取模块根目录的绝对路径
pub fn get_module_root() -> PathBuf {
    // 获取当前执行文件的绝对路径
    let exe_path = env::current_exe().unwrap_or_else(|_| PathBuf::from("/"));
    
    // 回溯两级目录:
    // core/bin/yuki-daemon -> core/bin -> core -> YukiDaemon
    exe_path
        .parent().unwrap_or(&exe_path) // .../core/bin
        .parent().unwrap_or(&exe_path) // .../core
        .parent().unwrap_or(&exe_path) // .../YukiDaemon (Root)
        .to_path_buf()
}