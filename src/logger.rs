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
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Handle;
use once_cell::sync::OnceCell;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use crate::common;
use crate::i18n::t_with_args;
use crate::fluent_args;

static LOG_HANDLE: OnceCell<Mutex<Handle>> = OnceCell::new();

/// 默认单文件上限 10 MB; 可用 LOG_ROTATE_BYTES 环境变量覆盖 (便于现场验证轮转).
const DEFAULT_ROTATE_BYTES: u64 = 10 * 1024 * 1024;
/// 默认保留历史日志数量; 可用 LOG_ROTATE_KEEP 覆盖.
const DEFAULT_ROTATE_KEEP: u32 = 3;

fn rotate_bytes() -> u64 {
    std::env::var("LOG_ROTATE_BYTES")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_ROTATE_BYTES)
}

fn rotate_keep() -> u32 {
    std::env::var("LOG_ROTATE_KEEP")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_ROTATE_KEEP)
}

fn parse_level(level_str: &str) -> LevelFilter {
    match level_str.to_uppercase().as_str() {
        "OFF" => LevelFilter::Off,
        "ERROR" => LevelFilter::Error,
        "WARN" => LevelFilter::Warn,
        "INFO" => LevelFilter::Info,
        "DEBUG" => LevelFilter::Debug,
        "TRACE" => LevelFilter::Trace,
        _ => LevelFilter::Debug, // 默认 DEBUG (全量调试)
    }
}

/// 选择实际写入目录: 优先模块内 logs/, 不可写时降级到 /data/local/tmp.
/// 这样即使 Magisk 模块根目录 read-only (部分定制 ROM), 也能保留日志.
fn resolve_log_dir() -> PathBuf {
    let primary = common::get_module_root().join("logs");
    match std::fs::create_dir_all(&primary) {
        Ok(()) => {
            // 用临时文件验证可写
            let probe = primary.join(".write_probe");
            if std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&probe)
                .and_then(|_| std::fs::remove_file(&probe))
                .is_ok()
            {
                return primary;
            }
        }
        Err(_) => {}
    }
    // fallback
    let fallback = PathBuf::from("/data/local/tmp/core-pilot/logs");
    let _ = std::fs::create_dir_all(&fallback);
    eprintln!(
        "[logger] module logs dir not writable, falling back to {}",
        fallback.display()
    );
    fallback
}

/// 写一行 fallback 警告, 把降级路径记到 logcat (便于用户察觉).
fn warn_fallback_chosen(dir: &Path) {
    if dir.starts_with("/data/local/tmp") {
        log::warn!(
            "logger falling back to {}, WebUI log viewer may show empty",
            dir.display()
        );
    }
}

fn build_config(level: LevelFilter) -> Result<Config> {
    let log_dir = resolve_log_dir();
    warn_fallback_chosen(&log_dir);
    let log_path = log_dir.join("daemon.log");
    let archive_pattern = log_dir.join("daemon.{}.log");

    let roller = FixedWindowRoller::builder()
        .build(archive_pattern.to_str().unwrap(), rotate_keep())?;
    let trigger = SizeTrigger::new(rotate_bytes());
    let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    let file_appender = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "[{d(%Y-%m-%d %H:%M:%S)}] [{l}] [{M}] {m}{n}",
        )))
        .build(log_path, Box::new(policy))?;

    // 是否同时输出到 stderr (调试用, 通过 LOG_TO_STDERR=1 开启)
    let stderr_enabled = std::env::var("LOG_TO_STDERR")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut builder = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(file_appender)));

    if stderr_enabled {
        let stderr_appender = ConsoleAppender::builder()
            .target(Target::Stderr)
            .encoder(Box::new(PatternEncoder::new(
                "[{d(%H:%M:%S)}] [{l}] [{M}] {m}{n}",
            )))
            .build();
        builder = builder.appender(Appender::builder().build("stderr", Box::new(stderr_appender)));
        let config = builder
            .build(
                Root::builder()
                    .appender("logfile")
                    .appender("stderr")
                    .build(level),
            )?;
        return Ok(config);
    }

    let config = builder
        .build(Root::builder().appender("logfile").build(level))?;
    Ok(config)
}

/// 初始化日志系统，启动时调用一次
pub fn init(level_str: &str) -> Result<()> {
    let level = parse_level(level_str);
    let config = build_config(level)?;
    let handle = log4rs::init_config(config)?;
    LOG_HANDLE.set(Mutex::new(handle))
        .map_err(|_| anyhow!("Logger already initialized"))?;
    Ok(())
}

/// 动态更新日志等级
pub fn update_level(level_str: &str) {
    let level = parse_level(level_str);
    if let Some(mutex) = LOG_HANDLE.get() {
        if let Ok(handle) = mutex.lock() {
            match build_config(level) {
                Ok(cfg) => {
                    handle.set_config(cfg);
                    log::debug!(
                        "{}",
                        t_with_args(
                            "log-level-updated",
                            &fluent_args!("level" => level.to_string())
                        )
                    );
                }
                Err(e) => eprintln!("Failed to rebuild logger config: {}", e),
            }
        }
    }
}