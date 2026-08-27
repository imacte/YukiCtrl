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

//! 八路感知聚合快照 (Ticket-03 / Phase 1)
//!
//! 设计原则 (沿用 cpu_monitor::CpuIdleSnapshot 的 OnceLock 模式):
//! - 每个采集器 (touch / gpu / io / swap / temp / fps / screen / cpu_idle)
//!   在自己的 tick 里调用 `X_state_push(...)` 把本轮采集结果写进全局 Arc<Mutex<...>>.
//! - 决策层 (scheduler/hotplug) 在自己的 tick 里调用 `sense_now()` 克隆整张表,
//!   一次性看到八路状态. 八路之间不强耦合, 采集器独立跑独立崩.
//!
//! 不变性:
//! - SenseSnapshot 是只读快照, 不持有任何 IO 资源.
//! - 所有 IO (open file / epoll fd / devfreq fd) 由各采集器自己持有, 本模块只做内存聚合.
//! - 任何字段缺失 (采集器崩 / 文件不存在) 用 Default 值表示: false / 0 / f32::NAN.
//!
//! 线程安全:
//! - 全局 SENSE_SNAPSHOT 是 `Arc<Mutex<SenseSnapshot>>`, 内部锁互斥, 读写都很短
//!   (只 clone 一份 ~200B struct), 200ms tick 下不会成为瓶颈.
//!
//! 时间戳: updated_at_ns 是 unix epoch ns. 0 表示该路从未被 push 过.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use log::debug;

use crate::monitor::cpu_monitor::{idle_snapshot_now, CpuIdleSnapshot};

/// 当前 unix epoch 纳秒
#[inline]
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

// =================================================================
//  1. 单路数据结构 (每个采集器对应一个)
// =================================================================

/// 触摸状态 (TouchMonitor 写入)
#[derive(Debug, Clone, Copy, Default)]
pub struct TouchState {
    /// 当前是否按下 (true = 手在屏幕上)
    pub down: bool,
    /// 按下起始时间 (unix ms); 0 = 当前未触摸
    pub down_since_ms: i64,
    /// 最近一次触摸事件到现在的时间 (ms); 用来判断 "触摸结束多久了"
    pub last_event_age_ms: u64,
    /// 累计本 tick 内 (200ms) 收到的事件数
    pub events_in_tick: u32,
    /// 最近触摸路径 (例如 /dev/input/event7); 空 = 探测失败
    pub device_path: &'static str,
    pub updated_at_ns: u64,
}

/// GPU 状态 (GpuMonitor 写入)
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuState {
    /// 当前频率 (Hz); 0 = 未知
    pub cur_freq_hz: u64,
    /// 最大频率 (Hz)
    pub max_freq_hz: u64,
    /// devfreq load (0..=100, busy %); f32::NAN = 不可读
    pub load_pct: f32,
    /// devfreq governor; 空 = 不可读
    pub governor: &'static str,
    pub updated_at_ns: u64,
}

/// IO 压力 (IoMonitor 写入, 读 /proc/pressure/io)
#[derive(Debug, Clone, Copy, Default)]
pub struct IoState {
    /// some=10 (10ms 窗口内任务因 IO 阻塞的总时间, us)
    pub some_us: u64,
    /// full=10 (10ms 窗口内所有任务都因 IO 阻塞的时间, us)
    pub full_us: u64,
    /// 换算成百分比
    pub some_pct: f32,
    pub full_pct: f32,
    pub updated_at_ns: u64,
}

/// Swap / 内存压力 (SwapMonitor 写入)
#[derive(Debug, Clone, Copy, Default)]
pub struct SwapState {
    /// 内存压力 some=10 (us), 同 IO 语义
    pub mem_some_us: u64,
    /// 内存压力 full=10 (us)
    pub mem_full_us: u64,
    /// 内存压力 full avg10 — PSI 原生百分比 (0..=100), 直接展示用.
    /// 注意: mem_full_us 是 total 累计 us, 不能当窗口百分比用!
    pub mem_full_avg10_pct: f32,
    /// SwapTotal (KiB)
    pub swap_total_kb: u64,
    /// SwapFree (KiB)
    pub swap_free_kb: u64,
    /// zram 已用 (bytes); 0 = 没有 zram
    pub zram_used_bytes: u64,
    /// zram 总容量 (bytes)
    pub zram_total_bytes: u64,
    pub updated_at_ns: u64,
}


// =================================================================
//  2. SenseSnapshot — 八路聚合
// =================================================================

#[derive(Debug, Clone, Default)]
pub struct SenseSnapshot {
    /// 2) 触摸
    pub touch: TouchState,
    /// 3) GPU
    pub gpu: GpuState,
    /// 4) IO 压力
    pub io: IoState,
    /// 5) Swap / 内存压力
    pub swap: SwapState,

    // ---- 下面 3 路在 ticket-02 之前已经存在, 这里只做轻量 re-export 字段 ----
    /// 6) 温度 (millidegree Celsius, 45000 = 45.0°C); i32::MIN = 不可读
    pub temp_millic: i32,
    /// 7) 屏幕 FPS (实测帧率, 0 = 不可读 / 屏幕关闭 / 画面静止)
    pub fps: u32,
    /// 屏幕刷新率 Hz (dumpsys display renderFrameRate; 0 = 不可读).
    /// 注意与 fps 区分: fps = 应用实际出帧速率, display_hz = 面板当前模式 Hz.
    pub display_hz: f32,
    /// 8) 屏幕是否亮起 (true = ON)
    pub screen_on: bool,

    /// 1) per-CPU idle/util 聚合 (Phase 2 / ticket-06).
    ///
    /// 不在采集器里 push — cpu_monitor.rs 不动; 在 `sense_now()` 内部自动
    /// 调 `cpu_monitor::idle_snapshot_now()` 拉取, 决策层拿到的永远是最新一份.
    /// 若 cpu_monitor 还没初始化完 (Vec 为空), 这里就是 Default = 空 Vec.
    pub cpu: CpuIdleSnapshot,

    /// 整张 snapshot 的最近更新时间
    pub updated_at_ns: u64,
}

impl SenseSnapshot {
    /// 触摸是否在最近 N 毫秒内活跃
    #[inline]
    pub fn touch_active_within(&self, window_ms: u64) -> bool {
        self.touch.last_event_age_ms <= window_ms
    }

    /// IO 是否压力高 (默认阈值 40%)
    #[inline]
    pub fn io_pressure_high(&self) -> bool {
        self.io.some_pct >= 40.0
    }

    /// 内存压力 (memory.full) 是否高 (默认阈值 40%)
    #[inline]
    pub fn mem_pressure_high(&self) -> bool {
        // /proc/pressure/memory full=10 时间窗是 10ms = 10000us
        let pct = (self.swap.mem_full_us as f32) / 10_000.0 * 100.0;
        pct >= 40.0
    }

    /// 任意一路是否新鲜 (updated_at_ns > 0 且与现在差 < 2s)
    pub fn any_stale(&self, max_age_ns: u64) -> bool {
        let now = now_ns();
        let fresh = |ts: u64| ts > 0 && now.saturating_sub(ts) <= max_age_ns;
        fresh(self.touch.updated_at_ns)
            || fresh(self.gpu.updated_at_ns)
            || fresh(self.io.updated_at_ns)
            || fresh(self.swap.updated_at_ns)
    }

    /// 温度转 Celsius (方便日志)
    #[inline]
    pub fn temp_c(&self) -> f32 {
        if self.temp_millic == i32::MIN {
            f32::NAN
        } else {
            self.temp_millic as f32 / 1000.0
        }
    }

    /// CPU 整系统平均利用率 (0.0..=100.0).
    ///
    /// Phase 2 综合压力指数的主要输入 (权重 0.40).
    /// 空 Vec (cpu_monitor 未启动) → 0.0, 让压力指数降级到只用 GPU/IO/Mem/frame.
    #[inline]
    pub fn cpu_util_avg(&self) -> f32 {
        if self.cpu.cpus.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.cpu.cpus.iter().map(|e| e.util_pct).sum();
        sum / self.cpu.cpus.len() as f32
    }
}

// =================================================================
//  3. 全局 handle + 读写 API (OnceLock 模式, 与 CpuIdleSnapshot 完全一致)
// =================================================================

static SENSE_SNAPSHOT: OnceLock<Arc<Mutex<SenseSnapshot>>> = OnceLock::new();

/// 获取全局 SenseSnapshot handle (多次调用返回同一个 Arc)
pub fn sense_snapshot_handle() -> Arc<Mutex<SenseSnapshot>> {
    SENSE_SNAPSHOT
        .get_or_init(|| Arc::new(Mutex::new(SenseSnapshot::default())))
        .clone()
}

/// 读取当前 SenseSnapshot (克隆一份给决策层)
///
/// 注意: 返回前会同步拉取最新 CPU idle 快照 (ticket-06).
/// 锁仅持有 O(1) clone 时间, 无 IO 阻塞.
pub fn sense_now() -> SenseSnapshot {
    let h = sense_snapshot_handle();
    let mut snap = h.lock().map(|g| g.clone()).unwrap_or_default();
    // ticket-06: 在决策层调用时拉取最新 CPU 状态, 保证 8 路永远新鲜.
    // cpu_monitor 不动; 这里只在返回前一次性读.
    snap.cpu = idle_snapshot_now();
    // 只在 debug 开启时才构造字符串 (避免热路径开销)
    if log::log_enabled!(log::Level::Debug) {
        // mem_some_us 是 10s 窗口内的累计 us, 转成 % 便于一眼读
        let mem_pct = (snap.swap.mem_some_us as f32 / 10_000_000.0 * 100.0).clamp(0.0, 100.0);
        debug!(
            "[sense_snapshot] touch={} gpu={}MHz fps={} temp={}°C screen={} io_pct={:.1} mem_pct={:.1} cpu_avg={:.1}",
            if snap.touch.down { "DOWN" } else { "up" },
            snap.gpu.cur_freq_hz / 1_000_000,
            snap.fps,
            snap.temp_c() as i32,
            snap.screen_on,
            snap.io.some_pct,
            mem_pct,
            snap.cpu_util_avg(),
        );
    }
    snap
}

// ---- 内部 push 函数 (各采集器 tick 末尾调用) ----

/// push 触摸状态 (TouchMonitor 调用)
pub(crate) fn touch_push(s: TouchState) {
    let h = sense_snapshot_handle();
    if let Ok(mut g) = h.lock() {
        g.touch = s;
        g.updated_at_ns = now_ns();
    }
}

/// push GPU 状态 (GpuMonitor 调用)
pub(crate) fn gpu_push(s: GpuState) {
    let h = sense_snapshot_handle();
    if let Ok(mut g) = h.lock() {
        g.gpu = s;
        g.updated_at_ns = now_ns();
    }
}

/// push IO 状态 (IoMonitor 调用)
pub(crate) fn io_push(s: IoState) {
    let h = sense_snapshot_handle();
    if let Ok(mut g) = h.lock() {
        g.io = s;
        g.updated_at_ns = now_ns();
    }
}

/// push Swap 状态 (SwapMonitor 调用)
pub(crate) fn swap_push(s: SwapState) {
    let h = sense_snapshot_handle();
    if let Ok(mut g) = h.lock() {
        g.swap = s;
        g.updated_at_ns = now_ns();
    }
}

/// push 温度
pub(crate) fn temp_push(millic: i32) {
    let h = sense_snapshot_handle();
    if let Ok(mut g) = h.lock() {
        g.temp_millic = millic;
        g.updated_at_ns = now_ns();
    }
}

/// push FPS
pub(crate) fn fps_push(fps: u32) {
    let h = sense_snapshot_handle();
    if let Ok(mut g) = h.lock() {
        g.fps = fps;
        g.updated_at_ns = now_ns();
    }
}

/// push 屏幕刷新率 (hz_poller 调用, ~2s tick)
pub(crate) fn hz_push(hz: f32) {
    let h = sense_snapshot_handle();
    if let Ok(mut g) = h.lock() {
        g.display_hz = hz;
        g.updated_at_ns = now_ns();
    }
}

/// push 屏幕状态
pub(crate) fn screen_push(on: bool) {
    let h = sense_snapshot_handle();
    if let Ok(mut g) = h.lock() {
        g.screen_on = on;
        g.updated_at_ns = now_ns();
    }
}

/// 轻量读取屏幕状态 (hotplug 200ms tick 专用).
///
/// 与 [`sense_now`] 不同: **不**触发 CPU 快照刷新 (hotplug 自己拉 idle_snapshot),
/// 只锁 O(1). sense_snapshot 尚未初始化 / 锁中毒时返回 `true` —
/// 读不到屏幕状态按"亮屏"处理是保守方向 (亮屏白名单更大, 全开倾向, 不丢性能).
pub fn screen_on_now() -> bool {
    sense_snapshot_handle()
        .lock()
        .map(|g| g.screen_on)
        .unwrap_or(true)
}

// =================================================================
//  4. 单元测试 (不依赖文件系统, 纯内存 push/pull)
// =================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sense_snapshot_default_is_zero() {
        let s = SenseSnapshot::default();
        assert!(!s.touch.down);
        assert_eq!(s.gpu.cur_freq_hz, 0);
        assert_eq!(s.io.some_us, 0);
        assert_eq!(s.swap.swap_total_kb, 0);
        assert_eq!(s.temp_millic, 0);
        assert_eq!(s.fps, 0);
        assert!(!s.screen_on);
        // Phase 2: cpu 字段也参与 default, 空 Vec → util_avg = 0.0
        assert!(s.cpu.cpus.is_empty());
        assert_eq!(s.cpu_util_avg(), 0.0);
    }

    #[test]
    fn sense_now_roundtrip() {
        touch_push(TouchState {
            down: true,
            last_event_age_ms: 100,
            device_path: "/dev/input/event7",
            updated_at_ns: now_ns(),
            ..Default::default()
        });
        let s = sense_now();
        assert!(s.touch.down);
        assert_eq!(s.touch.device_path, "/dev/input/event7");
        assert_eq!(s.touch.last_event_age_ms, 100);
    }

    #[test]
    fn touch_active_window() {
        let mut s = SenseSnapshot::default();
        s.touch.last_event_age_ms = 50;
        assert!(s.touch_active_within(100));
        assert!(!s.touch_active_within(20));
    }

    #[test]
    fn io_pressure_threshold() {
        let mut s = SenseSnapshot::default();
        s.io.some_pct = 35.0;
        assert!(!s.io_pressure_high());
        s.io.some_pct = 45.0;
        assert!(s.io_pressure_high());
    }

    #[test]
    fn temp_celsius_conversion() {
        let mut s = SenseSnapshot::default();
        s.temp_millic = 45000;
        assert!((s.temp_c() - 45.0).abs() < 0.01);
        s.temp_millic = i32::MIN;
        assert!(s.temp_c().is_nan());
    }
}


