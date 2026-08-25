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

## 当前状态
- ✅ ticket-02 baseline build 通
- ✅ 8路感知 + 用户区间 (ticket 03) spec 写好
- ❌ ticket 04 等 grill-me
- ❌ **绝对不能在 host 模拟**, 必须在手机实跑