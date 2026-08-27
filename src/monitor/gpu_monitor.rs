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

//! GPU 状态采集器 (Ticket-03 / Phase 1)
//!
//! 数据源: `/sys/class/devfreq/<gpu>/cur_freq`, `max_freq`, `load`, `governor`
//!         或者 kgsl 驱动的 gpu 节点
//! 周期: 200ms tick
//!
//! 推送: SenseSnapshot.gpu
//! 不修改 hotplug 模块.

use std::fs;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::{debug, info, warn};

use crate::common::DaemonEvent;
use crate::monitor::sense_snapshot::{gpu_push, GpuState};

const TICK_MS: u64 = 200;

#[inline]
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// 找 GPU devfreq 节点.
///
/// 扫描 /sys/class/devfreq/, 找 name 含 gpu / adreno / mali / kgsl 的目录.
/// 返回第一条命中. 若全无 → None (此时 push 出去的 GpuState 全 0).
fn find_gpu_devfreq_path() -> Option<String> {
    let dir = fs::read_dir("/sys/class/devfreq").ok()?;
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains("gpu")
            || name.contains("adreno")
            || name.contains("mali")
            || name.contains("kgsl")
        {
            return Some(entry.path().to_string_lossy().to_string());
        }
    }
    // 小米 / 高通有时候走 /sys/class/kgsl/kgsl-3d0/
    if Path::new("/sys/class/kgsl/kgsl-3d0").exists() {
        return Some("/sys/class/kgsl/kgsl-3d0".to_string());
    }
    None
}

/// 读一个文件里的整数, 失败返回 None
fn read_u64(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// 读一个文件里的浮点 (load 文件是 " 42\n" 这种格式)
fn read_u32_percent(path: &Path) -> Option<u32> {
    let s = fs::read_to_string(path).ok()?;
    let v: u32 = s.trim().parse().ok()?;
    Some(v.min(100))
}

/// 读 governor (字符串)
fn read_governor(path: &Path) -> Option<&'static str> {
    let s = fs::read_to_string(path).ok()?.trim().to_string();
    if s.is_empty() {
        return None;
    }
    // governor 字符串来自内核, 长度很短, 泄漏成 'static 即可
    Some(Box::leak(s.into_boxed_str()))
}

/// GPU 使用率节点 (高通 kgsl 优先 → devfreq/load 兜底)
const KGSL_BUSY_PATH: &str = "/sys/class/kgsl/kgsl-3d0/gpu_busy_percentage";
/// 高通当前频率兜底 (部分 kgsl 平台 devfreq 下没有 cur_freq)
const KGSL_CLK_PATH: &str = "/sys/class/kgsl/kgsl-3d0/gpuclk";

/// 读 "42 %" / "42%" 这类 busy 文件, 取出前导数字
fn read_kgsl_busy() -> Option<u32> {
    let s = fs::read_to_string(KGSL_BUSY_PATH).ok()?;
    let num: String = s.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    let v: u32 = num.parse().ok()?;
    Some(v.min(100))
}

/// 一次性 tick: 读 GPU 状态并 push
fn tick_once(gpu_root: Option<&str>) {
    let (cur, max, load, gov);
    if let Some(root) = gpu_root {
        let p = Path::new(root);
        cur = match read_u64(&p.join("cur_freq")) {
            Some(v) if v > 0 => Some(v),
            // devfreq 无 cur_freq (高通常见) → kgsl gpuclk 兜底
            _ => read_u64(Path::new(KGSL_CLK_PATH)),
        };
        max = read_u64(&p.join("max_freq"));
        load = match read_kgsl_busy() {
            Some(v) => Some(v),
            None => read_u32_percent(&p.join("load")),
        };
        gov = read_governor(&p.join("governor"));
    } else {
        cur = None;
        max = None;
        load = None;
        gov = None;
    }

    gpu_push(GpuState {
        cur_freq_hz: cur.unwrap_or(0),
        max_freq_hz: max.unwrap_or(0),
        load_pct: load.map(|v| v as f32).unwrap_or(f32::NAN),
        governor: gov.unwrap_or(""),
        updated_at_ns: now_ns(),
    });
}

/// 启动采集线程 (200ms tick)
pub fn start_gpu_loop(_tx: Sender<DaemonEvent>) {
    thread::Builder::new()
        .name("gpu_monitor".to_string())
        .spawn(move || {
            // 第一次启动时找一次, 之后如果设备动态出现 / 消失, 每 10s 重试一次
            let mut gpu_root: Option<String> = None;
            let mut last_root_resolve_ms: u64 = 0;
            loop {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                if gpu_root.is_none() || now_ms - last_root_resolve_ms > 10_000 {
                    gpu_root = find_gpu_devfreq_path();
                    last_root_resolve_ms = now_ms;
                    if let Some(ref r) = gpu_root {
                        info!("[gpu_monitor] using {}", r);
                    } else {
                        debug!("[gpu_monitor] no gpu devfreq found yet");
                    }
                }
                tick_once(gpu_root.as_deref());
                thread::sleep(Duration::from_millis(TICK_MS));
            }
        })
        .ok();
}
