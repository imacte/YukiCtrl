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

//! IO 压力采集器 (Ticket-03 / Phase 1)
//!
//! 数据源: `/proc/pressure/io`
//! 格式 (内核 PSI):
//!   some avg10=0.00 avg60=0.00 avg300=0.00 total=12345
//!   full avg10=0.00 avg60=0.00 avg300=0.00 total=6789
//!
//! 我们关心 `avg10` (最近 10 秒平均值, 单位 %) + `total` (累积时间, 微秒).
//! avg10 是百分比 (0..100); total 是绝对时间 us, 用作"过去到底卡了多久".
//!
//! 推送: SenseSnapshot.io
//! 周期: 200ms tick

use std::fs;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::{debug, warn};

use crate::common::DaemonEvent;
use crate::monitor::sense_snapshot::{io_push, IoState};

const TICK_MS: u64 = 200;
const PROC_PRESSURE_IO: &str = "/proc/pressure/io";

#[inline]
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[derive(Default, Debug, Clone, Copy)]
struct LineParsed {
    is_full: bool,
    avg10_pct: f32,
    /// avg10 = some10 / 100 * 1_000_000us (10s 窗口) 反推
    /// 我们这里也直接读 total
    total_us: u64,
}

/// 解析一行, 例如 "some avg10=12.34 avg60=... total=987654"
fn parse_line(line: &str) -> Option<LineParsed> {
    let mut p = LineParsed::default();
    if line.starts_with("some ") {
        p.is_full = false;
    } else if line.starts_with("full ") {
        p.is_full = true;
    } else {
        return None;
    }
    for token in line.split_whitespace().skip(1) {
        if let Some(v) = token.strip_prefix("avg10=") {
            p.avg10_pct = v.parse().unwrap_or(0.0);
        } else if let Some(v) = token.strip_prefix("total=") {
            p.total_us = v.parse().unwrap_or(0);
        }
    }
    Some(p)
}

/// 读 /proc/pressure/io 一次, 解析出 some/full
fn read_pressure() -> (LineParsed, LineParsed) {
    let content = match fs::read_to_string(PROC_PRESSURE_IO) {
        Ok(s) => s,
        Err(e) => {
            warn!("[io_monitor] read {} failed: {}", PROC_PRESSURE_IO, e);
            return (LineParsed::default(), LineParsed::default());
        }
    };
    let mut some = LineParsed::default();
    let mut full = LineParsed::default();
    for line in content.lines() {
        if let Some(p) = parse_line(line) {
            if p.is_full {
                full = p;
            } else {
                some = p;
            }
        }
    }
    debug!(
        "[io_monitor] PSI some_pct={:.2} full_pct={:.2} some_us={} full_us={}",
        some.avg10_pct, full.avg10_pct, some.total_us, full.total_us,
    );
    (some, full)
}

/// 一次性 tick: 解析 + push
fn tick_once() {
    let (some, full) = read_pressure();
    io_push(IoState {
        some_us: some.total_us,
        full_us: full.total_us,
        some_pct: some.avg10_pct,
        full_pct: full.avg10_pct,
        updated_at_ns: now_ns(),
    });
}

/// 启动采集线程
pub fn start_io_loop(_tx: Sender<DaemonEvent>) {
    debug!("[io_monitor] starting tick loop ({}ms)", TICK_MS);
    thread::Builder::new()
        .name("io_monitor".to_string())
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
    fn parse_line_some_avg10_total() {
        let s = "some avg10=12.34 avg60=0.00 avg300=0.00 total=987654";
        let p = parse_line(s).unwrap();
        assert!(!p.is_full);
        assert!((p.avg10_pct - 12.34).abs() < 0.001);
        assert_eq!(p.total_us, 987654);
    }

    #[test]
    fn parse_line_full() {
        let s = "full avg10=0.00 avg60=0.00 avg300=0.00 total=42";
        let p = parse_line(s).unwrap();
        assert!(p.is_full);
        assert_eq!(p.total_us, 42);
    }

    #[test]
    fn parse_line_unknown_returns_none() {
        assert!(parse_line("garbage data").is_none());
        assert!(parse_line("").is_none());
    }
}
