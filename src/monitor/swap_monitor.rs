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

//! Swap / 内存压力采集器 (Ticket-03 / Phase 1)
//!
//! 数据源:
//! - `/proc/pressure/memory`  → mem_some_us, mem_full_us (PSI avg10/total)
//! - `/proc/meminfo`         → SwapTotal, SwapFree (KiB)
//! - `/sys/block/zram0/`     → mem_used_total, disksize (bytes)
//!
//! 推送: SenseSnapshot.swap
//! 周期: 200ms tick

use std::fs;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::debug;

use crate::common::DaemonEvent;
use crate::monitor::sense_snapshot::{swap_push, SwapState};

const TICK_MS: u64 = 200;
const PROC_PRESSURE_MEM: &str = "/proc/pressure/memory";
const PROC_MEMINFO: &str = "/proc/meminfo";
const ZRAM_DIR: &str = "/sys/block/zram0";

#[inline]
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// 解析 PSI 单行 (复用 io_monitor 的格式: "some avg10=... total=...")
fn parse_psi_line(line: &str) -> Option<(bool, f32, u64)> {
    let is_full;
    let mut avg10_pct: f32 = 0.0;
    let mut total_us: u64 = 0;
    if line.starts_with("some ") {
        is_full = false;
    } else if line.starts_with("full ") {
        is_full = true;
    } else {
        return None;
    }
    for tok in line.split_whitespace().skip(1) {
        if let Some(v) = tok.strip_prefix("avg10=") {
            avg10_pct = v.parse().unwrap_or(0.0);
        } else if let Some(v) = tok.strip_prefix("total=") {
            total_us = v.parse().unwrap_or(0);
        }
    }
    Some((is_full, avg10_pct, total_us))
}

/// 读 /proc/pressure/memory 一次
///
/// 返回 (some_total_us, full_total_us, full_avg10_pct).
/// avg10 是 PSI 原生百分比 (0..=100); total 是开机以来累计 us.
/// Bugfix: 此前只存 total, snapshot_writer 又拿 total/10000 当百分比 → 恒 100%.
fn read_memory_pressure() -> (u64, u64, f32) {
    let s = match fs::read_to_string(PROC_PRESSURE_MEM) {
        Ok(s) => s,
        Err(_) => return (0, 0, 0.0),
    };
    let mut some_us = 0u64;
    let mut full_us = 0u64;
    let mut full_avg10 = 0f32;
    for line in s.lines() {
        if let Some((is_full, avg10, total)) = parse_psi_line(line) {
            if is_full {
                full_us = total;
                full_avg10 = avg10;
            } else {
                some_us = total;
            }
        }
    }
    (some_us, full_us, full_avg10)
}

/// 读 /proc/meminfo, 提取 SwapTotal/SwapFree (KiB)
fn read_swap_kb() -> (u64, u64) {
    let s = match fs::read_to_string(PROC_MEMINFO) {
        Ok(s) => s,
        Err(_) => return (0, 0),
    };
    let mut total = 0;
    let mut free = 0;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("SwapTotal:") {
            total = rest.trim().split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("SwapFree:") {
            free = rest.trim().split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
        }
    }
    (total, free)
}

/// 读 zram 状态, 失败 → (0, 0)
///
/// MIUI 内核 `/sys/block/zram0/mem_used_total` 可能恒报 0,
/// 此时兜底解析 `/sys/block/zram0/mm_stat` 第 2 列 (orig_data_size≈used bytes).
/// mm_stat 格式: disksize orig_data_size compr_data_size mem_used_total ...
fn read_zram() -> (u64, u64) {
    let p = Path::new(ZRAM_DIR);
    if !p.exists() {
        return (0, 0);
    }
    let parse_u64 = |s: &str| s.trim().parse::<u64>().unwrap_or(0);
    let used = fs::read_to_string(p.join("mem_used_total"))
        .map(|s| parse_u64(&s))
        .unwrap_or(0);
    let used = if used == 0 {
        fs::read_to_string(p.join("mm_stat"))
            .ok()
            .and_then(|s| s.split_whitespace().nth(1).map(|v| parse_u64(v)))
            .unwrap_or(0)
    } else {
        used
    };
    let total = fs::read_to_string(p.join("disksize"))
        .map(|s| parse_u64(&s))
        .unwrap_or(0);
    (used, total)
}

fn tick_once() {
    let (mem_some, mem_full, mem_full_avg10) = read_memory_pressure();
    let (swap_total, swap_free) = read_swap_kb();
    let (zram_used, zram_total) = read_zram();
    if zram_used > 0 || zram_total > 0 {
        debug!(
            "[swap_monitor] zram used={} ({:.0}MB) total={} mem_full_avg10={}",
            zram_used,
            zram_used as f32 / (1024.0 * 1024.0),
            zram_total,
            mem_full_avg10
        );
    }
    swap_push(SwapState {
        mem_some_us: mem_some,
        mem_full_us: mem_full,
        mem_full_avg10_pct: mem_full_avg10,
        swap_total_kb: swap_total,
        swap_free_kb: swap_free,
        zram_used_bytes: zram_used,
        zram_total_bytes: zram_total,
        updated_at_ns: now_ns(),
    });
}

/// 启动采集线程
pub fn start_swap_loop(_tx: Sender<DaemonEvent>) {
    thread::Builder::new()
        .name("swap_monitor".to_string())
        .spawn(move || loop {
            tick_once();
            thread::sleep(Duration::from_millis(TICK_MS));
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_memory_psi_line_some() {
        let (is_full, avg, total) = parse_psi_line("some avg10=5.00 avg60=0.00 total=1234").unwrap();
        assert!(!is_full);
        assert_eq!(total, 1234);
        assert!((avg - 5.0).abs() < 0.01);
    }

    #[test]
    fn parse_memory_psi_line_full() {
        let (is_full, _avg, total) = parse_psi_line("full avg10=0.00 total=9999").unwrap();
        assert!(is_full);
        assert_eq!(total, 9999);
    }
}
