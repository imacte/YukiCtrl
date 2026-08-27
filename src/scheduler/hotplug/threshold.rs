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

//! 热插拔阈值判定 (D3, D6)
//!
//! 设计要点:
//! - **per-core** 独立 hysteresis, 不 cluster-wide (D3)
//! - 两个阈值: `off_threshold_idle_pct` / `on_threshold_util_pct`
//! - 触发 disable: `idle_pct > off_threshold` 持续 `DISABLE_DEBOUNCE_TICKS` 个 tick
//! - 触发 enable:  `util_pct > on_threshold` OR FAS panic
//! - D2 白名单: cpu0/1 永远不被 disable

use std::collections::HashSet;

/// CPU 永远不被 disable 的白名单 (D2: cpu0/1)
///
/// `cpu0` = SMP boot CPU, 关了 kernel panic
/// `cpu1` = 同 cluster A510, 与 cpu0 共 cache, 关了收益最大但风险也最大
/// `baseline 实测`: cpu0/1 capacity=379, 确实是 LITTLE
pub const PROTECTED_CPUS: &[u32] = &[0, 1];

/// disable 触发需要的连续 tick 数 (D3: 200ms tick * 5 = 1s debounce)
pub const DISABLE_DEBOUNCE_TICKS: u32 = 5;

/// enable 触发需要的连续 tick 数 (200ms * 2 = 400ms, FAS panic 时立刻 enable 不需要 debounce)
pub const ENABLE_DEBOUNCE_TICKS: u32 = 2;

/// thermal 强制全开的温度阈值 (°C)
pub const THERMAL_FORCE_ALL_ON_C: f32 = 70.0;

/// 最少在线核数 (漏洞 2: 防止 cpu2-7 全关导致通知/闹钟卡顿)
/// 基线实测 cpu0/1 是 LITTLE (capacity=379), cpu2-7 包含 4×A710 + 2×A510.
/// 通知/闹钟唤醒时, 至少 4 核就绪避免冷启动延迟.
pub const MIN_ONLINE_CORES: u32 = 4;

/// 触摸开核后保护窗口 (ms) — 漏洞 1: 开核后短时间内不允许关核
pub const TOUCH_PROTECT_MS: i64 = 200;

/// 单个 CPU 的 idle/util 快照, 由 cpu_monitor 提供
#[derive(Debug, Clone, Copy)]
pub struct CpuLoad {
    pub cpu_id: u32,
    /// 0..=100 (空闲百分比)
    pub idle_pct: f32,
    /// 0..=100 (工作百分比, 100 - idle_pct 简化)
    pub util_pct: f32,
}

/// 阈值配置 (D6: WebUI 可调)
#[derive(Debug, Clone, Copy)]
pub struct HotplugThresholds {
    /// idle_pct 高于此值且持续 DISABLE_DEBOUNCE_TICKS tick 才允许 disable
    pub off_threshold_idle_pct: f32,
    /// util_pct 高于此值持续 ENABLE_DEBOUNCE_TICKS tick 才 enable
    pub on_threshold_util_pct: f32,
    /// 漏洞 2: 至少保留在线核数 (WebUI 可调, 默认 MIN_ONLINE_CORES)
    pub min_online_cores: u32,
}

impl Default for HotplugThresholds {
    fn default() -> Self {
        Self {
            off_threshold_idle_pct: 95.0,
            on_threshold_util_pct: 30.0,
            min_online_cores: MIN_ONLINE_CORES,
        }
    }
}

/// WebUI toggle (D4)
#[derive(Debug, Clone, Copy, Default)]
pub struct HotplugToggles {
    /// 锁屏时禁用热插拔 (关屏后 cpu 全开, 避免唤醒延迟)
    pub lockscreen_onoff: bool,
    /// 灭屏时禁用热插拔 (Doze 模式不需要动态)
    pub screens_onoff: bool,
}

/// 用户场景下哪些 cpu 不参与 hotplug 决策
#[derive(Debug, Clone, Default)]
pub struct CpuAllowList {
    pub additional_protected: HashSet<u32>,
}

impl CpuAllowList {
    /// 该 cpu 是否在白名单内 (D2: cpu0/1 + 用户额外加的)
    pub fn is_protected(&self, cpu_id: u32) -> bool {
        PROTECTED_CPUS.contains(&cpu_id) || self.additional_protected.contains(&cpu_id)
    }
}

/// 单个 CPU 的 hotplug 内部状态 (per-core 独立, D3)
#[derive(Debug, Clone, Copy, Default)]
struct CpuHotplugState {
    online: bool,
    disable_debounce: u32,
    enable_debounce: u32,
    /// 漏洞 1: 触摸开核后保护窗口 (Unix epoch ms). 在此之前不允许 disable
    touch_cooldown_until_ms: i64,
}

/// 决策结果
#[derive(Debug, Default, Clone)]
pub struct HotplugDecision {
    /// 本 tick 需要 enable 的 cpu 列表
    pub to_enable: Vec<u32>,
    /// 本 tick 需要 disable 的 cpu 列表
    pub to_disable: Vec<u32>,
    /// Bug 2 标记: sysfs 写出去后回读发现值不等于目标 → 这些 cpu 被外部模块接管
    /// mod.rs 在 run_one_tick 里检查这个, 把它们的 entry.online 强制设回实际值
    pub external_override: Vec<(u32, bool)>,
}

impl HotplugDecision {
    pub fn is_empty(&self) -> bool {
        self.to_enable.is_empty() && self.to_disable.is_empty()
    }
}

/// 热插拔判定器 (无状态可配置, 但内部维护 per-cpu debounce)
pub struct ThresholdDecider {
    thresholds: HotplugThresholds,
    allow_list: CpuAllowList,
    per_cpu: std::collections::HashMap<u32, CpuHotplugState>,
}

impl ThresholdDecider {
    pub fn new(thresholds: HotplugThresholds, allow_list: CpuAllowList) -> Self {
        Self {
            thresholds,
            allow_list,
            per_cpu: Default::default(),
        }
    }

    /// 单 tick 决策 (200ms 周期)
    ///
    /// # Arguments
    /// * `loads` - 本 tick 所有 cpu 的 idle/util 快照
    /// * `thermal_c` - 当前 SoC thermal 温度 (°C)
    /// * `fas_panic` - FAS 是否丢帧 (D5: true 时立即 enable 所有, bypass debounce)
    /// * `enabled` - 全局 enable (lockscreen_onoff || screens_onoff 关闭时为 false)
    /// * `touch_down` - 漏洞 1: 当前是否有手指按压, true 时立即 enable 所有核 + 开核后保护 200ms 不关
    /// * `now_ms` - 当前 Unix epoch ms, 用于触摸保护窗口计时
    pub fn tick(
        &mut self,
        loads: &[CpuLoad],
        thermal_c: f32,
        fas_panic: bool,
        enabled: bool,
        touch_down: bool,
        now_ms: i64,
    ) -> HotplugDecision {
        // --- 0. 热保护 (thermal > 70°C → 全开) ---
        if thermal_c >= THERMAL_FORCE_ALL_ON_C {
            let to_enable: Vec<u32> = loads
                .iter()
                .filter(|l| !self.state(l.cpu_id).online)
                .map(|l| l.cpu_id)
                .collect();
            // Bug 1 修复: 不调 self.mark, 等 mod.rs 拿到 sysfs 返回才 mark
            // 预填 enable_debounce 防下一 tick 重复决策
            for cpu_id in to_enable.iter().copied() {
                if let Some(s) = self.per_cpu.get_mut(&cpu_id) {
                    s.enable_debounce = ENABLE_DEBOUNCE_TICKS;
                    s.disable_debounce = 0;
                }
            }
            return HotplugDecision {
                to_enable,
                to_disable: vec![],
                external_override: vec![],
            };
        }

        // --- 1. 全局禁用 (lockscreen/灭屏) ---
        if !enabled {
            // Bug 1 修复: global bypass 改成只 collect + 预填 debounce
            let to_enable: Vec<u32> = loads.iter()
                .filter(|l| !self.state(l.cpu_id).online)
                .map(|l| l.cpu_id)
                .collect();
            for cpu_id in to_enable.iter().copied() {
                if let Some(s) = self.per_cpu.get_mut(&cpu_id) {
                    s.enable_debounce = ENABLE_DEBOUNCE_TICKS;
                    s.disable_debounce = 0;
                }
            }
            return HotplugDecision {
                to_enable,
                to_disable: vec![],
                external_override: vec![],
            };
        }

        // --- 2. FAS panic: 立即 enable 所有 ---
        if fas_panic {
            let to_enable: Vec<u32> = loads
                .iter()
                .filter(|l| !self.state(l.cpu_id).online)
                .map(|l| l.cpu_id)
                .collect();
            // Bug 1 修复: 不调 self.mark, 等 mod.rs 拿到 sysfs 返回才 mark
            for cpu_id in to_enable.iter().copied() {
                if let Some(s) = self.per_cpu.get_mut(&cpu_id) {
                    s.disable_debounce = 0;
                    s.enable_debounce = ENABLE_DEBOUNCE_TICKS;
                }
            }
            return HotplugDecision {
                to_enable,
                to_disable: vec![],
                external_override: vec![],
            };
        }

        // --- 2.5 触摸旁路 (漏洞 1) ---
        // touch_down=true 时立即 collect 所有 offline 核到 to_enable, 写 touch_cooldown_until_ms
        // 在 cooldown 窗口内 (200ms) 不允许 disable
        // Bug 1 修复: 不调 self.mark (它会改 entry.online), 等 mod.rs 拿到 sysfs 返回才 mark
        if touch_down {
            let to_enable: Vec<u32> = loads.iter()
                .filter(|l| !self.state(l.cpu_id).online)
                .map(|l| l.cpu_id).collect();
            // 所有核 (含已 online) 刷 cooldown, 防止下一 tick 立刻关掉
            for load in loads {
                if let Some(s) = self.per_cpu.get_mut(&load.cpu_id) {
                    s.touch_cooldown_until_ms = now_ms + TOUCH_PROTECT_MS;
                }
            }
            // 给要 enable 的核预填 enable_debounce, 防止下一 tick 重复决策
            for cpu_id in to_enable.iter().copied() {
                if let Some(s) = self.per_cpu.get_mut(&cpu_id) {
                    s.enable_debounce = ENABLE_DEBOUNCE_TICKS;
                    s.disable_debounce = 0;
                }
            }
            return HotplugDecision {
                to_enable, to_disable: vec![], external_override: vec![],
            };
        }

        // --- 3. 正常决策 (per-core hysteresis) ---
        let mut to_enable = Vec::new();
        let mut to_disable = Vec::new();

        // 漏洞 2: 统计当前 online 核数, 用于关核安全约束
        let mut current_online: u32 = 0;
        for (_, st) in &self.per_cpu {
            if st.online { current_online += 1; }
        }
        let min_online = self.thresholds.min_online_cores.max(2);

        for load in loads {
            let cpu_id = load.cpu_id;
            let entry = self.per_cpu.entry(cpu_id).or_default();
            let prev_online = entry.online;

            // 3a. enable 决策 (Bug 1 修复后: 只收集列表, 不改 entry.online)
            if !prev_online && load.util_pct >= self.thresholds.on_threshold_util_pct {
                entry.enable_debounce = entry.enable_debounce.saturating_add(1);
                entry.disable_debounce = 0;
                if entry.enable_debounce >= ENABLE_DEBOUNCE_TICKS {
                    to_enable.push(cpu_id);
                    entry.enable_debounce = 0;
                    // 注意: entry.online 不在这里改, 等 mod.rs 拿到 sysfs 返回后再改
                }
                continue;
            }

            // 3b. disable 决策
            //   优先级: protected (D2) > touch_cooldown (漏洞 1) > min_online (漏洞 2) > idle 阈值
            if prev_online && load.idle_pct >= self.thresholds.off_threshold_idle_pct {
                let blocked_by_protect = self.allow_list.is_protected(cpu_id);
                let blocked_by_touch_cooldown = now_ms < entry.touch_cooldown_until_ms;
                let blocked_by_min_online = current_online <= min_online;

                if !blocked_by_protect && !blocked_by_touch_cooldown && !blocked_by_min_online {
                    entry.disable_debounce = entry.disable_debounce.saturating_add(1);
                    entry.enable_debounce = 0;
                    if entry.disable_debounce >= DISABLE_DEBOUNCE_TICKS {
                        to_disable.push(cpu_id);
                        entry.disable_debounce = 0;
                        // 注意: entry.online 不在这里改, 等 mod.rs 拿到 sysfs 返回后再改
                    }
                    continue;
                }
                // 被保护: 重置 disable 计数器, 保留 enable 计数
                entry.disable_debounce = 0;
                continue;
            }

            // 3c. 既未 enable 也未 disable → 重置两个 debounce 计数
            entry.disable_debounce = 0;
            entry.enable_debounce = 0;
        }

        HotplugDecision {
            to_enable,
            to_disable,
            external_override: vec![],
        }
    }

    fn state(&self, cpu_id: u32) -> CpuHotplugState {
        self.per_cpu.get(&cpu_id).copied().unwrap_or_default()
    }

    fn mark(&mut self, cpu_id: u32, online: bool) {
        let entry = self.per_cpu.entry(cpu_id).or_default();
        entry.online = online;
        entry.enable_debounce = 0;
        entry.disable_debounce = 0;
    }

    /// Bug 1 彻底修复后的统一接口: mod.rs 在 sysfs 写返回成功后调此方法更新 decider 内部状态.
    ///   写入成功 (enable) → mark_online(cpu, true)
    ///   写入成功 (disable) → mark_online(cpu, false)
    ///   写入失败 → 不调 (entry.online 保持决策前真实状态, 下次 tick 自然重试)
    ///   回读发现外部覆盖 → mark_online(cpu, actual)
    /// 重置 debounce 计数是为了避免下一 tick 立即再次决策.
    pub fn mark_online(&mut self, cpu_id: u32, online: bool) {
        let entry = self.per_cpu.entry(cpu_id).or_default();
        entry.online = online;
        entry.enable_debounce = 0;
        entry.disable_debounce = 0;
    }

    /// 当前所有 cpu 的 online 状态 (bitmask, bit N = cpu N online?)
    pub fn online_mask(&self) -> u64 {
        let mut mask = 0u64;
        for (cpu_id, state) in &self.per_cpu {
            if state.online {
                mask |= 1u64 << cpu_id;
            }
        }
        mask
    }

    pub fn thresholds(&self) -> HotplugThresholds {
        self.thresholds
    }

    pub fn set_thresholds(&mut self, t: HotplugThresholds) {
        self.thresholds = t;
    }

    pub fn allow_list(&self) -> &CpuAllowList {
        &self.allow_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(cpu: u32, idle: f32) -> CpuLoad {
        CpuLoad {
            cpu_id: cpu,
            idle_pct: idle,
            util_pct: 100.0 - idle,
        }
    }

    #[test]
    fn cpu0_and_cpu1_never_disabled() {
        let mut d = ThresholdDecider::new(HotplugThresholds::default(), CpuAllowList::default());
        for _ in 0..20 {
            let loads = vec![load(0, 99.0), load(1, 99.0), load(2, 99.0), load(3, 50.0)];
            let dec = d.tick(&loads, 50.0, false, true, false, 0);
            assert!(!dec.to_disable.contains(&0), "cpu0 must never be disabled");
            assert!(!dec.to_disable.contains(&1), "cpu1 must never be disabled");
        }
    }

    #[test]
    fn idle_above_off_threshold_disables() {
        let mut d = ThresholdDecider::new(HotplugThresholds::default(), CpuAllowList::default());
        for _ in 0..3 {
            d.tick(&[load(2, 30.0)], 50.0, false, true, false, 0);
        }
        for tick in 1..=DISABLE_DEBOUNCE_TICKS + 1 {
            let dec = d.tick(&[load(2, 99.0)], 50.0, false, true, false, 0);
            if tick == DISABLE_DEBOUNCE_TICKS {
                assert!(dec.to_disable.contains(&2), "cpu2 should be disabled at tick {}", tick);
            }
        }
    }

    #[test]
    fn fas_panic_force_enable() {
        let mut d = ThresholdDecider::new(HotplugThresholds::default(), CpuAllowList::default());
        for _ in 0..DISABLE_DEBOUNCE_TICKS + 2 {
            d.tick(&[load(3, 99.0)], 50.0, false, true, false, 0);
        }
        let dec = d.tick(&[load(3, 99.0)], 50.0, true, true, false, 0);
        assert!(dec.to_enable.contains(&3), "fas_panic must force-enable cpu3");
    }

    #[test]
    fn thermal_above_70_force_all_on() {
        let mut d = ThresholdDecider::new(HotplugThresholds::default(), CpuAllowList::default());
        for _ in 0..DISABLE_DEBOUNCE_TICKS + 2 {
            d.tick(&[load(4, 99.0)], 50.0, false, true, false, 0);
        }
        let dec = d.tick(&[load(4, 99.0)], 75.0, false, true, false, 0);
        assert!(dec.to_enable.contains(&4), "thermal must force-enable cpu4");
    }

    #[test]
    fn hysteresis_prevents_immediate_re_enable() {
        let mut d = ThresholdDecider::new(HotplugThresholds::default(), CpuAllowList::default());
        d.tick(&[load(5, 30.0)], 50.0, false, true, false, 0);
        for _ in 0..DISABLE_DEBOUNCE_TICKS {
            d.tick(&[load(5, 99.0)], 50.0, false, true, false, 0);
        }
        let dec = d.tick(&[load(5, 10.0)], 50.0, false, true, false, 0);
        assert!(!dec.to_enable.contains(&5), "single tick spike should not re-enable");
    }
}