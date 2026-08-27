# yumi 调度器个性化改造 · 总规划

> **Status**: ready-for-agent (baseline 已 PASS, ticket-03/04 spec 已写, ticket-05/06/07 待规划)
> **Last updated**: 2026-08-26 (after ticket-02 baseline validation on Xiaomi 14 Pro)
> **Branch**: `ticket-02-baseline-build` → next: `ticket-03-percpu-perception` or `ticket-04-disable-core`
> **Pair with**: `.scratch/yumi-personalize/issues/ticket-02..07-*.md` + `.clinerules/rules.md`

---

## 1. 当前进度快照

| 阶段 | 状态 | 产出 |
|---|---|---|
| Ticket-02 Baseline | ✅ 完成 | 原项目在小米14 Pro 澎湃OS 上编译、装机、运行、WebUI 全部通过 |
| Ticket-03 Per-CPU 感知 | 📝 spec 已写 | `ticket-03-percpu-perception.md`, 等实施 |
| Ticket-04 关核 | 📝 spec 已写, D1-D8 决策敲定 | `ticket-04-disable-core.md`, 等实施 |
| Ticket-05 App 规则 | ❌ 未写 | 本 spec 草案 |
| Ticket-06 WebUI 扩展 | ❌ 未写 | 本 spec 草案 |
| Ticket-07 可靠性 | ❌ 未写 | 本 spec 草案 |

---

## 2. 系统架构 (感知→决策→执行)

```
┌─────────────────────────────────────────────────────────────────┐
│                        WebUI (Vite + Vue)                       │
│  感知面板 │ 8卡片配置(亮/息×2) │ 核心映射 │ App规则 │ 日志      │
└──────────────────────────────┬──────────────────────────────────┘
                               │ IPC Channel
┌──────────────────────────────▼──────────────────────────────────┐
│                      Daemon (Rust)                               │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐ │
│  │ 感知层        │  │ 决策层        │  │ 执行层                 │ │
│  │              │  │              │  │                       │ │
│  │ CPU(eBPF)    │  │ PID控制器    │  │ CPU freq 写入         │ │
│  │ 帧平滑(eBPF) │  │ Boost状态机  │  │ GPU devfreq 写入      │ │
│  │ 触摸(epoll)  │  │ 关核状态机    │  │ IO scheduler 写入     │ │
│  │ GPU(sysfs)   │  │ App规则匹配  │  │ Swap swappiness 写入  │ │
│  │ IO(psi)      │  │ 温度保护     │  │ 核心 online 写入      │ │
│  │ Swap(psi)    │  │ 漂移回写     │  │ 恢复脚本              │ │
│  │ 温度(sysfs)  │  │              │  │                       │ │
│  │ 屏幕状态     │  │              │  │                       │ │
│  │ 前台App      │  │              │  │                       │ │
│  └──────────────┘  └──────────────┘  └───────────────────────┘ │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Watchdog + 失败恢复 + 开机自启                            │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---
## 3. 剩余开发阶段规划 (按优先级排序)

### Phase 1: 感知层完善 (Ticket-03 实施)

**目标**: 把原项目从 per-cluster 升级到 per-CPU, 新增触摸/GPU/IO/Swap 四路感知.

| 任务 | 文件 | 说明 |
|---|---|---|
| 拆分 ClusterState → CpuState | `src/scheduler/cpu_load_governor.rs` | 每个 CPU 独立 PID 调频 |
| Per-CPU sysfs writer | `src/scheduler/cpu_load_governor.rs` | 串行写 scaling_min/max_freq |
| TouchMonitor | `src/monitor/touch_monitor.rs` 新增 | epoll 读 /dev/input/event* |
| GpuMonitor | `src/monitor/gpu_monitor.rs` 新增 | 读 devfreq load/cur_freq |
| IoMonitor | `src/monitor/io_monitor.rs` 新增 | 读 /proc/pressure/io |
| SwapMonitor | `src/monitor/swap_monitor.rs` 新增 | 读 /proc/pressure/memory + zram stat |
| SenseSnapshot 聚合 | `src/monitor/mod.rs` | 统一感知数据结构 |

**验收标准**:
- 日志能打印八路感知数据
- 触摸屏幕时 touch_state 变化
- per-CPU 频率独立调整

### Phase 2: 关核功能 (Ticket-04 实施)

**目标**: 实现 D1-D8 决策, 支持 cpu5/6/7 动态开关. 详见 `issues/ticket-04-disable-core.md` 第 D1-D8 章节.

| 任务 | 文件 | 说明 |
|---|---|---|
| HotplugLoop 主循环 | `src/scheduler/hotplug/mod.rs` 新增 | 200ms tick |
| Threshold 判定 | `src/scheduler/hotplug/threshold.rs` 新增 | off/on threshold + hysteresis |
| FAS panic 通道 | `src/scheduler/fas/mod.rs` 修改 | Arc<AtomicBool> |
| eBPF map 改 HASH | `src/monitor/cpu_monitor.rs` + eBPF 程序 | 支持热插拔 |
| 白名单保护 | hotplug 模块 | cpu0/1 永不被关 |
| 温度保护 | hotplug 模块 | >70°C 强制全开 |
| 核心映射 WebUI | `webui/src/components/CoreMap.vue` 新增 | 8核网格 + 2 toggle + 2 slider |

**验收标准**:
- 息屏后 cpu5/6/7 online=0
- 触摸后核心全开
- cpu0/1 永不被关
- 温度 >70°C 时全开

**风险**: KSU allowlist 是 UI 操作, 不能脚本化, 需要 customize.sh 提示.

### Phase 3: App 规则引擎 (新增 Ticket-05)

**目标**: 让用户能对特定 App 施加调度偏置 (限制或加速).

| 任务 | 文件 | 说明 |
|---|---|---|
| AppRules 配置模型 | `src/config.rs` | 规则列表: 包名 + 类型 + 强度 |
| 规则匹配引擎 | `src/scheduler/app_rule.rs` 新增 | 前台 App 匹配 → 参数偏置 |
| WebUI 规则管理页 | `webui/src/components/AppRules.vue` 新增 | 添加/删除规则, 强度滑块 |

**验收标准**:
- 添加限制规则后, 对应 App 前台时 max_freq 降低
- 添加加速规则后, 对应 App 更容易触发 boost

### Phase 4: WebUI 全面扩展 (新增 Ticket-06)

**目标**: 实现所有配置项的傻瓜化 UI.

| 任务 | 说明 |
|---|---|
| 8 卡片配置页 | CPU/GPU/IO/Swap/触摸/帧平滑/温度/热插拔, 每个卡片亮屏/息屏两个分页 |
| 实时感知面板 | 显示八路感知数据 + 核心在线状态 |
| IO 调度器动态下拉 | 读 /sys/block/*/queue/scheduler 生成选项 |
| 帮助系统 | 每个参数旁的"?"按钮, 显示中文傻瓜说明 |
| App 规则页面 | 规则管理 |
| 配置导入/导出 | YAML 文件导入导出 |

**验收标准**:
- 所有配置项可通过 WebUI 修改并保存
- 实时数据面板更新正常
- 傻瓜说明完整

### Phase 5: 可靠性增强 (新增 Ticket-07)

**目标**: 保证模块稳定运行, 异常时自动恢复.

| 任务 | 文件 | 说明 |
|---|---|---|
| Watchdog 线程 | `src/watchdog.rs` 新增 | 5s 检查心跳/map/核心数 |
| restore_defaults.sh | `scripts/restore_defaults.sh` 扩展 | 恢复所有 sysfs 参数 |
| 开机自启 | module 脚本 | 先恢复再启动 |
| 用户通知 | Android 通知 | 异常时提醒 |

**验收标准**:
- kill 模块进程能被拉起
- 崩溃后参数恢复默认
- 重启手机自动运行

---
## 4. 完整配置项清单 (最终版)

> 所有配置项将在 WebUI 中可配, 每个都有傻瓜说明.

### CPU
| 配置项 | 亮屏默认 | 息屏默认 | 说明 |
|---|---|---|---|
| min_freq_khz | 500000 | 300000 | 最低频率, 调高响应快但耗电 |
| max_freq_khz | 2800000 | 1200000 | 最高频率, 调高性能强但发热 |
| target_util | 70 | 30 | 目标 CPU 利用率 |
| pid_p/i/d | 原默认 | 原默认 | PID 系数, 高级用户 |
| freq_hysteresis | 5 | 5 | 频率迟滞, 防止抖动 |
| up/down_rate_limit | 200 | 100/300 | 升降频速率限制 |

### GPU
| 配置项 | 亮屏默认 | 息屏默认 | 说明 |
|---|---|---|---|
| min_freq | 200 | 100 | GPU 最低频率 |
| max_freq | 800 | 200 | GPU 最高频率 |
| boost_threshold | 85 | 0 | GPU boost 触发负载阈值 |
| idle_threshold | 20 | 0 | 空闲压频阈值 |
| idle_freq | 100 | 100 | 空闲时目标频率 |

### IO
| 配置项 | 亮屏默认 | 息屏默认 | 说明 |
|---|---|---|---|
| scheduler | mq-deadline | none | 调度器, 动态检测可选值 |
| read_ahead_kb | 512 | 128 | 预读大小 |
| nomerges | 0 | 0 | 禁止合并 IO 请求 |
| iostats | 0 | 0 | IO 统计开关 |
| io_psi_threshold | 80 | 90 | IO 压力触发阈值 |

### Swap
| 配置项 | 亮屏默认 | 息屏默认 | 说明 |
|---|---|---|---|
| swappiness | 100 | 0 | 换页倾向 |
| zram_pressure_threshold | 80 | 0 | 内存压力触发阈值 |

### 触摸
| 配置项 | 亮屏默认 | 息屏默认 | 说明 |
|---|---|---|---|
| boost_enabled | true | false | 是否启用触摸 boost |
| boost_duration_ms | 200 | 0 | boost 持续时间 |
| boost_strength | 15 | 0 | boost 强度 (%) |
| boost_decay | 30 | 0 | 衰减速度 |

### 帧平滑
| 配置项 | 亮屏默认 | 息屏默认 | 说明 |
|---|---|---|---|
| frame_drop_threshold | 20 | 100 | 掉帧判定阈值 (%) |
| frame_drop_boost_enabled | true | false | 掉帧时是否提频 |
| frame_drop_boost_strength | 10 | 0 | 掉帧 boost 强度 (%) |
| frame_drop_boost_duration | 500 | 0 | 掉帧 boost 持续时间 (ms) |

### 温度
| 配置项 | 默认值 | 说明 |
|---|---|---|
| core_temp_threshold | 85 | 核心温度硬阈值, 超过强制降频 |
| core_temp_soft_threshold | 75 | 软阈值, 开始逐渐降频 |
| gpu_temp_threshold | 85 | GPU 温度阈值 |
| temp_hysteresis | 5 | 温度迟滞 |

### 突发突破
| 配置项 | 亮屏默认 | 息屏默认 | 说明 |
|---|---|---|---|
| burst_boost_enabled | true | false | 是否允许突破上限 |
| burst_trigger_cpu_util | 90 | 0 | CPU 利用率触发阈值 |
| burst_trigger_gpu_load | 85 | 0 | GPU 负载触发阈值 |
| burst_trigger_touch | true | false | 是否要求触摸 |
| burst_trigger_frame_drop | true | false | 是否要求掉帧 |
| burst_max_override_pct | 15 | 0 | 突破幅度 (%) |
| burst_duration_ms | 2000 | 0 | 突破持续时间 |
| burst_cooldown_ms | 5000 | 0 | 冷却时间 |
| fallback_strategy | linear | linear | 回落策略 |
| fallback_duration_ms | 30000 | 0 | 回落到正常区间时间 |

### 核心热插拔 (per D1-D8)
| 配置项 | 亮屏默认 | 息屏默认 | 说明 |
|---|---|---|---|
| hotplug_enabled | true | true | 是否允许自动关核 |
| hotplug_big_cores | [5,6,7] | [5,6,7] | 可关的核心 ID (D2 保护 cpu0/1) |
| hotplug_order | [7,6,5] | [7,6,5] | 关核顺序 (D8 无 ordering, 此为建议默认) |
| idle_enter_threshold | 20 | 10 | 进入关核的负载阈值 (%) — D6 slider A |
| idle_duration_ms | 10000 | 5000 | 持续低负载多久关核 (D3 hysteresis) |
| exit_threshold | 60 | 50 | 退出关核的负载阈值 (%) — D6 slider B |
| lockscreen_onoff_toggle | true | n/a | D4 toggle ① |
| screens_onoff_toggle | true | n/a | D4 toggle ② |

### App 规则
| 配置项 | 说明 |
|---|---|
| enabled | 是否启用 App 规则 |
| rules[] | 规则列表 |

规则结构:
```yaml
- package: "com.example.app"
  type: "restrict"        # restrict / boost
  strength: "medium"      # light / medium / heavy
  max_freq_scale: 0.8     # 自定义缩放系数
  target_util_offset: -20 # 目标利用率偏移
  disable_burst: true     # 是否禁用 burst
  boost_threshold_offset: 15  # boost 触发阈值偏移
```

---
## 5. 决策逻辑完整流程图

```
感知数据输入 (八路)
    ↓
屏幕状态判断 (亮屏/息屏)
    ↓
加载对应配置集
    ↓
App 规则匹配 (前台包名)
    ├── 匹配 restrict → 应用负向偏置
    ├── 匹配 boost → 应用正向偏置
    └── 不匹配 → 无偏置
    ↓
PID 基础调频 (在用户 min/max 区间内)
    ↓
检查突发条件
    ├── 触摸 + 高 CPU/GPU + 掉帧 → BURST 状态 (临时突破上限)
    ├── BURST 中且负载回落 → FALLBACK (线性回落到正常区间)
    └── 正常 → NORMAL
    ↓
关核判定 (D1-D8)
    ├── 息屏 → 按息屏策略关核
    ├── 亮屏低负载 + 无掉帧 → 考虑关核
    ├── 触摸/高负载/掉帧 → 立即开核
    └── FAS panic (D5 Arc<AtomicBool>) → 立即开核 (最高优先级)
    ↓
温度保护检查 (最高优先级)
    ├── > soft_threshold → 降频
    └── > hard_threshold → 强制最低频 + 全核开
    ↓
漂移检测 + 回写
    ↓
写 sysfs (频率 / IO / Swap / 核心 online)
```

---

## 6. 约束清单

- 不能修改 Android 内核, 只能通过 sysfs/procfs 读写.
- 必须尊重系统温控, 温度超过阈值禁止 boost.
- 不得破坏 eBPF map 稳定性, 真关核前需改造为 HASH 结构.
- 所有新增配置有默认值, WebUI 提供易懂说明.
- 维护成本要低, 模块化设计, 配置与代码分离.

---

## 7. 风险登记册

| 风险 | 等级 | 缓解 |
|---|---|---|
| KSU sysfs allowlist 必须 UI 操作 | 高 | customize.sh + WebUI banner 提示用户去 KSU manager 添加 |
| 关核后 eBPF PerCpuArray 映射丢失 | 高 | Phase 2 之前先重构为 HASH (CPU ID → 状态) |
| 温度误报导致频繁 boost 抑制 | 中 | temp_hysteresis=5 + 软阈值渐进降频 |
| App 规则匹配拼写错误导致误匹配 | 中 | 包名全量存储, WebUI 输入时不报错但反黄提示 |
| Phase 3-7 推翻 Phase 1-2 设计 | 低 | 所有数据走 SenseSnapshot, 决策层不变 |
| WebUI 频道断裂 (IPC Channel 溢出) | 中 | per-update diff + 限流 5 Hz |
| 突发突破 cooldown 错乱 | 中 | FAS panic Arc<AtomicBool> 在 BURST/FALLBACK 独立 tick |
| 开机自启与 KSU 模块加载顺序 | 低 | customize.sh service.sh 遵循 post-fs-data + late_start |

---

## 8. 明天醒来第一步

**二选一** (不冲突, 可同时启动两个分支):

1. **Phase 1** (Ticket-03 实施): 写 per-CPU 感知代码 + 四个新采集器
2. **Phase 2** RED tests (按 ticket-04 D1-D8): 测试驱动关核逻辑

**推荐顺序**: 先 Phase 1 (基础感知) → Phase 2 (关核复用感知) → Phase 3 → 4 → 5.

**给模型的总任务书模板** (可复制):

```
我正在改造 GitHub 项目 imacte/yumi (Rust 编写的 Android 调度器, 使用 eBPF 感知 CPU 负载和帧平滑, 通过 PID 控制器调整 CPU 频率, 支持 WebUI). 当前版本 v2.0.2, 已在小米14 Pro 澎湃OS 上完成 baseline 编译验证.

改造目标:
1. 感知层扩展: 保留原项目 eBPF CPU 利用率、帧平滑、温度、屏幕状态、前台应用检测, 新增触摸事件、GPU 负载、IO 压力、Swap 压力, 共八路感知.
2. 决策层升级: 不按 App 切换固定模式, 而是根据八路感知在用户配置区间内自动寻优. 突发负载时允许临时突破上限, 然后按策略回落.
3. 配置模型扩展: 所有参数支持亮屏/息屏两套独立配置. 支持动态热插拔大核 (cpu5/6/7). 支持按 App 包名设置调度偏置.
4. 可靠性: watchdog 自检、失败恢复、开机自启、异常通知.
5. WebUI: 所有配置项可网页配置, 傻瓜化中文说明, 实时感知数据展示.

约束:
- 不能修改 Android 内核, 只能通过 sysfs/procfs 读写.
- 必须尊重系统温控, 温度超过阈值禁止 boost.
- 不得破坏 eBPF map 稳定性, 真关核前需改造为 HASH 结构.
- 所有新增配置有默认值, WebUI 提供易懂说明.
- 维护成本要低, 模块化设计, 配置与代码分离.

开发环境: Windows + Rust nightly + Android NDK + bpf-linker, 目标设备小米14 Pro 澎湃OS.
```

详细 spec 已在 `.scratch/yumi-personalize/spec.md` (本文件), 按 Phase 1 → 2 → 3 → 4 → 5 逐个投喂.

---

## 9. 当前状态 (持续更新)

- ✅ ticket-02 baseline build 通 + WebUI 跑通 (KSU WebUIActivity 加载本地 webroot)
- ✅ ticket-03 per-CPU perception spec 写好
- ✅ ticket-04 disable-core spec + D1-D8 决策敲定
- ✅ 本总规划 spec.md 写好 (Section 1-9)
- ⏭️ 下一步: 投喂 Phase 1 / Phase 2 RED tests 任务书, 开始实施
- ❌ **绝对不能在 host 模拟**, 必须在手机实跑 (KSU allowlist 是 UI 操作, 突发突破逻辑需实测)
