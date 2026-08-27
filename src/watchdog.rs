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

//! 可靠性守护 — 任务 #6 (reliability / watchdog)
//!
//! 设计原则 (与项目其它模块一致: 保护层, 不动业务逻辑):
//!
//! * 主线程 (monitor 循环) 每次成功 tick 调用 `heartbeat_tick()` 写入当前
//!   unix epoch 秒数, watchdog 自己的线程每 `WATCHDOG_INTERVAL_SEC` 秒
//!   读一次时间差.
//! * 看门狗只看 "心跳是否超时", "在线核心数是否下降到 1", "CPU 温度是否
//!   ≥ 95°C" 三个外部可观测信号, 不读业务模块的内存状态, 避免与八路感知
//!   的 `OnceLock` 抢锁.
//! * 异常 → 调用 `scripts/restore_defaults.sh` (模块脚本, sh 退出码 0 = OK).
//!   连续 3 次恢复仍异常 → 写 logcat + Android notification + 自我退出
//!   (由 service.sh / Magisk 自动重启).
//!
//! 线程模型:
//! * `heartbeat_tick()` 在任意线程调用 (幂等, 仅写原子值).
//! * `start_watchdog_thread()` 启动一个后台 daemon 线程, 阻塞跑循环,
//!   直到进程退出.

use log::{debug, info, warn, error};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const WATCHDOG_INTERVAL_SEC: u64 = 5;
const HEARTBEAT_TIMEOUT_SEC: u64 = 15;
const MIN_ONLINE_CORES: usize = 2;
const TEMP_CRITICAL_MILLIC: i32 = 95_000;
const MAX_RECOVERY_ATTEMPTS: u32 = 3;
const RESTORE_SCRIPT_NAME: &str = "scripts/restore_defaults.sh";

static HEARTBEAT_AT_SEC: AtomicU64 = AtomicU64::new(0);
static RECOVERY_FAIL_COUNT: AtomicU64 = AtomicU64::new(0);
static WATCHDOG_SPAWNED: OnceLock<()> = OnceLock::new();

/// 主循环成功 tick 后调用. 幂等, 线程安全, 不阻塞.
pub fn heartbeat_tick() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    HEARTBEAT_AT_SEC.store(now, Ordering::Relaxed);
    // 不打 log: 此函数每秒被调用数十次, 会淹没日志
}

/// 查询当前心跳时间距今秒数. u64::MAX = 从未 tick 过.
pub fn heartbeat_age_sec() -> u64 {
    let last = HEARTBEAT_AT_SEC.load(Ordering::Relaxed);
    if last == 0 {
        return u64::MAX;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.saturating_sub(last)
}

/// 启动 watchdog 后台线程. 重复调用幂等 (只 spawn 一次).
pub fn start_watchdog_thread() {
    let _ = WATCHDOG_SPAWNED.get_or_init(|| {
        heartbeat_tick();
        thread::Builder::new()
            .name("yumi_watchdog".to_string())
            .spawn(|| {
                info!(
                    "[watchdog] started (interval={}s, heartbeat_timeout={}s, min_cores={}, temp_critical_millic={})",
                    WATCHDOG_INTERVAL_SEC, HEARTBEAT_TIMEOUT_SEC, MIN_ONLINE_CORES, TEMP_CRITICAL_MILLIC
                );
                loop {
                    thread::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SEC));
                    run_one_cycle();
                }
            })
            .expect("failed to spawn watchdog thread");
        ()
    });
}

fn run_one_cycle() {
    let age = heartbeat_age_sec();
    let online = count_online_cores();
    let temp_millic = read_sense_temp_millic();
    let fail_count = RECOVERY_FAIL_COUNT.load(Ordering::Relaxed);
    debug!(
        "[watchdog] cycle tick: hb_age={}s online_cores={} temp_millic={} fail_count={}",
        age, online, temp_millic, fail_count,
    );

    if age > HEARTBEAT_TIMEOUT_SEC {
        warn!("[watchdog] heartbeat stale: {}s > {}s", age, HEARTBEAT_TIMEOUT_SEC);
        try_recover("heartbeat_stale");
        return;
    }

    if online < MIN_ONLINE_CORES {
        warn!("[watchdog] online cores too few: {} < {}", online, MIN_ONLINE_CORES);
        try_recover("cores_offline");
        return;
    }

    if temp_millic != i32::MIN && temp_millic >= TEMP_CRITICAL_MILLIC {
        warn!("[watchdog] CPU temperature critical: {}°C", temp_millic / 1000);
        try_recover("temp_critical");
        return;
    }

    let prev_fail = RECOVERY_FAIL_COUNT.swap(0, Ordering::Relaxed);
    if prev_fail > 0 {
        debug!("[watchdog] clear recovery fail_count (was {})", prev_fail);
    }
}

fn count_online_cores() -> usize {
    let mut count = 0usize;
    let mut found_any = false;
    for cpu_id in 0..32 {
        let path = format!("/sys/devices/system/cpu/cpu{}/online", cpu_id);
        match std::fs::read_to_string(&path) {
            Ok(s) => {
                found_any = true;
                if s.trim() == "1" {
                    count += 1;
                }
            }
            Err(_) => {
                if cpu_id == 0 && std::path::Path::new("/sys/devices/system/cpu/cpu0").exists() {
                    found_any = true;
                    count += 1;
                }
            }
        }
    }
    if !found_any { usize::MAX } else { count }
}

fn read_sense_temp_millic() -> i32 {
    crate::monitor::sense_snapshot::sense_now().temp_millic
}

/// 触发一次自愈: 调用 restore_defaults.sh + 用户通知.
fn try_recover(reason: &'static str) {
    let attempt = RECOVERY_FAIL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    error!("[watchdog] recovery attempt #{} triggered by '{}'", attempt, reason);

    let script_path = resolve_restore_script_path();
    debug!("[watchdog] invoking {} (reason={}, attempt={})", script_path, reason, attempt);
    // MODDIR 必须显式传给子 shell —— restore_defaults.sh 内部用 $MODDIR 写日志
    // 默认值已经脚本里兜底, 但传 env 后即便路径未来变化也不用改脚本.
    let moddir = crate::common::get_module_root().to_string_lossy().into_owned();
    let result = Command::new("/system/bin/sh")
        .arg(&script_path)
        .arg(reason)
        .env("MODDIR", &moddir)
        .output();

    match result {
        Ok(out) if out.status.success() => {
            info!("[watchdog] restore_defaults.sh OK (attempt #{}, reason='{}')", attempt, reason);
            debug!(
                "[watchdog] restore_defaults.sh stdout (first 200B)={:?}",
                String::from_utf8_lossy(&out.stdout).chars().take(200).collect::<String>()
            );
        }
        Ok(out) => {
            warn!(
                "[watchdog] restore_defaults.sh exit={:?} stderr={:?}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Err(e) => {
            warn!("[watchdog] restore_defaults.sh exec failed: {} (path={})", e, script_path);
        }
    }

    notify_user(reason, attempt);

    if attempt >= MAX_RECOVERY_ATTEMPTS as u64 {
        error!("[watchdog] recovery failed {} times, exiting for external restart", attempt);
        thread::sleep(Duration::from_millis(200));
        std::process::exit(0);
    }
}

fn resolve_restore_script_path() -> String {
    let root = crate::common::get_module_root();
    root.join(RESTORE_SCRIPT_NAME).to_string_lossy().into_owned()
}

/// 用户通知: logcat + Android notification (前台可见).
fn notify_user(reason: &str, attempt: u64) {
    error!("[watchdog] USER NOTICE: '{}' detected, recovery attempt #{}", reason, attempt);

    let tag = "yumi_watchdog";
    let title = "核心领航员调度异常";
    let body = format!(
        "检测到异常: {}。已尝试自愈 {} 次, 持续异常会重启 daemon。",
        reason, attempt
    );
    let _ = Command::new("cmd")
        .args(&["notification", "post", "-t", tag, tag, title, &body])
        .output();

    let log_path = crate::common::get_module_root().join("logs/watchdog.log");
    let line = format!(
        "{}\t{}\tattempt={}\n",
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        reason,
        attempt
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_tick_writes_and_reads_back() {
        let before = heartbeat_age_sec();
        heartbeat_tick();
        let after = heartbeat_age_sec();
        assert!(after <= before + 1, "heartbeat should refresh timestamp");
    }

    #[test]
    fn constants_are_sane() {
        assert!(WATCHDOG_INTERVAL_SEC >= 1);
        assert!(HEARTBEAT_TIMEOUT_SEC > WATCHDOG_INTERVAL_SEC);
        assert!(MIN_ONLINE_CORES >= 1);
        assert!(TEMP_CRITICAL_MILLIC >= 60_000);
    }
}
