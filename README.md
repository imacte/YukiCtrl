[Read this document in English](README.en.md)

# yumi - 智能 CPU 调度控制器

<div align="center">

[![Android](https://img.shields.io/badge/platform-Android-3DDC84.svg?style=for-the-badge&logo=android)](https://developer.android.com/)
[![Rust](https://img.shields.io/badge/core-Rust-%23dea584.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![WebUI](https://img.shields.io/badge/UI-WebUI-4FC08D.svg?style=for-the-badge&logo=html5)](https://developer.mozilla.org/en-US/docs/Web/HTML)
[![AArch64](https://img.shields.io/badge/arch-AArch64-FF6B6B.svg?style=for-the-badge)](https://en.wikipedia.org/wiki/AArch64)
[![Root Required](https://img.shields.io/badge/Root-Required-FF5722.svg?style=for-the-badge)](https://magiskmanager.com/)

**🚀 智能 CPU 调度系统 - 轻量 WebUI + 高性能 Rust 守护进程 + 内置 FAS 帧感知调度**

</div>

-----

## 📋 项目介绍

**yumi** 是一个功能强大的 Android CPU 调度控制系统，由轻量级的 **WebUI** 管理界面和高性能的 **Rust 守护进程 (yumi)** 组成。通过先进的调度算法和高度可配置的性能模型，它能够根据不同的使用场景动态调整 CPU 频率、核心分配策略以及精细的调速器参数，实现最佳的性能与能效平衡。内置的 **FAS (Frame Aware Scheduling)** 帧感知调度引擎可实时分析游戏帧时间，以逐帧精度动态调频，在保证流畅度的同时最大化省电。

### ✨ 主要特性

  * 🔄 **智能动态模式切换** - 根据当前应用自动调整性能模式。
  * 🎯 **FAS 帧感知调度** - 内置帧时间分析引擎，逐帧动态调频，游戏场景下兼顾流畅与功耗。
  * 🌐 **轻量 WebUI** - 无需安装额外 App，通过浏览器即可管理调度配置。
  * 📱 **应用规则管理** - 为不同应用设置专属的性能策略。
  * ⚡ **应用启动加速** - 监控 cgroup 变化，实现应用启动时的临时性能提升。
  * 🔧 **高度可配置** - YAML 配置文件支持深度自定义，配置热重载无需重启。

## 🔧 系统要求

  * **Android 版本**: Android 8.0 (API 26) 及以上。
  * **架构支持**: ARM64 (AArch64)。
  * **权限要求**: Root 权限。

## 🎯 性能模式

yumi 提供五种性能模式：

| 模式 | 图标 | 特点 | 适用场景 |
| :--- | :--- | :--- | :--- |
| **省电 (Powersave)** | 🔋 | 最大化续航，降低性能释放。 | 待机、轻度使用、阅读。 |
| **均衡 (Balance)** | ⚖️ | 性能与功耗的最佳平衡点。 | 日常使用、社交应用。 |
| **性能 (Performance)** | ⚡ | 优先性能，适度增加功耗。 | 大型应用、轻度游戏。 |
| **极速 (Fast)** | 🚀 | 最大性能释放，忽略功耗。 | 重度游戏、性能测试。 |
| **FAS (帧感知调度)** | 🎯 | 实时分析帧时间，逐帧动态调频，自动档位切换。 | 游戏场景，兼顾流畅与省电。 |

## 🌐 WebUI 管理界面

yumi 内置轻量级 WebUI，通过浏览器即可完成所有管理操作，无需安装额外 App。

  * **模式切换** - 实时切换性能模式。
  * **应用规则管理** - 为不同应用配置专属性能策略。
  * **配置编辑** - 在线编辑 YAML 配置文件。
  * **日志查看** - 实时查看 yumi 守护进程日志。

-----

### 🛠️ 调度核心 (yumi)

yumi 的核心是由一个 Rust 守护进程 **yumi** 驱动的。它负责执行所有底层的系统调优指令，以极低的资源占用实现高效的性能控制。

#### 核心特性

  * **高性能 Rust 实现**: 极低的系统资源占用，运行功耗极低。
  * **实时配置监听**: 支持配置文件（`config.yaml`）和模式文件（`mode.txt`）热重载，切换模式无需重启。
  * **多层次优化策略**: 从 CPU 频率到 I/O 调度的全方位调优。
  * **智能应用启动加速**: 监控 `top-app` cgroup 变化，实现应用启动时的临时性能提升，加快加载速度。
  * **内置 FAS 引擎**: 帧感知调度，无需依赖外部模块即可实现逐帧动态调频。

#### 调度功能

| 功能模块 | 描述 |
| :--- | :--- |
| **CPU 频率控制** | 动态调整各核心簇的最小/最大频率。 |
| **FAS 帧感知调度** | 内置帧时间分析引擎，实时分析游戏帧间隔，通过 perf_index 映射到 CPU 频率，逐帧动态调频。 |
| **调速器管理** | 支持 schedutil、walt 等多种调速器及其内部参数的精细化调整。 |
| **核心分配 (Cpuset)** | 为前台、后台等不同任务组分配合适的 CPU 核心，是功耗和性能管理的关键。 |
| **I/O 调度优化** | 遍历所有块设备，可自定义 I/O 调度器、预读大小、合并策略及 iostats 等参数。 |
| **EAS 调度器调优** | 针对支持 EAS (Energy Aware Scheduling) 的内核进行高级参数优化。 |
| **核心绑定优化** | **(AffinitySetter)** 自动创建 `yumi` 和 `Rubbish` cgroup，将系统关键进程（如 `surfaceflinger`）绑定到 `yumi` 组，并将干扰进程（如 `kswapd0`, `logcat`）隔离到 `Rubbish` 组，显著提升 UI 流畅度。 |
| **冲突管理** | 自动禁用大部分主流的用户态和内核态性能增强（如 FEAS，在非极速模式下），确保调度策略的唯一性。 |

-----

### 🎯 FAS 帧感知调度详解

FAS (Frame Aware Scheduling) 是 yumi 内置的帧感知动态调频引擎，专为游戏场景设计。与传统的静态模式不同，FAS 通过实时分析每一帧的渲染时间来精确控制 CPU 频率，在保证流畅度的同时尽可能降低功耗。

#### 工作原理

FAS 引擎维护一个 **perf_index**（性能指数，范围 0-1000），并根据帧时间的实时反馈来调整它：

  * **帧时间超出预算** → perf_index 上升 → CPU 频率提高
  * **帧时间满足预算** → perf_index 缓慢下降 → CPU 频率降低
  * **perf_index 通过线性插值映射到各核心簇的实际频率档位**

#### 核心机制

  * **自动帧率档位切换**: 支持多档位帧率（如 30/60/90/120/144 fps），根据实际渲染能力自动升降档。降档前会先尝试提频确认是否真的需要降档，避免误降。
  * **加载场景检测**: 自动识别游戏加载画面（持续重帧），进入加载状态后锁定中高频率，加载结束后带保护地恢复正常调度。支持硬加载和软加载（帧率骤降但未达重帧阈值）两种模式。
  * **场景切换感知**: 通过帧时间变异系数检测场景过渡（如地图切换），过渡期间减缓调频幅度，防止频率剧烈波动。
  * **频率迟滞防抖**: 相邻频率档位间设置迟滞带，防止在边界处频繁跳档。
  * **Jank 冷却机制**: 发生严重掉帧后进入冷却期，期间维持较高频率，避免掉帧后立即降频引发连锁卡顿。
  * **外部锁定感知**: 检测频率是否被外部因素（如温控）覆盖，若持续 mismatch 则暂时退避让出控制权，冷却后自动恢复。
  * **小窗模式支持**: FAS 状态支持挂起/恢复，短暂切离（如小窗操作）后可快速恢复调度，无需重新初始化。

#### FAS 配置 (`rules.yaml`)

FAS 的参数通过 `rules.yaml` 中的 `fas_rules` 节进行配置：

```yaml
fas_rules:
  fps_gears: [30.0, 60.0, 90.0, 120.0, 144.0]
  fps_margin: "3.0"
  heavy_frame_threshold_ms: 150.0
  loading_cumulative_ms: 2500.0
  post_loading_ignore_frames: 5
  post_loading_perf_min: 500.0
  post_loading_perf_max: 800.0
  instant_error_threshold_ms: 4.0
  perf_floor: 150.0
  freq_hysteresis: 0.015
```

| 参数 | 类型 | 默认值 | 描述 |
| :--- | :--- | :--- | :--- |
| `fps_gears` | float[] | [20,24,30,45,60,90,120,144] | 支持的帧率档位列表，FAS 会在这些档位间自动切换。 |
| `fps_margin` | string | "3.0" | 帧率余量（fps），EMA 预算 = 1000/(target - margin)，提供一定的容错空间。 |
| `heavy_frame_threshold_ms` | float | 150.0 | 重帧阈值（毫秒），超过此值的帧被视为加载帧。 |
| `loading_cumulative_ms` | float | 2500.0 | 累计重帧时长超过此值后进入加载状态。 |
| `post_loading_ignore_frames` | int | 5 | 加载结束后忽略的帧数，用于过滤过渡噪声。 |
| `post_loading_perf_min` | float | 500.0 | 加载结束后的最低 perf_index。 |
| `post_loading_perf_max` | float | 800.0 | 加载结束后的最高 perf_index。 |
| `instant_error_threshold_ms` | float | 4.0 | 瞬时误差阈值，帧时间超出预算超过此值时触发紧急提频。 |
| `perf_floor` | float | 150.0 | perf_index 下限，防止频率降到过低。 |
| `freq_hysteresis` | float | 0.015 | 频率迟滞系数，防止相邻档位间频繁跳变。 |

-----

### ⚙️ 高级配置 (`config.yaml` 详解)

yumi 使用 YAML 格式的配置文件，允许用户进行深度自定义。

#### 1️⃣ 元信息 (`meta`)

这部分定义了守护进程的基本行为。

```yaml
meta:
  loglevel: "INFO"
  language: "en"
```

| 字段 | 类型 | 描述 |
| :--- | :--- | :--- |
| `loglevel` | string | 日志记录详细程度。可选值：`DEBUG`, `INFO`, `WARN`, `ERROR`。 |
| `language` | string | 守护进程日志的语言。目前支持 `en` (英语) 和 `zh` (中文)。 |

#### 2️⃣ 功能开关 (`function`)

此部分包含了所有主要功能的总开关。

```yaml
function:
  AffinitySetter: true
  CpuIdleScalingGovernor: false
  EasScheduler: true
  cpuset: true
  LoadBalancing: true
  EnableFeas: false
  IOOptimization: true
  AppLaunchBoost: true
```

| 功能 | 描述 |
| :--- | :--- |
| `AffinitySetter` | **(推荐)** **HyperOS 3 勿开**，启用核心绑定优化（`yumi` 和 `Rubbish` cgroup）。 |
| `CpuIdleScalingGovernor`| 是否允许自定义 CPU Idle 调速器（见 `CpuIdle` 部分）。 |
| `EasScheduler` | 如果内核支持 **EAS**，开启可应用优化参数。 |
| `cpuset` | **(推荐)** 启用 Cpuset 功能，为不同任务组分配合适的 CPU 核心（见 `Cpuset` 部分）。 |
| `LoadBalancing` | 启用 CFS 负载均衡优化，让任务在核心间的分配更合理。 |
| `EnableFeas` | 是否在**极速模式 (fast)** 下尝试启用内核的 FEAS 功能。 |
| `IOOptimization` | 启用 I/O 优化，遍历所有块设备应用调度器和参数设置（见 `IO_Settings` 部分）。 |
| `AppLaunchBoost` | **(推荐)** 启用应用启动加速，加快加载速度（见 `AppLaunchBoostSettings` 部分）。 |

#### 3️⃣ 应用启动加速 (`AppLaunchBoostSettings`)

需要 `function.AppLaunchBoost` 为 `true`。

```yaml
AppLaunchBoostSettings:
  BoostRateMs: 600
  SmallCoreBoostFreq: "max"
  MediumCoreBoostFreq: "max"
  BigCoreBoostFreq: "max"
  SuperBigCoreBoostFreq: "max"
```

| 字段 | 类型 | 描述 |
| :--- | :--- | :--- |
| `BoostRateMs` | int | 启动加速的持续时间（毫秒）。 |
| `SmallCoreBoostFreq` | string/int | 小核加速频率，支持 `"min"`、`"max"` 或具体数值。留空表示 `"max"`。 |
| `MediumCoreBoostFreq` | string/int | 中核加速频率，同上。 |
| `BigCoreBoostFreq` | string/int | 大核加速频率，同上。 |
| `SuperBigCoreBoostFreq` | string/int | 超大核加速频率，同上。 |

#### 4️⃣ 核心框架与分配 (`CoreFramework` & `CoreAllocation`)

此部分定义了设备的物理核心架构，是所有频率和核心控制功能的基础，**必须正确配置！**

  * **核心框架 (`CoreFramework`)**: 告诉程序不同核心簇对应的 `policy` 路径 (可在 `/sys/devices/system/cpu/cpufreq/` 目录查看)。设为 `-1` 表示该核心簇不存在。
    ```yaml
    CoreFramework:
      SmallCorePath: 0
      MediumCorePath: 2
      BigCorePath: 5
      SuperBigCorePath: 7
    ```
  * **核心分配 (`CoreAllocation`)**: 为 `AffinitySetter` 功能提供参数，指定将系统关键进程（`yumi` cgroup）绑定到的核心范围。
    ```yaml
    CoreAllocation:
      CpuSetCore: "2-7"
    ```

#### 5️⃣ I/O 设置 (`IO_Settings`)

需要 `function.IOOptimization` 为 `true`。启用后会遍历 `/sys/block/*` 下的所有块设备，逐一应用以下参数（自动判断路径是否存在）。

```yaml
IO_Settings:
  Scheduler: "none"
  read_ahead_kb: "128"
  nomerges: "2"
  iostats: "0"
```

| 字段 | 类型 | 描述 |
| :--- | :--- | :--- |
| `Scheduler` | string | I/O 调度器，如 `"none"`, `"mq-deadline"`, `"bfq"`, `"kyber"`。留空则不修改。 |
| `read_ahead_kb` | string | 预读大小（KB）。 |
| `nomerges` | string | 合并策略。`0`=允许合并，`1`=仅简单合并，`2`=禁止合并。 |
| `iostats` | string | I/O 统计信息。`0`=禁用（推荐，减少开销），`1`=启用。 |

#### 6️⃣ CFS 调度器参数 (`CompletelyFairSchedulerValue`)

```yaml
CompletelyFairSchedulerValue:
  sched_child_runs_first: ""
  sched_rt_period_us: ""
  sched_rt_runtime_us: ""
```

留空的字段不会被写入，保持系统默认值。

#### 7️⃣ CPU Idle (`CpuIdle`)

需要 `function.CpuIdleScalingGovernor` 为 `true`。

```yaml
CpuIdle:
  current_governor: "ladder"
```

  * `current_governor`: 设置 CPU Idle 调速器。

#### 8️⃣ Cpuset (核心分组)

需要 `function.cpuset` 为 `true`。它将不同类型的任务组限制在指定的 CPU 核心上运行。

```yaml
Cpuset:
  top_app: "0-7"
  foreground: "0-7"
  background: "0-3"
  system_background: "0-2"
  restricted: "0-1"
```

| 字段 | 描述 | 建议值 |
| :--- | :--- | :--- |
| `top_app` | 当前在前台运行的应用。 | 应分配所有核心，如 `"0-7"`。 |
| `foreground` | 前台服务和可见的应用。 | 也应分配所有或大部分核心。 |
| `background` | 后台运行的应用和服务。 | **应限制在能效核心**，如 `"0-3"`，以节省功耗。 |
| `system_background` | 系统后台服务。 | 同样应限制在能效核心。 |
| `restricted` | 被系统限制的后台应用。 | 应分配最少的核心。 |

#### 9️⃣ 动态调速器参数 (`pGovPath` & `Govsets`)

此功能允许对 CPU 调速器的内部参数进行精细化调整。配置分为两步：

1.  **定义可用参数 (`pGovPath`)**: 建立一个"参数字典"，按**调速器名称**分组，定义所有可能会用到的参数的【纯文件名】。
    ```yaml
    pGovPath:
      schedutil:
        path1: "up_rate_limit_us"
      walt:
        path1: "target_loads"
    ```
2.  **在模式中设置参数值 (`Govsets`)**: 在**每一个性能模式**内部，同样按**调速器名称**分组，使用 `pGovPath` 中定义的**键**来设置具体**数值**。程序会智能地将设置只应用到正在使用该调速器的核心上。
    ```yaml
    # performance 模式示例
    performance:
      Govsets:
        schedutil:
          path1:
            SmallCore: "0"
            MediumCore: "500"
            BigCore: "0"
            SuperBigCore: "0"
    ```

#### 🔟 功耗模型详解 (以 `performance` 模式为例)

一个完整的性能模式，是由以下**五个模块**共同定义的。您可以自由组合，打造最适合您的模式。

```yaml
performance:
  Governor: { ... } # 调速器：决定CPU频率如何响应负载
  Freq: { ... }     # CPU频率：定义每个核心簇的最小/最大频率
  Uclamp: { ... }   # Uclamp：向调度器提供性能需求的提示 (0-100)
  Govsets: { ... }  # 调速器参数：精细化调整调速器的具体行为
  Other: { ... }    # 其他设置
```

**详细说明：**

  * **`Governor` (调速器)**:
      * `Global`: "schedutil" (全局默认)
      * `SmallCore`: "" (为空则使用全局)
      * ... (其他核心簇)
  * **`Freq` (CPU频率)**:
      * `SmallCoreMinFreq`: 0 (或 "min")
      * `SmallCoreMaxFreq`: 9999999 (或 "max")
      * ... (其他核心簇)
      * **注意**: 频率字段支持 `"min"` 和 `"max"` 字符串，守护进程会将其分别转换为 `0` 和 `9999999`。
  * **`Uclamp` (Uclamp 设置)**:
      * `UclampTopAppMin`: "0"
      * `UclampTopAppMax`: "100"
      * `UclampTopApplatency_sensitive`: "0"
      * `UclampForeGroundMin`: "0"
      * `UclampForeGroundMax`: "70"
      * `UclampBackGroundMin`: "0"
      * `UclampBackGroundMax`: "50"
  * **`Govsets` (调速器参数)**:
      * (结构见上文)
  * **`Other` (其他设置)**:
      * `ufsClkGate`: false (是否禁用 UFS 时钟门控)

## 📥 安装说明

### 前置要求

1.  **获取 Root 权限**

### 安装步骤

1.  **下载模块** - 从 [Releases](https://github.com/imacte/YukiCtrl/releases) 下载最新版本。
2.  **刷入模块** - 通过 Magisk / KernelSU 刷入 yumi 模块。
3.  **访问 WebUI** - 模块启动后，通过浏览器访问 WebUI 进行管理和配置。
4.  **配置规则** - 根据需要为不同应用设置性能策略。

## 🚀 性能优化建议

### 日常使用

1.  **使用均衡模式** - 为大部分应用提供最佳的性能功耗平衡。
2.  **设置应用规则** - 为游戏应用设置性能或极速模式。

### 游戏优化

1.  **使用 FAS 模式** - 帧感知调度可在保证流畅度的同时自动省电，推荐作为游戏的首选模式。
2.  **调整 FAS 参数** - 根据游戏特性调整 `rules.yaml` 中的帧率档位和余量。
3.  **使用性能/极速模式** - 对于 FAS 无法覆盖的场景，可切换到静态高性能模式。
4.  **启用应用启动加速** - 减少游戏加载时间。
5.  **监控温度** - 长时间高性能使用时注意设备温度。

### 省电优化

1.  **使用省电模式** - 在低负载场景下最大化续航。
2.  **限制后台应用** - 通过 `Cpuset` 限制后台应用的 CPU 使用。
3.  **优化 I/O 调度** - 减少存储访问的功耗开销。
4.  **关闭不需要的功能** - 根据需要禁用部分高级功能。

## 🔍 故障排除

### 常见问题

**Q: 模块无法获取 Root 权限？**

  * 确保设备已正确 Root 并安装 Magisk / KernelSU。
  * 检查 Root 管理器中是否允许了 yumi 的 Root 请求。
  * 尝试重新刷入模块或重启设备。

**Q: 智能动态模式不工作？**

  * 验证应用规则是否正确配置。
  * 验证 yumi 模块是否安装并正常运行。

**Q: 性能模式切换无效？**

  * 验证 yumi 模块是否安装并正常运行。
  * 查看 yumi 模块日志以确定具体错误信息。
  * 验证配置文件格式是否正确（`config.yaml` 严格区分大小写）。

**Q: FAS 模式下帧率不稳定？**

  * 检查 `rules.yaml` 中的 `fps_gears` 是否包含目标帧率。
  * 适当增大 `fps_margin` 可提供更多余量，减少边界波动。
  * 查看日志中的 FAS 心跳信息（每 30 帧输出一次），确认调度状态是否正常。
  * 若频率被温控覆盖，日志中会出现 "externally locked" 提示，属正常退避行为。

## 📊 项目统计

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=imacte/YukiCtrl&type=Date)](https://star-history.com/#imacte/YukiCtrl&Date)

</div>

## 📮 联系我们

  * **GitHub Issues** - [项目问题和建议](https://github.com/imacte/YukiCtrl/issues)
  * **QQ 群** - 1036909137
  * **Telegram** - [加入 TG 频道](https://t.me/+gp4adLJAsXYzMjc1)

-----

<div align="center">

<sub>📅 文档更新时间：2026年2月25日</sub><br>
<sub>🚀 yumi - 让每一台 Android 设备都拥有最佳的性能体验</sub>

</div>