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

mod fps_window;
mod pid;
mod policy_controller;
mod gear_state;
mod frame_pipeline;
mod pid_jank;
mod policy_mgmt;

// Phase 2 / ticket-06: controller 里新增的 free function (compute_pressure_index /
// mode_target_pressure) 需要从 scheduler/mod.rs 调用, 提升为 pub mod.
pub mod controller;

pub use controller::FasController;

// ============================================================
//  FAS_PANIC — 丢帧紧急信号 (D5)
// ============================================================
//
// 用途: 当 FAS 检测到 frame dropped (frame_delta_ns > fixed_max_frame_ms),
//      set FAS_PANIC=true → hotplug loop 立刻 enable 所有 cpu, 不等 debounce.
//
// 设计: Arc<AtomicBool> 跨线程共享, hotplug 在 200ms tick 头部 read.
//
// 生命周期:
//   - frame_pipeline.rs::update_frame 检测丢帧 → set true
//   - hotplug loop 读完 snapshot 后 → reset false (避免 panic 残留)
//
// 线程安全: AtomicBool + Relaxed ordering (无 happens-before 关系, 只关心最终可见性)

use std::sync::atomic::{AtomicBool, Ordering};

/// 全局 FAS 丢帧 panic 信号 (D5)
pub static FAS_PANIC: AtomicBool = AtomicBool::new(false);

/// FAS 检测到丢帧, 设置 panic 信号.
/// 调用方: frame_pipeline.rs::update_frame 在 frame_delta_ns > max_ns 时调用
#[inline]
pub fn set_fas_panic() {
    FAS_PANIC.store(true, Ordering::Relaxed);
}

/// 读取 panic 信号 (hotplug loop 用)
#[inline]
pub fn is_fas_panic() -> bool {
    FAS_PANIC.load(Ordering::Relaxed)
}

/// hotplug loop 处理完 panic 后清除 (防止一直 enable, 让 debounce 重新接管)
#[inline]
pub fn clear_fas_panic() {
    FAS_PANIC.store(false, Ordering::Relaxed);
}
