# Ticket 04 — 关核方案 D (Disable Hot Cores, Per-Frame)

## 目标

在小米14 Pro (骁龙 8 Gen 3, 8 核 big.LITTLE: 4×A510 + 3×A715 + 1×X4) 上,
**在游戏/前台应用切换时**,关掉部分高功耗核心, 降低 idle 漏电 + 释放 thermal budget.

## 方案对比 (用户原话 "关核方案D" — 我猜指 4 个方案中的第 D 个, 需确认)

| 方案 | 描述 | 风险 | 备注 |
|---|---|---|---|
| A | 关所有小核 (4×A510), 只留大核 | 漏电优化但损失 multi-thread | 不适合 multi-app |
| B | 关最高频 X4, 留 A715+A510 | 性能 -10% | thermal友好 |
| C | 全开 + thermal throttling | 不动核心, 依赖 SoC | baseline |
| **D** | **根据当前 FPS / util 动态开关** | 调度复杂 | **"方案D" 我假定是这个** |

**待 grill-me 确认**:方案 D = "动态基于 FPS / util 切换核心" 是不是用户原意? 还是字面意思的 "方案 D" 是其他定义 (例如 "Disable when Display off" 或别的).

## 方案 D 详细设计 (假定)

### 触发条件
- 进入游戏模式 (前台 app UID 1000+ + high refresh rate 90Hz+)
- OR FPS < target - 10% 持续 1s → 启 core (升核)
- OR FPS > target + 5% 持续 5s + 热传感器 < threshold → 关 core (降核)

### 实现层
1. **新增 BPF event**: 在 scheduler 模块加 `core_disable_request` map (PerCpuArray<u8>)
2. **新增 API**: `set_disabled_cpus(mask: u64)` — 写 `/sys/devices/system/cpu/cpuN/online`
3. **调度决策**: 在 `scheduler/scheduler.rs` 加 `DecideCoreMask(frame_pipeline) -> u64`
4. **WebUI**: 加 "核心映射" 视图, 显示当前 8 核 online/offline + 热传感器

### 设备差异
- 小米14 Pro: 4+3+1, big.LITTLE — A510 是 LITTLE, A715 是 MID, X4 是 BIG
- 8 核映射需 in-code 配置文件 (`config/cpu_map_xiaomi_14_pro.yaml`) 而不是 hardcode

### 安全约束 (必需)
1. **永远不关 cpu 0** (SMP boot CPU, 关了 kernel panic)
2. **至少保留 1 个 big core** (foreground app 主调度)
3. **切换间隔至少 500ms** (防止 hot plug 抖动)
4. **thermal > 70°C 时强制开全核** (cool down first)

## 风险 / 边界

1. **SoC hotplug 锁** — 部分手机 CPU hotplug 走 trustzone, 关核需要 root + 厂商允许
2. **kernel panic** — cpu 0 关掉必死, mask 必须 & !0x1
3. **应用迁移** — 关核后调度器需要把 running task 迁移, 大延迟 (~50ms) 可能 jank
4. **持久化** — 重启后 `/sys/.../online` 会回 default, module 启动时还原

## 测试 seam (RED → GREEN)

1. **RED**: 写 `tests/core_disable_decide.rs` — mock frame_pipeline, 断言
   - FPS drop → mask 增加 (开更多核)
   - FPS high + cool → mask 减少
   - cpu 0 永远在 mask 里
2. **GREEN**: 实现 `DecideCoreMask` + 集成进 scheduler.rs
3. **PHONE TEST** (grill-me 必问): **必须** 手机上跑, logcat 监控 panic 或 ANR

## 决策点 (grill-me 必问)

1. **方案 D 是什么** — 我上面假设对吗?
2. **关哪些核** — LITTLE / MID / BIG 优先级?
3. **触发窗口** — 启动延迟? 多久才能再次切换?
4. **用户开关** — WebUI 给用户 override 还是全自动?
5. **何时回全核** — app switch / 灭屏 / 低电?

---

## D1–D8 最终决策 (baseline 验证后敲定, 2026-08-26)

> 上面"方案 D 详细设计 (假定)"是 grill-me 前的草稿。这里是**实测后敲定的实施方案**。
> 这些决策来自 baseline 在 Xiaomi 14 Pro 上跑通后,基于实际 SELinux / hotplug 限制调整。

| 编号 | 决策 | 理由 / 实测依据 |
|---|---|---|
| **D1** | **all-scenario** 启用:游戏 / 普通 app / idle / 锁屏 全部场景都跑 hotplug 循环 | 用户原意是"全局省电+温控",不是只游戏场景 |
| **D2** | **永远白名单保护 cpu0/1** (2 个最低频小核), 其他 cpu (cpu2..cpu7) 才允许被关 | cpu0 = SMP boot CPU (不能关); cpu1 与 cpu0 同 cluster (A510),关 cpu0/1 反而省电收益最大化; baseline 实测 `cpu0/1 capacity=379`,确实是小核 |
| **D3** | **per-core 200ms tick** (5 Hz),独立 hysteresis band (每个 cpu 独立的 on/off 时间窗口) | 200ms 与现有 CLG 周期对齐,避免 race; per-core 而非 cluster-wide,更精细 |
| **D4** | **WebUI 给 2 个独立 toggle**:①`lockscreen-onoff` (锁屏时关核) ②`screens-onoff` (灭屏时关核) | 用户在 baseline WebUI 上要的精细控制;2 个独立开关,不强绑 |
| **D5** | **FAS drop-frame panic**:`Arc<AtomicBool>` flag,FAS 检测到掉帧 → set true → hotplug loop 立即扩核 (bypass tick 延迟) | FAS 是掉帧最敏感模块; 必须让它有 panic 通道; `Arc<AtomicBool>` 跨线程零成本 |
| **D6** | **2 个 threshold slider**(WebUI):①`off_threshold_idle_pct` (idle% 超过此值才允许关) ②`on_threshold_idle_pct` (idle% 低于此值才允许开),hysteresis 由这两个值天然形成 | 用户可调,符合 baseline WebUI 4模式风格;比硬编码阈值好 |
| **D7** | **KernelSU Next allowlist**(`/sys/devices/system/cpu/cpu*/online` 和 `/hotplug/target`)给 yumi 进程 | baseline 实测:即使 `chmod 666`,SELinux Enforcing 仍然 deny;必须 KSU allowlist 机制 (KernelSU 模块 manifest 加 `sysfsAllow` 或 KSU manager UI 添加) |
| **D8** | **no ordering**:多个 cpu 跨阈值时,谁先跨谁先动,不强制优先级 | 避免一个 cpu 等另一个的复杂度;hysteresis 自然防抖; FAS panic 优先 |

### D1–D8 衍生的实施架构

```
┌─────────────────────────────────────────────────────────────┐
│ monitor/cpu_monitor.rs (existing)                           │
│   BPF PerCpuArray: idle_pct[8], util_pct[8]                 │
│   ↓ read at 200ms tick                                       │
├─────────────────────────────────────────────────────────────┤
│ scheduler/hotplug/mod.rs (NEW)                              │
│   - HotplugLoop::tick() @ 200ms                              │
│   - reads cpu_monitor.snapshot()                            │
│   - for cpu in [2..7]:                                      │
│       - if idle_pct > off_threshold AND not white-listed:   │
│           - mark candidate_to_disable                       │
│       - if util_pct > on_threshold OR FAS_panic:            │
│           - mark candidate_to_enable                        │
│   - apply mask: write /sys/.../cpuN/online (1/0)            │
│   - D5: FAS_panic = Arc<AtomicBool>, checked every tick     │
├─────────────────────────────────────────────────────────────┤
│ scheduler/fas/mod.rs (modified)                             │
│   - on frame drop: FAS_PANIC.store(true, Relaxed)          │
│   - reset to false when FPS recovers                       │
├─────────────────────────────────────────────────────────────┤
│ WebUI (Vite + Vue, existing)                                │
│   - new section "核心映射":                                   │
│     * 8 cpu grid (online/offline 实时状态)                  │
│     * thermal zone 显示                                      │
│     * 2 toggle (lockscreen-onoff, screens-onoff)            │
│     * 2 slider (off_threshold, on_threshold)                │
└─────────────────────────────────────────────────────────────┘
```

### 关键文件清单 (实施时)

| 文件 | 操作 | 说明 |
|---|---|---|
| `src/scheduler/hotplug/mod.rs` | **新增** | HotplugLoop 主循环,200ms tick |
| `src/scheduler/hotplug/threshold.rs` | **新增** | off/on threshold + hysteresis 判定 |
| `src/scheduler/fas/mod.rs` | **修改** | 加 `Arc<AtomicBool> FAS_PANIC` |
| `src/monitor/cpu_monitor.rs` | **可能扩展** | 加 `idle_pct` per-cpu 导出 (已部分支持, 需确认) |
| `webui/src/components/CoreMap.vue` | **新增** | 8 cpu 网格 + 2 toggle + 2 slider |
| `webui/src/api/hotplug.ts` | **新增** | 前端 → daemon 通信 (走 IPC Channel) |
| `module/module.prop` | **可能加** | `webui=true` (KSU WebUI 自动接管, baseline 已实测可用) |
| `module/webroot/index.html` | **不动** | baseline 已 build, 等新组件打包再重建 |

### 测试 seam (per D3, D5, D6)

1. **RED** `tests/hotplug_threshold.rs`
   - mock cpu_snapshot + FAS_panic flag
   - assert: cpu0/1 永远不出现在 disable 候选
   - assert: idle_pct > off → 进 disable 候选
   - assert: FAS_panic=true → 强制 enable (即使 idle 高)
   - assert: hysteresis — disable 后 200ms 内不复 enable

2. **RED** `tests/hotplug_safety.rs`
   - assert: 任何输入下, mask & 0x3 != 0 (cpu0/1 永不被关)
   - assert: thermal > 70°C → mask = 0xFF (全开)

3. **GREEN**: 实现 + 集成进 scheduler.rs main loop
4. **PHONE TEST**: baseline WebUI 风格,在小米14 Pro 实跑, logcat 监控

### 风险与边界 (D7 专属更新)

- **D7 风险**:KSU allowlist 是 KSU manager UI 操作,**不能脚本化**
  - 解决:在 customize.sh 加 ui_print 提示用户去 KSU manager → Superuser → yumi → 模块设置 → 允许 sysfs
  - 或者:让 yumi 自己提示 (WebUI 加载时检测,红色 banner 提示)
- **D5 风险**:`Arc<AtomicBool>` 在 hotplug tick 里读取,如果 FAS panic 持续 > tick interval,会出现"反复 enable"——加 hysteresis 即可
- **D8 风险**:无 ordering 可能让低优先级 cpu (A510) 长期被关 → 用户可能感知不到性能下降 → **WebUI 必须显示当前 online map**

---

## 当前状态 (更新于 baseline 验证后)

- ✅ ticket-02 baseline build 通 + WebUI 跑通 (KSU WebUIActivity 加载本地 webroot)
- ✅ baseline 实测:8 核在线,CLG 激活 4 cluster,AppDetect 自动切模式
- ✅ baseline 实测:SELinux 拦截 `chmod 666` 的 sysfs,确认 **D7 allowlist 必要性**
- ✅ ticket-03 per-CPU perception spec 写好 (本 ticket 不动它)
- ✅ **D1–D8 决策敲定** (本节)
- ⏭️ 下一步:按 D1–D8 实施架构写 `src/scheduler/hotplug/`,补 RED tests,phone 实测
- ❌ **绝对不能在 host 模拟**, 必须在手机实跑 (D7 allowlist 是 KSU UI 操作)