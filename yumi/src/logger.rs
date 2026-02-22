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

use anyhow::{anyhow, Result};
use log::LevelFilter;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Handle;
use once_cell::sync::OnceCell;
use std::sync::Mutex;
use crate::common;
use crate::i18n::t_with_args;
use crate::fluent_args;

// 全局保存 log4rs 的句柄，用于后续动态更新配置
static LOG_HANDLE: OnceCell<Mutex<Handle>> = OnceCell::new();

/// 将字符串转换为 LevelFilter
fn parse_level(level_str: &str) -> LevelFilter {
    match level_str.to_uppercase().as_str() {
        "OFF" => LevelFilter::Off,
        "ERROR" => LevelFilter::Error,
        "WARN" => LevelFilter::Warn,
        "INFO" => LevelFilter::Info,
        "DEBUG" => LevelFilter::Debug,
        "TRACE" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

/// 构建 log4rs 配置的辅助函数
fn build_config(level: LevelFilter) -> Result<Config> {
    let root = common::get_module_root();
    let log_file_path = root.join("logs/daemon.log");
    
    // 1. 定义归档策略 (FixedWindowRoller)
    // 当日志触发滚动时，旧日志会被重命名为 daemon.1.log, daemon.2.log 等
    // 这里 pattern 中的 "{}" 会被替换为索引数字
    let archive_pattern = root.join("logs/daemon.{}.log"); 
    let window_roller = FixedWindowRoller::builder()
        .build(
            archive_pattern.to_str().unwrap(), // 归档文件路径模式
            3, // 保留的历史文件数量 (保留最近的 3 个)
        )?;

    // 2. 定义触发策略 (SizeTrigger)
    // 当文件大小超过 5MB 时触发滚动
    let size_limit = 5 * 1024 * 1024; // 5MB
    let size_trigger = SizeTrigger::new(size_limit);

    // 3. 组合策略
    let compound_policy = CompoundPolicy::new(
        Box::new(size_trigger),
        Box::new(window_roller)
    );

    // 4. 构建 RollingFileAppender
    let log_file_appender = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("[{d(%Y-%m-%d %H:%M:%S)}] [{l}] [{M}] {m}{n}")))
        .build(log_file_path, Box::new(compound_policy))?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(log_file_appender)))
        .build(
            Root::builder()
                .appender("logfile")
                .build(level)
        )?;

    Ok(config)
}

/// 初始化日志系统 (只在 main 启动时调用一次)
pub fn init(level_str: &str) -> Result<()> {
    let level = parse_level(level_str);
    let config = build_config(level)?;

    // 初始化 log4rs 并获取句柄
    let handle = log4rs::init_config(config)?;

    // 将句柄保存到全局变量，供后续更新使用
    LOG_HANDLE.set(Mutex::new(handle))
        .map_err(|_| anyhow!("Logger already initialized"))?;

    Ok(())
}

/// 动态更新日志等级 (在 Config Watcher 中调用)
pub fn update_level(level_str: &str) {
    let level = parse_level(level_str);
    
    if let Some(mutex) = LOG_HANDLE.get() {
        if let Ok(handle) = mutex.lock() {
            // 重新构建配置
            match build_config(level) {
                Ok(new_config) => {
                    // 使用句柄热更新配置
                    handle.set_config(new_config);
                    log::info!("{}", t_with_args("log-level-updated", &fluent_args!("level" => level.to_string())));
                }
                Err(e) => {
                    // 这里只能用 println，因为 log 系统可能处于中间状态，或者为了保险
                    eprintln!("Failed to rebuild logger config: {}", e);
                }
            }
        }
    }
}