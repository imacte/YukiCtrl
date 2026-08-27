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

//! 全局触摸信号 (漏洞 1)
//!
//! 设计: `TOUCH_DOWN` AtomicBool + `TOUCH_DOWN_SINCE` AtomicI64 (touch 起始 unix ms)
//!
//! 用途:
//! - touch_monitor 检测到手指 down → set true, 记录起始时间
//! - touch_monitor 检测到 up 或超时 → set false
//! - hotplug loop tick 头部 read: 如果 TOUCH_DOWN=true 且屏幕刚亮起
//!   → 立即 enable 所有核 (bypass debounce, 优先级: thermal > touch > normal hysteresis)
//! - 开核后 200ms 内不关核 (touch_cooldown)
//!
//! 位置: 放在 hotplug 子模块, 不污染 monitor/. 真正的 epoll 采集器写在 monitor/touch_monitor.rs
//!       但信号共享放在这里是因为 hotplug 是主要消费者.
//!
//! 线程安全: AtomicBool + Relaxed (无 happens-before, 只关心最终可见性, 与 FAS_PANIC 同款)

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

/// 全局触摸按下信号 (漏洞 1)
pub static TOUCH_DOWN: AtomicBool = AtomicBool::new(false);

/// 触摸起始时间 (Unix epoch ms); 0 表示当前未触摸
pub static TOUCH_DOWN_SINCE_MS: AtomicI64 = AtomicI64::new(0);

/// 开核后保护窗口 (200ms 不关核, 与一个 hotplug tick 对齐)
pub const TOUCH_COOLDOWN_MS: i64 = 200;

#[inline]
pub fn set_touch_down(now_ms: i64) {
    TOUCH_DOWN.store(true, Ordering::Relaxed);
    TOUCH_DOWN_SINCE_MS.store(now_ms, Ordering::Relaxed);
}

#[inline]
pub fn clear_touch_down() {
    TOUCH_DOWN.store(false, Ordering::Relaxed);
    TOUCH_DOWN_SINCE_MS.store(0, Ordering::Relaxed);
}

#[inline]
pub fn is_touch_down() -> bool {
    TOUCH_DOWN.load(Ordering::Relaxed)
}

#[inline]
pub fn touch_down_since_ms() -> i64 {
    TOUCH_DOWN_SINCE_MS.load(Ordering::Relaxed)
}