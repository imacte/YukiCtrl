[Read this document in English](README.en.md)

# YukiCtrl - 智能 CPU 调度控制器

<div align="center">

[![Android](https://img.shields.io/badge/platform-Android-3DDC84.svg?style=for-the-badge&logo=android)](https://developer.android.com/)
[![Kotlin](https://img.shields.io/badge/language-Kotlin-7F52FF.svg?style=for-the-badge&logo=kotlin)](https://kotlinlang.org/)
[![Rust](https://img.shields.io/badge/core-Rust-%23dea584.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![AArch64](https://img.shields.io/badge/arch-AArch64-FF6B6B.svg?style=for-the-badge)](https://en.wikipedia.org/wiki/AArch64)
[![Root Required](https://img.shields.io/badge/Root-Required-FF5722.svg?style=for-the-badge)](https://magiskmanager.com/)

**🚀 智能 CPU 调度系统 - 现代化 Android 应用 + 高性能 C++ 守护进程**

</div>

-----

## 📋 项目介绍

**YukiCtrl** 是一个功能强大的 Android CPU 调度控制应用，由现代化的 **Kotlin + Jetpack Compose** 用户界面和高性能的 **C++ 守护进程 (YukiCpuScheduler)** 组成。通过先进的调度算法和高度可配置的性能模型，它能够根据不同的使用场景动态调整 CPU 频率、总线速度、核心分配策略以及精细的调速器参数，实现最佳的性能与能效平衡。

### ✨ 主要特性

  * 🔄 **智能动态模式切换** - 根据当前应用自动调整性能模式。
  * ⚡ **实时性能监控** - 通过通知栏实时显示当前模式和应用信息。
  * 🎮 **悬浮窗快速控制** - 无需返回应用即可快速调整性能模式。
  * 📱 **应用规则管理** - 为不同应用设置专属的性能策略。
  * 🎯 **多种调度核心支持** - 支持 YukiCpuScheduler 和通用脚本模式。
  * 🌈 **丰富的主题系统** - 多种配色方案和自定义背景支持。
  * 🔧 **高级配置编辑** - 内置 YAML 配置编辑器和日志查看器。

## 🔧 系统要求

  * **Android 版本**: Android 8.0 (API 26) 及以上。
  * **架构支持**: ARM64 (AArch64)。
  * **权限要求**: Root 权限。

## 🎯 性能模式

YukiCtrl 提供四种性能模式：

| 模式 | 图标 | 特点 | 适用场景 |
| :--- | :--- | :--- | :--- |
| **省电 (Powersave)** | 🔋 | 最大化续航，降低性能释放。 | 待机、轻度使用、阅读。 |
| **均衡 (Balance)** | ⚖️ | 性能与功耗的最佳平衡点。 | 日常使用、社交应用。 |
| **性能 (Performance)** | ⚡ | 优先性能，适度增加功耗。 | 大型应用、轻度游戏。 |
| **极速 (Fast)** | 🚀 | 最大性能释放，忽略功耗。 | 重度游戏、性能测试。 |
| **fas (Fas)** |  | 兼容fas模块。 |  |

## 📱 应用功能详解

### 🔄 智能动态模式

  * **无障碍服务集成** - 通过无障碍服务检测应用切换。
  * **应用规则管理** - 为不同应用设置专属性能策略。
  * **实时模式切换** - 根据当前运行的应用自动调整性能模式。
  * **全局默认模式** - 为未设置规则的应用提供默认性能模式。

### 🎮 悬浮窗控制

  * **快速模式切换** - 无需返回应用即可调整性能。
  * **实时信息显示** - 显示当前应用和性能模式。
  * **拖拽自由定位** - 悬浮窗位置可自由调整。
  * **主题跟随** - 悬浮窗外观跟随应用主题。

### 📊 系统监控

  * **持续前台通知** - 通知栏实时显示当前状态。
  * **应用信息展示** - 显示当前运行应用的名称。
  * **模式状态指示** - 清晰显示当前激活的性能模式。
  * **点击交互** - 点击通知可快速打开悬浮窗。

### 🔧 高级功能

  * **详细配置编辑** - 内置 YAML 配置文件编辑器。
  * **实时日志查看** - 查看 YukiCpuScheduler 守护进程日志。
  * **脚本管理** - 独立的系统优化脚本开关控制。
  * **自动核心检测** - 智能识别设备的 CPU 核心架构。

-----

### 🛠️ 调度核心 (YukiCpuScheduler)

YukiCtrl 的核心是由一个rust守护进程 **YukiCpuScheduler** 驱动的。它负责执行所有底层的系统调优指令，以极低的资源占用实现高效的性能控制。

#### 核心特性

  * **高性能rust实现**: 极低的系统资源占用，运行功耗极低。
  * **实时配置监听**: 支持配置文件热重载，切换模式无需重启。
  * **多层次优化策略**: 从 CPU 频率到总线速度的全方位调优。
  * **智能应用启动加速**: 应用启动时的临时性能提升，加快加载速度。

#### 调度功能

| 功能模块 | 描述 |
| :--- | :--- |
| **CPU 频率控制** | 动态调整各核心簇的最小/最大频率。 |
| **调速器管理** | 支持 schedutil、walt 等多种调速器及其内部参数的精细化调整。 |
| **核心分配 (Cpuset)** | 为前台、后台等不同任务组分配合适的 CPU 核心，是功耗和性能管理的关键。 |
| **总线频率优化** | 精细控制SoC内部数据总线（LLCC缓存/DDR内存）的频率，对系统响应速度和功耗有显著影响。 |
| **I/O 调度优化** | 优化存储设备的访问策略，可自定义I/O调度器。 |
| **EAS 调度器调优** | 针对支持 EAS (Energy Aware Scheduling) 的内核进行高级参数优化。 |
| **核心绑定优化** | 通过 `AffinitySetter` 对系统关键进程进行静态核心绑定，显著提升UI流畅度。 |
| **冲突管理** | 自动禁用大部分主流的用户态和内核态性能增强（如 touch boost），确保调度策略的唯一性。 |

-----

### ⚙️ 高级配置 (`config.yaml` 详解)

YukiCtrl 使用 YAML 格式的配置文件，允许用户进行深度自定义。

#### 1️⃣ 元信息 (`meta`)

这部分定义了配置文件的基本信息。

```yaml
meta:
  name: "YukiCpuScheduler Profile"
  author: "yuki"
  loglevel: "INFO"
```

| 字段 | 类型 | 描述 |
| :--- | :--- | :--- |
| `configVersion` | number | **至关重要**。此版本号必须与程序要求的版本完全一致。 |
| `loglevel` | string | 日志记录详细程度。可选值：`DEBUG`, `INFO`, `WARNING`, `ERROR`。 |

#### 2️⃣ 功能开关 (`function`)

此部分包含了所有主要功能的总开关。

```yaml
function:
  DisableQcomGpu: true
  AffinitySetter: true
  CpuIdleScaling_Governor: false
  EasScheduler: true
  cpuset: true
  LoadBalancing: true
  EnableFeas: false
  AdjIOScheduler: true
  AppLaunchBoost: true
```

| 功能 | 描述 |
| :--- | :--- |
| `AffinitySetter` | **(推荐)** **hyperos3勿开**，对系统关键进程进行静态核心绑定，**显著提升UI流畅度**。 |
| `CpuIdleScaling_Governor`| 是否允许自定义 CPU Idle 调速器。 |
| `EasScheduler` | 如果内核支持 **EAS**，开启可应用优化参数。 |
| `cpuset` | **(推荐)** 启用 Cpuset 功能，为不同任务组分配合适的 CPU 核心。 |
| `LoadBalancing` | 启用 CFS 负载均衡优化，让任务在核心间的分配更合理。 |
| `EnableFeas` | 是否在**极速模式**下尝试启用内核的 FEAS 功能。 |
| `AdjIOScheduler` | 是否允许自定义 I/O 调速器。 |
| `AppLaunchBoost` | **(推荐)** 启用应用启动加速，加快加载速度。 |

#### 3️⃣ 核心框架与分配 (`CoreFramework` & `CoreAllocation`)

此部分定义了设备的物理核心架构，是所有频率和核心控制功能的基础，**必须正确配置！**

  * **核心框架 (`CoreFramework`)**: 告诉程序不同核心簇对应的 `policy` 路径 (可在 `/sys/devices/system/cpu/cpufreq/` 目录查看)。
    ```yaml
    CoreFramework:
      SmallCorePath: 0
      MediumCorePath: 2
      BigCorePath: 5
      SuperBigCorePath: 7
    ```
  * **核心分配 (`CoreAllocation`)**: 为 `AffinitySetter` 功能提供参数，指定将系统关键进程绑定到的核心范围。
    ```yaml
    CoreAllocation:
      cpusetCore: "2-7"
    ```

#### 4️⃣ 总线频率控制 (`Bus_dcvs_Path` & `Bus_dcvs`)

此功能允许精细控制SoC内部数据总线（LLCC缓存/DDR内存）的频率。配置分为两步：

1.  **全局路径定义 (`Bus_dcvs_Path`)**: **一次性**告诉程序控制总线频率的系统文件位于何处。程序会智能判断并只对已填写的路径进行操作。
    ```yaml
    # 高通平台示例
    Bus_dcvs_Path:
      CPUllccminPath: "/sys/devices/system/cpu/bus_dcvs/L3/soc:qcom,memlat:l3:prime/min_freq"
      CPUllccmaxPath: "/sys/devices/system/cpu/bus_dcvs/L3/soc:qcom,memlat:l3:prime/max_freq"
      CPUddrminPath: "/sys/devices/system/cpu/bus_dcvs/DDR/soc:qcom,memlat:ddr:prime/min_freq"
      CPUddrmaxPath: "/sys/devices/system/cpu/bus_dcvs/DDR/soc:qcom,memlat:ddr:prime/max_freq"
    ```
2.  **模式内数值设定 (`Bus_dcvs`)**: 在**每一个性能模式**内部，设定希望写入的具体频率数值。
    ```yaml
    # performance 模式示例
    performance:
      Bus_dcvs:
        CPUllccmin: 1555000
        CPUddrmax: 3196000
    ```

#### 5️⃣ 动态调速器参数 (`pGovPath` & `Govsets`)

此功能允许对 CPU 调速器的内部参数进行精细化调整。配置也分为两步：

1.  **定义可用参数 (`pGovPath`)**: 建立一个“参数字典”，按**调速器名称**分组，定义所有可能会用到的参数的【纯文件名】。
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
          path1: "0"      # 对应 up_rate_limit_us
        walt:
          path1: "95"     # 对应 target_loads
    ```

#### 6️⃣ Cpuset (核心分组)

需要 `function.cpuset` 为 `true`。它将不同类型的任务组限制在指定的 CPU 核心上运行。

| 字段 | 描述 | 建议值 |
| :--- | :--- | :--- |
| `top_app` | 当前在前台运行的应用。 | 应分配所有核心，如 `"0-7"`。 |
| `foreground` | 前台服务和可见的应用。 | 也应分配所有或大部分核心。 |
| `background` | 后台运行的应用和服务。 | **应限制在能效核心**，如 `"0-3"`，以节省功耗。 |
| `system_background` | 系统后台服务。 | 同样应限制在能效核心。 |
| `restricted` | 被系统限制的后台应用。 | 应分配最少的核心。 |

#### 7️⃣ 功耗模型详解 (以 `performance` 模式为例)

一个完整的性能模式，是由以下**六个模块**共同定义的。您可以自由组合，打造最适合您的模式。

```yaml
performance:
  Governor: { ... } # 调速器：决定CPU频率如何响应负载
  Freq: { ... }     # CPU频率：定义每个核心簇的最小/最大频率
  Uclamp: { ... }   # Uclamp：向调度器提供性能需求的提示 (0-100)
  Bus_dcvs: { ... } # 总线频率：设置SoC内部数据总线的频率
  Govsets: { ... }  # 调速器参数：精细化调整调速器的具体行为
  Other: { ... }    # 其他设置
```

## 📥 安装说明

### 前置要求

1.  **获取 Root 权限**

### 安装步骤

1.  **下载应用** - 从 [Releases](https://github.com/imacte/YukiCtrl/releases) 下载最新版本的 APK。
2.  **安装应用** - 允许来自未知来源的应用安装。
3.  **首次运行** - 应用会自动请求 Root 权限并初始化系统。
4.  **权限配置** - 根据应用提示完成无障碍服务等权限的配置。

### 配置建议

1.  **开启无障碍服务** - 启用智能动态模式功能。
2.  **授予悬浮窗权限** - 启用悬浮窗快速控制功能。
3.  **设置应用规则** - 为常用应用配置专属的性能策略。
4.  **调整通知设置** - 确保状态通知始终显示。

## 🚀 性能优化建议

### 日常使用

1.  **使用均衡模式** - 为大部分应用提供最佳的性能功耗平衡。
2.  **设置应用规则** - 为游戏应用设置性能或极速模式。
3.  **启用智能切换** - 让系统根据应用自动调整性能。
4.  **合理使用悬浮窗** - 在需要时快速调整性能模式。

### 游戏优化

1.  **使用性能/极速模式** - 为游戏提供最大性能释放。
2.  **启用应用启动加速** - 减少游戏加载时间。
3.  **调整核心分配** - 确保游戏进程获得足够的 CPU 资源。
4.  **监控温度** - 长时间高性能使用时注意设备温度。

### 省电优化

1.  **使用省电模式** - 在低负载场景下最大化续航。
2.  **限制后台应用** - 通过 Cpuset 限制后台应用的 CPU 使用。
3.  **优化 I/O 调度** - 减少存储访问的功耗开销。
4.  **关闭不需要的功能** - 根据需要禁用部分高级功能。

## 🔍 故障排除

### 常见问题

**Q: 应用无法获取 Root 权限？**

  * 确保设备已正确 Root 并安装 Magisk。
  * 检查 Magisk 设置中是否允许了 YukiCtrl 的 Root 请求。
  * 尝试重新安装应用或重启设备。

**Q: 智能动态模式不工作？**

  * 确保已开启无障碍服务权限。
  * 检查应用是否在省电白名单中。
  * 验证应用规则是否正确配置。

**Q: 悬浮窗无法显示？**

  * 检查是否已授予悬浮窗权限。
  * 确保无障碍服务正常运行。
  * 尝试手动开启悬浮窗功能。

**Q: 性能模式切换无效？**

  * 检查 YukiCpuScheduler 守护进程是否正常运行。
  * 查看应用日志以确定具体错误信息。
  * 验证配置文件格式是否正确。

## 📊 项目统计

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=imacte/YukiCtrl&type=Date)](https://star-history.com/#imacte/YukiCtrl&Date)

</div>

## 📮 联系我们

  * **GitHub Issues** - [项目问题和建议](https://github.com/imacte/YukiCtrl/issues)
  * **QQ 群 1** - 1036909137
  * **QQ 群 2** - 1055174076

-----

<div align="center">

<sub>📅 文档更新时间：2025年10月11日</sub><br>
<sub>🚀 YukiCtrl - 让每一台 Android 设备都拥有最佳的性能体验</sub>

</div>