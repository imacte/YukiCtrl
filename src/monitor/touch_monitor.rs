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

//! 触摸事件采集器 (Ticket-03 / Phase 1)
//!
//! 目标: 把"用户在触摸屏幕"这个事实暴露给 hotplug 决策层, 用于:
//! - 开核旁路 (漏洞 1): TOUCH_DOWN=true 且屏幕刚亮 → 立即开核, bypass debounce
//! - 触摸后保护窗口 (TOUCH_COOLDOWN_MS = 200ms): 开核后短时间内不关
//!
//! 实现:
//! - 后台独立线程, epoll 阻塞等 /dev/input/event* 的输入事件
//! - 只识别 EV_ABS / ABS_MT_TRACKING_ID (协议 A) 和 EV_KEY / BTN_TOUCH (老协议)
//! - 触摸 down → 调 `crate::scheduler::hotplug::touch_signal::set_touch_down(now_ms)`
//! - 触摸 up 或超时 5s 无新事件 → 调 `clear_touch_down()`
//! - 每 200ms 把当前状态 push 进 SenseSnapshot.touch
//!
//! 设备发现:
//! - 优先 env `TOUCH_DEV`
//! - 否则扫 /dev/input/event*, 用 EVIOCGNAME 找名字含 touch / synaptics / goodix 等
//! - 找不到 → 线程 sleep 5s 重试, 不 panic
//!
//! 不修改 hotplug 模块的任何文件. 通过 `crate::scheduler::hotplug::touch_signal` 调公开 API.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::{debug, info, warn};

use crate::common::DaemonEvent;
use crate::monitor::sense_snapshot::{touch_push, TouchState};
use crate::scheduler::hotplug::touch_signal as tsig;

/// epoll 单次阻塞超时 (ms) = hotplug tick
const EPOLL_TIMEOUT_MS: i32 = 200;

/// 触摸结束超时 (ms): 这么久没新事件就算 up
const TOUCH_TIMEOUT_MS: i64 = 5_000;

/// input_event 结构体 (Linux kernel uapi)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct InputEvent {
    time_sec: u64,
    time_usec: u64,
    /// type (EV_KEY=0x01, EV_ABS=0x03, EV_SYN=0x00)
    typ: u16,
    /// code (BTN_TOUCH=0x14a, ABS_MT_TRACKING_ID=0x39)
    code: u16,
    value: i32,
}

/// 当前 unix epoch 毫秒
#[inline]
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 当前 unix epoch 纳秒 (sense_snapshot::now_ns 是私有, 这里复用自己实现的版本)
#[inline]
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// 判断一个 input_event 是否表示 "手指按下"
#[inline]
fn is_touch_down(ev: &InputEvent) -> bool {
    // 协议 A: ABS_MT_TRACKING_ID, value >= 0 表示按下
    if ev.typ == 0x03 && ev.code == 0x39 {
        return ev.value >= 0;
    }
    // 老协议: BTN_TOUCH value == 1
    if ev.typ == 0x01 && ev.code == 0x14a {
        return ev.value == 1;
    }
    false
}

/// 判断一个 input_event 是否表示 "手指抬起"
#[inline]
fn is_touch_up(ev: &InputEvent) -> bool {
    if ev.typ == 0x03 && ev.code == 0x39 {
        return ev.value == -1;
    }
    if ev.typ == 0x01 && ev.code == 0x14a {
        return ev.value == 0;
    }
    false
}

/// 找触摸设备:
///
/// 1) env TOUCH_DEV 非空 + 存在 → 直接返回
/// 2) 扫 /dev/input/event*, 读 EVIOCGNAME 长 256 字节, 找名字含触屏关键词的
fn find_touch_device() -> Option<String> {
    if let Ok(dev) = std::env::var("TOUCH_DEV") {
        if !dev.is_empty() && Path::new(&dev).exists() {
            info!("[touch_monitor] TOUCH_DEV env override: {}", dev);
            return Some(dev);
        }
    }
    let dir = match std::fs::read_dir("/dev/input") {
        Ok(d) => d,
        Err(e) => {
            warn!("[touch_monitor] cannot read /dev/input: {}", e);
            return None;
        }
    };
    for entry in dir.flatten() {
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if !s.starts_with("event") {
            continue;
        }
        let path = format!("/dev/input/{}", s);
        if probe_touch_device(&path) {
            return Some(path);
        }
    }
    None
}

/// 用 EVIOCGNAME (Linux ioctl 0x80_00_45_06) 探 device 名字, 含触屏关键词就 true
fn probe_touch_device(path: &str) -> bool {
    let f = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            warn!("[touch_monitor] open({}) failed: {}", path, e);
            return false;
        },
    };
    let fd = f.as_raw_fd();
    let mut buf = [0u8; 256];
    let res = unsafe {
        libc::ioctl(fd, 0x81_00_45_06u32 as _, buf.as_mut_ptr())
    };
    if res < 0 {
        let err = std::io::Error::last_os_error();
        warn!("[touch_monitor] ioctl(EVIOCGNAME) on {} failed: {}", path, err);
        // 尝试用 EVIOCGBIT(0) 1 byte 判断 EV 能力作为兜底
        return false;
    }
    let name_end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let name = String::from_utf8_lossy(&buf[..name_end]).to_lowercase();
    // 扩展关键词: 小米 14 Pro synaptics_tcm_touch 应该匹配 'synaptics' 或 'touch'
    let matches = name.contains("touch")
        || name.contains("synaptics")
        || name.contains("goodix")
        || name.contains("atmel")
        || name.contains("fts")
        || name.contains("qdti")
        || name.contains("nt36xxx")
        || name.contains("himax")
        || name.contains("mi_touch")
        || name.contains("xiaomi");
    info!("[touch_monitor] probed {}: name={:?} match={}", path, String::from_utf8_lossy(&buf[..name_end]), matches);
    matches
}

/// 全局停止标志 — daemon 主流程退出时调用 `request_stop_touch_monitor()`.
/// 后台线程在两个点检查: 设备发现重试间隔, 主 epoll 循环顶部.
static TOUCH_MONITOR_STOP: AtomicBool = AtomicBool::new(false);

/// 请求停止触摸采集线程 (主流程调用, idempotent)
pub fn request_stop_touch_monitor() {
    debug!("[touch_monitor] request_stop");
    TOUCH_MONITOR_STOP.store(true, Ordering::SeqCst);
}

/// 主线程入口: 阻塞运行 epoll 循环
pub fn start_touch_loop(_tx: Sender<DaemonEvent>) {
    thread::Builder::new()
        .name("touch_monitor".to_string())
        .spawn(move || {
            if let Err(e) = touch_loop_inner() {
                warn!("[touch_monitor] loop exited: {}", e);
            }
        })
        .ok();
}

fn touch_loop_inner() -> Result<(), Box<dyn std::error::Error>> {
    // 设备发现循环: 找不到就一直重试 (但每秒检查一次 STOP)
    let device_path = loop {
        if TOUCH_MONITOR_STOP.load(Ordering::SeqCst) {
            info!("[touch_monitor] stop requested before device found, exiting");
            return Ok(());
        }
        if let Some(p) = find_touch_device() {
            break p;
        }
        warn!("[touch_monitor] no touch device found, retry in 5s");
        // 分段 sleep 以便快速响应 STOP
        for _ in 0..50 {
            if TOUCH_MONITOR_STOP.load(Ordering::SeqCst) { return Ok(()); }
            thread::sleep(Duration::from_millis(100));
        }
    };
    info!("[touch_monitor] using device: {}", device_path);

    // 把 device path 泄漏成 &'static str 供 TouchState.device_path 用
    let device_path_static: &'static str = Box::leak(device_path.clone().into_boxed_str());

    let mut dev_file = OpenOptions::new().read(true).write(false).open(&device_path)?;
    let fd = dev_file.as_raw_fd();

    // epoll
    let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
    if epoll_fd < 0 {
        return Err("epoll_create1 failed".into());
    }
    let mut ev = libc::epoll_event {
        events: libc::EPOLLIN as _,
        u64: 1,
    };
    let rc = unsafe { libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut ev) };
    if rc < 0 {
        unsafe { libc::close(epoll_fd) };
        return Err("epoll_ctl ADD failed".into());
    }

    let mut last_event_ms: i64 = now_ms();
    let mut events_in_tick: u32 = 0;
    let mut last_push_ms: i64 = 0;
    let mut last_push_was_down: Option<bool> = None;
    let ev_size = std::mem::size_of::<InputEvent>();
    debug!("[touch_monitor] entering epoll loop on {}", device_path);

    loop {
        // 每次 epoll_wait 前检查 STOP
        if TOUCH_MONITOR_STOP.load(Ordering::SeqCst) {
            info!("[touch_monitor] stop requested, exiting epoll loop");
            break;
        }

        let mut events = [libc::epoll_event { events: 0, u64: 0 }; 1];
        let n = unsafe {
            libc::epoll_wait(epoll_fd, events.as_mut_ptr(), 1, EPOLL_TIMEOUT_MS)
        };

        if n > 0 {
            // 一次最多读 32 个 input_event
            let mut buf = vec![0u8; 32 * ev_size];
            let got = dev_file.read(&mut buf).unwrap_or(0);
            let count = got / ev_size;
            for i in 0..count {
                let ev: InputEvent = unsafe {
                    std::ptr::read_unaligned(buf.as_ptr().add(i * ev_size) as *const _)
                };
                let now = now_ms();
                last_event_ms = now;
                events_in_tick = events_in_tick.saturating_add(1);
                if is_touch_down(&ev) {
                    debug!("[touch_monitor] DOWN @ {} ms", now);
                    tsig::set_touch_down(now);
                } else if is_touch_up(&ev) {
                    debug!("[touch_monitor] UP @ {} ms", now);
                    tsig::clear_touch_down();
                }
            }
        }

        // 超时自动 up
        let now = now_ms();
        if tsig::is_touch_down() && now - last_event_ms > TOUCH_TIMEOUT_MS {
            debug!("[touch_monitor] timeout ({} ms no event)", now - last_event_ms);
            tsig::clear_touch_down();
        }

        // 每 200ms push 一次
        if now - last_push_ms >= EPOLL_TIMEOUT_MS as i64 {
            let down = tsig::is_touch_down();
            let down_since = tsig::touch_down_since_ms();
            let last_age = if down {
                0
            } else {
                now.saturating_sub(last_event_ms).max(0) as u64
            };
            // 只在 down 状态切换或事件计数>0 时打 debug, 避免每 200ms 一条日志
            if last_push_was_down != Some(down) || events_in_tick > 0 {
                debug!(
                    "[touch_monitor] tick push down={} since_ms={} last_age_ms={} events={}",
                    down, down_since, last_age, events_in_tick,
                );
                last_push_was_down = Some(down);
            }
            touch_push(TouchState {
                down,
                down_since_ms: down_since,
                last_event_age_ms: last_age,
                events_in_tick,
                device_path: device_path_static,
                updated_at_ns: now_ns(),
            });
            events_in_tick = 0;
            last_push_ms = now;
        }
    }

    unsafe { libc::close(epoll_fd) };
    Ok(())
}

