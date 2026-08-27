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

/// CPU 永远不被 disable 的白名单基线 (D2: cpu0/1)
///
/// `cpu0` = SMP boot CPU, 关了 kernel panic — 这是内核安全底线, 无论用户怎么配都保留.
/// `cpu1` = 同 cluster A510, 与 cpu0 共 cache, 关了收益最大但风险也最大.
/// `baseline 实测`: cpu0/1 capacity=379, 确实是 LITTLE
///
/// 实际生效的白名单来自用户配置 (`screen_on_keep_cores` / `screen_off_keep_cores`),
/// 见 [`CpuAllowList::from_keep_cores`] — cpu0 恒保护 + 至少 2 核的内建约束在那里实现.
pub const PROTECTED_CPUS: &[u32] = &[0, 1];

/// 系统 CPU 总数上限 (决策只认 cpu0..=7, 8 核 SoC)
pub const MAX_CPU_ID: u32 = 7;

/// disable 触发需要的连续 tick 数 (D3: 200ms tick * 5 = 1s debounce)
pub const DISABLE_DEBOUNCE_TICKS: u32 = 5;

/// enable 触发需要的连续 tick 数 (200ms * 2 = 400ms, FAS panic 时立刻 enable 不需要 debounce)
pub const ENABLE_DEBOUNCE_TICKS: u32 = 2;

/// thermal 强制全开的默认温度阈值 (°C)
pub const THERMAL_FORCE_ALL_ON_C: f32 = 70.0;

/// 主动关核后的 enable 冷却窗 (ms):
/// offline 核在 /proc/stat 里统计冻结, 刚关掉的核会读到失真利用率.
/// 禁止在这个窗口内因该失真数据把核再次拉起, 防 关↔开 抖动.
pub const DISABLE_REENABLE_COOLDOWN_MS: i64 = 1500;

/// 最少在线核数 (漏洞 2: 防止 cpu2-7 全关导致通知/闹钟卡顿)
/// 基线实测 cpu0/1 是 LITTLE (capacity=379), cpu2-7 包含 4×A710 + 2×A510.
/// 通知/闹钟唤醒时, 至少 4 核就绪避免冷启动延迟.
pub const MIN_ONLINE_CORES: u32 = 4;

/// 触摸开核后保护窗口 (ms) — 漏洞 1: 开核后短时间内不允许关核
/// 可配置化 (modules.touch.{on,off}.duration_ms): daemon 启动/配置热重载/
/// 屏幕切换时由 scheduler 更新; 200ms 为历史默认.
pub const TOUCH_PROTECT_MS: i64 = 200;

/// 运行期触摸保护窗 (由 modules.touch 双套配置驱动; 初值 = 历史默认)
pub static TOUCH_PROTECT_MS_RUNTIME: std::sync::atomic::AtomicI64 =
    std::sync::atomic::AtomicI64::new(TOUCH_PROTECT_MS);

/// 更新触摸保护窗 (mod.rs 每 tick 从全局配置快照写入; 幂等)
pub fn set_touch_protect_ms(ms: i64) {
    TOUCH_PROTECT_MS_RUNTIME.store(ms.clamp(50, 2000), std::sync::atomic::Ordering::Relaxed);
}

#[inline]
pub fn touch_protect_ms() -> i64 {
    TOUCH_PROTECT_MS_RUNTIME.load(std::sync::atomic::Ordering::Relaxed)
}

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
    /// SoC 温度达到此值时强制全核在线 (°C, WebUI 可调, 默认 70.0)
    pub thermal_force_all_on_c: f32,
}

impl Default for HotplugThresholds {
    fn default() -> Self {
        Self {
            off_threshold_idle_pct: 95.0,
            on_threshold_util_pct: 30.0,
            min_online_cores: MIN_ONLINE_CORES,
            thermal_force_all_on_c: THERMAL_FORCE_ALL_ON_C,
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
#[derive(Debug, Clone)]
pub struct CpuAllowList {
    pub additional_protected: HashSet<u32>,
}

impl Default for CpuAllowList {
    /// 默认 = PROTECTED_CPUS (cpu0/1), 与历史行为一致
    fn default() -> Self {
        Self { additional_protected: PROTECTED_CPUS.iter().copied().collect() }
    }
}

impl CpuAllowList {
    /// 该 cpu 是否在白名单内
    ///
    /// 安全约束: `cpu0` = SMP boot CPU, **无论配置如何永远保护** (关了 kernel panic).
    pub fn is_protected(&self, cpu_id: u32) -> bool {
        if cpu_id == 0 { return true; }
        self.additional_protected.contains(&cpu_id)
    }

    /// 从用户配置的 keep_cores 构造合法白名单 (任务 A 安全约束内建):
    ///
    /// 1. 只接受 0..=MAX_CPU_ID (越界值静默丢弃, 去 duplicates)
    /// 2. `cpu0` 强制加入 (boot CPU 绝对不能关)
    /// 3. 有效核心数 < 2 时自动补 `cpu1` (防止全关; WebUI 层还会前置校验 >= 2)
    ///
    /// # Examples (语义说明, 非 doc-test — 本 crate 为 bin-only)
    ///
    /// - `from_keep_cores(&[0,1,2,3,4,5])` → 保护 {0..5}, cpu6/7 可动态关
    /// - `from_keep_cores(&[])`            → 兜底保护 {0,1}
    /// - `from_keep_cores(&[4])`           → 保护 {0,4} (已满足至少 2 个保护核)
    /// - `from_keep_cores(&[0,1,99])`      → 越界值丢弃
    pub fn from_keep_cores(cores: &[u32]) -> Self {
        let mut set: HashSet<u32> =
            cores.iter().copied().filter(|&c| c <= MAX_CPU_ID).collect();
        set.insert(0); // boot CPU 底线
        if set.len() < 2 {
            set.insert(1); // 至少保留 2 核
        }
        Self { additional_protected: set }
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
    /// 我方主动 disable 的时间戳 (0 = 不是我方关的).
    /// 用于防振荡: offline 核的 /proc/stat 统计冻结, util 读数不可信,
    /// 若在关核后立刻拿这个垃圾值做 enable 判定会形成 关↔开 抖动.
    disabled_at_unix_ms: i64,
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
        // --- 0. 热保护 (超过配置的温度阈值 → 全开) ---
        if thermal_c >= self.thresholds.thermal_force_all_on_c {
            let to_enable: Vec<u32> = (0..=MAX_CPU_ID)
                .filter(|&c| !self.state(c).online)
                .collect();
            // Bug 1 修复: 不调 self.mark, 等 mod.rs 拿到 sysfs 返回才 mark
            // 预填 enable_debounce 防下一 tick 重复决策
            for cpu_id in to_enable.iter().copied() {
                let s = self.per_cpu.entry(cpu_id).or_default();
                s.enable_debounce = ENABLE_DEBOUNCE_TICKS;
                s.disable_debounce = 0;
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
            let to_enable: Vec<u32> = (0..=MAX_CPU_ID)
                .filter(|&c| !self.state(c).online)
                .collect();
            for cpu_id in to_enable.iter().copied() {
                let s = self.per_cpu.entry(cpu_id).or_default();
                s.enable_debounce = ENABLE_DEBOUNCE_TICKS;
                s.disable_debounce = 0;
            }
            return HotplugDecision {
                to_enable,
                to_disable: vec![],
                external_override: vec![],
            };
        }

        // --- 2. FAS panic: 立即 enable 所有 ---
        if fas_panic {
            let to_enable: Vec<u32> = (0..=MAX_CPU_ID)
                .filter(|&c| !self.state(c).online)
                .collect();
            // Bug 1 修复: 不调 self.mark, 等 mod.rs 拿到 sysfs 返回才 mark
            for cpu_id in to_enable.iter().copied() {
                let s = self.per_cpu.entry(cpu_id).or_default();
                s.disable_debounce = 0;
                s.enable_debounce = ENABLE_DEBOUNCE_TICKS;
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
        // 失明修复: 遍历 0..=7 全集而非 loads — cpu_monitor 只为在线核产出条目,
        // 离线核不在 loads 里, 旧实现"唤醒全部核心"对失明核永远不生效.
        if touch_down {
            let to_enable: Vec<u32> = (0..=MAX_CPU_ID)
                .filter(|&c| !self.state(c).online)
                .collect();
            // 所有核 (含已 online) 刷 cooldown, 防止下一 tick 立刻关掉
            // (窗口时长可配置: modules.touch.{on,off}.duration_ms)
            let protect_ms = touch_protect_ms();
            for c in 0..=MAX_CPU_ID {
                let s = self.per_cpu.entry(c).or_default();
                s.touch_cooldown_until_ms = now_ms + protect_ms;
            }
            // 给要 enable 的核预填 enable_debounce, 防止下一 tick 重复决策
            for cpu_id in to_enable.iter().copied() {
                let s = self.per_cpu.entry(cpu_id).or_default();
                s.enable_debounce = ENABLE_DEBOUNCE_TICKS;
                s.disable_debounce = 0;
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

        // 白名单强制在线: keep_cores 内但当前 offline 的核立即拉起 (bypass debounce).
        // 典型场景: 息屏白名单 [0,1] → 亮屏切 [0..5], cpu2-5 应在下个 tick 立刻回线上,
        // 否则亮屏瞬间只剩 2 个小核跑, 用户感觉卡顿 (任务 A 的动机).
        // Bug 1 约束不变: 只收集列表, 等 mod.rs sysfs 写成功后才 mark_online.
        // 失明修复 (真机 bug): cpu_monitor 只为在线核产出 load 条目, 离线保护核不在
        // loads 里, 旧实现遍历 loads 永远看不到它们 → cpu3/5 滞留离线且 min_online
        // 只能拦关不能拉起. 改为遍历 0..=MAX_CPU_ID 全集 (per_cpu.keys() 不行 —
        // 初始空 map / 从未被 mark 的核同样失明).
        for cpu_id in 0..=MAX_CPU_ID {
            if self.allow_list.is_protected(cpu_id) && !self.state(cpu_id).online {
                to_enable.push(cpu_id);
                let s = self.per_cpu.entry(cpu_id).or_default();
                s.enable_debounce = ENABLE_DEBOUNCE_TICKS; // 防下一 tick 重复决策
                s.disable_debounce = 0;
            }
        }

        for load in loads {
            let cpu_id = load.cpu_id;
            let entry = self.per_cpu.entry(cpu_id).or_default();
            let prev_online = entry.online;

            // 3a. enable 决策 (Bug 1 修复后: 只收集列表, 不改 entry.online)
            if !prev_online && load.util_pct >= self.thresholds.on_threshold_util_pct {
                // 防振荡冷却: 我方刚关掉的核, 短窗内 offline util 统计失真, 不做 enable 决策.
                // (保护核强制在线分支在上面已先行处理, 走不到这里)
                if now_ms.saturating_sub(entry.disabled_at_unix_ms) < DISABLE_REENABLE_COOLDOWN_MS {
                    entry.disable_debounce = 0;
                    continue;
                }
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
        self.mark_online_at(cpu_id, online, 0);
    }

    /// 同 [`mark_online`], 附带决策时刻 (unix ms).
    /// disable 成功时打点, 驱动 [`DISABLE_REENABLE_COOLDOWN_MS`] 防振荡冷却;
    /// enable 成功时清零冷却.
    pub fn mark_online_at(&mut self, cpu_id: u32, online: bool, now_ms: i64) {
        let entry = self.per_cpu.entry(cpu_id).or_default();
        entry.online = online;
        entry.enable_debounce = 0;
        entry.disable_debounce = 0;
        entry.disabled_at_unix_ms = if online { 0 } else { now_ms };
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

    /// decider 视图中单核的 online 状态 (供 mod.rs 与 sysfs 实际值对账).
    /// 外部 (用户 echo / 其他 governor) 改核状态时 decider 收不到通知,
    /// 视图漂移会让白名单强制在线分支误以为核还在线而永不拉起.
    pub fn is_cpu_online_view(&self, cpu_id: u32) -> bool {
        self.state(cpu_id).online
    }

    pub fn thresholds(&self) -> HotplugThresholds {
        self.thresholds
    }

    pub fn set_thresholds(&mut self, t: HotplugThresholds) {
        self.thresholds = t;
    }

    /// 更新 keep_cores 白名单 (任务 A: 屏幕状态切换 / 配置热更新时每 tick 调用).
    ///
    /// - 内部走 [`CpuAllowList::from_keep_cores`], cpu0 恒保护 + 至少 2 核的约束在那里兜底
    /// - 返回 true 表示白名单发生了变化 (调用方可记日志)
    /// - 变化时重置所有 per-cpu debounce, 防止旧白名单下的计数污染新决策
    ///   (新保护核的下 tick 强制在线逻辑会接管拉起)
    pub fn set_keep_cores(&mut self, cores: &[u32]) -> bool {
        let new_list = CpuAllowList::from_keep_cores(cores);
        if new_list.additional_protected == self.allow_list.additional_protected {
            return false;
        }
        for st in self.per_cpu.values_mut() {
            st.disable_debounce = 0;
            st.enable_debounce = 0;
        }
        self.allow_list = new_list;
        true
    }

    /// 当前生效的白名单快照 (升序), 用于 state.yaml 输出与日志
    pub fn active_keep_cores(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.allow_list.additional_protected.iter().copied().collect();
        if !v.contains(&0) { v.push(0); } // is_protected 对 cpu0 恒真, 保持输出一致
        v.sort_unstable();
        v
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
        // Bug1 collect-only 改造后 per_cpu.online 由 mod.rs mark; 单测需要预置在线状态.
        // 且必须全核预置 (current_online=8 > min_online=4), 否则 disable 被
        // 漏洞2 最少在线守卫挡住 — 这正是本套件在 collect-only 改造后失绿的根因.
        for i in 0..8u32 { d.mark_online(i, true); }
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

    // ============ 任务 A: 可配置保留核心 (keep_cores) ============

    #[test]
    fn from_keep_cores_cpu0_always_included() {
        let al = CpuAllowList::from_keep_cores(&[]);
        assert!(al.is_protected(0), "boot CPU must always be protected");
    }

    #[test]
    fn keep_cores_minimum_two_online_protected() {
        // 空白名单 / 只勾一个核心时自动兜底到 {cpu0, cpu1} (任务 A 安全约束 #2)
        assert!(CpuAllowList::from_keep_cores(&[]).is_protected(1));
        assert!(CpuAllowList::from_keep_cores(&[0]).is_protected(1));
        // 只勾一个非 boot 核心时: 保护集 = {cpu0, 该核}, 已满足"至少 2 个保护核"
        let al = CpuAllowList::from_keep_cores(&[4]);
        assert!(al.is_protected(0) && al.is_protected(4));
        assert!(!al.is_protected(1));
        assert!(!al.is_protected(3));
    }

    #[test]
    fn from_keep_cores_drops_out_of_range() {
        let al = CpuAllowList::from_keep_cores(&[0, 1, 99]);
        assert!(al.is_protected(0) && al.is_protected(1));
        assert!(!al.is_protected(99));
        assert_eq!(al.additional_protected.len(), 2);
    }

    #[test]
    fn screen_on_six_cores_only_six_and_seven_can_sleep() {
        let mut d = ThresholdDecider::new(HotplugThresholds::default(),
            CpuAllowList::from_keep_cores(&[0, 1, 2, 3, 4, 5]));
        // 预置全部核心在线 (模拟 mark 已经由 mod.rs 完成)
        for i in 0..8u32 { d.mark_online(i, true); }
        let loads = || vec![
            load(0, 10.0), load(1, 10.0), load(2, 10.0),
            load(3, 10.0), load(4, 10.0), load(5, 10.0),
            load(6, 99.0), load(7, 50.0),
        ];
        // 亮屏白名单 [0..5]: 即使 cpu6 长时间空闲, 也只有 6/7 允许出现在关核候选里;
        // 且 debounce 攒满 DISABLE_DEBOUNCE_TICKS 后 cpu6 才真正进入 to_disable.
        for tick in 1..=DISABLE_DEBOUNCE_TICKS + 1 {
            let dec = d.tick(&loads(), 50.0, false, true, false, 0);
            assert!(dec.to_disable.iter().all(|&c| c == 6),
                "only non-protected core (cpu6) can be disabled, got {:?}", dec.to_disable);
            if tick == DISABLE_DEBOUNCE_TICKS {
                assert!(dec.to_disable.contains(&6));
            }
        }
    }

    #[test]
    fn protected_core_marked_offline_is_force_enabled() {
        // 场景: 息屏用 [0,1], cpu3 被关; 切亮屏白名单 [0..5] → cpu3 必须立即拉起
        let mut d = ThresholdDecider::new(HotplugThresholds::default(), CpuAllowList::default());
        d.mark_online(3, false); // 模拟息屏时被关
        assert!(d.set_keep_cores(&[0, 1, 2, 3, 4, 5]), "whitelist change should be detected");
        let dec = d.tick(&[load(3, 90.0)], 45.0, false, true, false, 0);
        assert!(dec.to_enable.contains(&3), "protected offline core must be force-enabled");
    }

    #[test]
    fn set_keep_cores_idempotent_change_detection() {
        let mut d = ThresholdDecider::new(HotplugThresholds::default(), CpuAllowList::default());
        assert!(d.set_keep_cores(&[0, 1, 2, 3, 4, 5]));
        assert!(!d.set_keep_cores(&[5, 4, 3, 2, 1, 0]), "same set in any order = no change");
        assert!(d.set_keep_cores(&[0, 1]));
    }

    #[test]
    fn active_keep_cores_reflects_current_whitelist() {
        let mut d = ThresholdDecider::new(HotplugThresholds::default(), CpuAllowList::default());
        d.set_keep_cores(&[0, 2, 5]);
        // 3 个有效保护核 (>= 2), 不触发兜底; cpu0 即使没写在配置里也永远在内
        assert_eq!(d.active_keep_cores(), vec![0, 2, 5]);
        d.set_keep_cores(&[]);
        assert_eq!(d.active_keep_cores(), vec![0, 1]); // 空配置 → {cpu0,cpu1} 兜底
    }

    #[test]
    fn disable_reenable_cooldown_blocks_spurious_enable() {
        // 场景 (真机观测到的振荡): 我方关掉 cpu6 后, /proc/stat 中该核统计冻结,
        // util 读数失真地高 → 修复前下一 tick 就被重新 enable, 形成 关↔开 抖动.
        let mut d = ThresholdDecider::new(HotplugThresholds::default(),
            CpuAllowList::from_keep_cores(&[0, 1, 2, 3, 4, 5]));
        for i in 0..8u32 { d.mark_online_at(i, true, 10_000); }
        // t=10_000..10_800: cpu6 关核 (debounce 攒满 5 tick 后进入 to_disable)
        for k in 0..DISABLE_DEBOUNCE_TICKS {
            let dec = d.tick(&[load(6, 99.0)], 45.0, false, true, false, 10_000 + (k as i64) * 200);
            if k == DISABLE_DEBOUNCE_TICKS - 1 {
                assert!(dec.to_disable.contains(&6));
            }
        }
        d.mark_online_at(6, false, 10_800);
        // t=10_200 (1 tick 后): 失真的 util=100 不允许触发 enable
        let dec = d.tick(&[load(6, 100.0)], 45.0, false, true, false, 10_200);
        assert!(!dec.to_enable.contains(&6), "spurious re-enable within cooldown");
        // t=12_500/12_700 (冷却窗 1500ms 已过): 连续两次高负载采样正常唤醒
        // 注意 load() 第二参是 idle → util = 100-idle
        let dec = d.tick(&[load(6, 0.0)], 45.0, false, true, false, 12_500);
        assert!(!dec.to_enable.contains(&6), "enable debounce needs 2 ticks");
        let dec = d.tick(&[load(6, 0.0)], 45.0, false, true, false, 12_700);
        assert!(dec.to_enable.contains(&6), "must be enable-able after cooldown");
    }

    // ============ 失明死锁回归 (真机 bug 2026-08-27): 离线核脱离 loads 视野 ============
    //
    // 根因: cpu_monitor 只为在线核产出 load 条目 (只遍历 online_cpus_list),
    // 离线核完全不在 loads 里. 旧实现的拉起路径全部遍历 loads → 保护核一旦在
    // 合法窗口 (灭屏白名单切换 / 短暂 idle) 被摘, 就再无路径能拉起, 永久滞留离线
    // (真机观测: online_mask=0x17={0,1,2,4}, 白名单 [0..5] 内的 cpu3/5 失明).
    // 注意: 这些测试的 loads 故意只含在线核 — 与生产数据形状一致;
    // 旧测试 protected_core_marked_offline_is_force_enabled 给离线核手工造了
    // load 条目, 与生产不符, 因此没能拦住这个 bug.

    #[test]
    fn whitelist_force_online_sees_blind_offline_cores() {
        // 用户指定场景: online={0,1,2,4} (mask 0x17), 白名单 [0..5]
        // → tick 后 cpu3 和 cpu5 必须出现在 to_enable (1 tick 内)
        let mut d = ThresholdDecider::new(HotplugThresholds::default(),
            CpuAllowList::from_keep_cores(&[0, 1, 2, 3, 4, 5]));
        for i in [0u32, 1, 2, 4] { d.mark_online(i, true); }
        // 关键: loads 只含在线核 (生产形状), cpu3/5 "失明"
        let loads = vec![load(0, 50.0), load(1, 50.0), load(2, 50.0), load(4, 50.0)];
        let dec = d.tick(&loads, 45.0, false, true, false, 10_000);
        assert!(dec.to_enable.contains(&3), "blind offline protected cpu3 must be force-enabled");
        assert!(dec.to_enable.contains(&5), "blind offline protected cpu5 must be force-enabled");
        // 非保护失明核 (cpu6/7) 不被白名单分支误拉 — 它们仍由负载 enable 决策管理
        assert!(!dec.to_enable.contains(&6) && !dec.to_enable.contains(&7),
            "non-protected blind cores must not be woken by whitelist branch, got {:?}",
            dec.to_enable);
    }

    #[test]
    fn touch_bypass_sees_blind_offline_cores() {
        // 触摸旁路语义 = "唤醒全部核心"; 离线核不在 loads 里时旧实现同样拉不起
        let mut d = ThresholdDecider::new(HotplugThresholds::default(), CpuAllowList::default());
        for i in [0u32, 1, 2, 4] { d.mark_online(i, true); }
        let loads = vec![load(0, 50.0), load(1, 50.0), load(2, 50.0), load(4, 50.0)];
        let dec = d.tick(&loads, 45.0, false, true, true, 10_000); // touch_down=true
        for c in [3u32, 5, 6, 7] {
            assert!(dec.to_enable.contains(&c), "touch must wake blind offline cpu{}", c);
        }
    }

    #[test]
    fn screen_off_to_on_restores_whitelist_within_2_ticks() {
        // 端到端: 灭屏白名单 [0,1], cpu2-5 被摘 (loads 同步失明, 只剩 cpu0/1);
        // 亮屏切 [0..5] → 第 1 tick cpu2-5 全部进 to_enable;
        // mod.rs sysfs 写成功 mark_online 后, 第 2 tick 幂等 (不重复决策)
        let mut d = ThresholdDecider::new(HotplugThresholds::default(),
            CpuAllowList::from_keep_cores(&[0, 1]));
        for i in [0u32, 1] { d.mark_online(i, true); }
        let loads = vec![load(0, 50.0), load(1, 50.0)];
        let dec = d.tick(&loads, 45.0, false, true, false, 10_000);
        assert!(dec.to_enable.is_empty() && dec.to_disable.is_empty(),
            "screen-off whitelist [0,1] with only cpu0/1 online: nothing to do");

        // 亮屏: 白名单切换 (mod.rs 每 tick 调 set_keep_cores)
        d.set_keep_cores(&[0, 1, 2, 3, 4, 5]);
        let dec = d.tick(&loads, 45.0, false, true, false, 10_200);
        for c in [2u32, 3, 4, 5] {
            assert!(dec.to_enable.contains(&c),
                "cpu{} must be force-enabled on screen-on tick 1", c);
        }
        // 模拟 mod.rs 写 sysfs 成功 → mark_online_at; 第 2 tick 必须幂等
        for c in [2u32, 3, 4, 5] { d.mark_online_at(c, true, 10_200); }
        let dec = d.tick(&loads, 45.0, false, true, false, 10_400);
        assert!(dec.is_empty(),
            "tick 2 must be idempotent after mark_online, got to_enable={:?}", dec.to_enable);
    }
}