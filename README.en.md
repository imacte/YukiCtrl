[阅读中文文档](README.md)

# YukiCtrl - Intelligent CPU Scheduling Controller

<div align="center">

[![Android](https://img.shields.io/badge/platform-Android-3DDC84.svg?style=for-the-badge&logo=android)](https://developer.android.com/)
[![Kotlin](https://img.shields.io/badge/language-Kotlin-7F52FF.svg?style=for-the-badge&logo=kotlin)](https://kotlinlang.org/)
[![Rust](https://img.shields.io/badge/core-Rust-%23dea584.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![AArch64](https://img.shields.io/badge/arch-AArch64-FF6B6B.svg?style=for-the-badge)](https://en.wikipedia.org/wiki/AArch64)
[![Root Required](https://img.shields.io/badge/Root-Required-FF5722.svg?style=for-the-badge)](https://magiskmanager.com/)

**🚀 An Intelligent CPU Scheduling System - Modern Android App + High-Performance rust Daemon**

</div>

-----

## 📋 About The Project

**YukiCtrl** is a powerful Android CPU scheduling control application, consisting of a modern **Kotlin + Jetpack Compose** user interface and a high-performance **rust daemon (YukiCpuScheduler)**. Through advanced scheduling algorithms and highly configurable performance models, it dynamically adjusts CPU frequency, bus speed, core allocation strategies, and fine-tunes governor parameters for different usage scenarios to achieve the optimal balance between performance and power efficiency.

### ✨ Key Features

  * 🔄 **Smart Dynamic Mode Switching** - Automatically adjusts performance modes based on the current application.
  * ⚡ **Real-time Performance Monitoring** - Displays the current mode and app information in the notification bar.
  * 🎮 **Floating Window for Quick Control** - Quickly adjust performance modes without returning to the app.
  * 📱 **App Rule Management** - Set dedicated performance strategies for different applications.
  * 🎯 **Multiple Scheduling Core Support** - Supports both YukiCpuScheduler and general-purpose script modes.
  * 🌈 **Rich Theming System** - Multiple color schemes and custom background support.
  * 🔧 **Advanced Configuration Editing** - Built-in YAML configuration editor and log viewer.

## 🔧 System Requirements

  * **Android Version**: Android 8.0 (API 26) and above.
  * **Architecture Support**: ARM64 (AArch64).
  * **Permissions Required**: Root access.

## 🎯 Performance Modes

YukiCtrl offers four main performance modes:

| Mode | Icon | Characteristics | Use Case |
| :--- | :--- | :--- | :--- |
| **Powersave** | 🔋 | Maximizes battery life, reduces performance output. | Standby, light use, reading. |
| **Balance** | ⚖️ | The optimal balance between performance and power consumption. | Daily use, social apps. |
| **Performance** | ⚡ | Prioritizes performance with a moderate increase in power consumption. | Large applications, light gaming. |
| **Fast** | 🚀 | Unleashes maximum performance, ignoring power consumption. | Heavy gaming, performance testing. |
| **Fas** | | **Compatibility Mode**. Releases CPU frequency control (only modifies node permissions), for compatibility with external modules like FAS. | Use with other scheduling modules. |

## 📱 Application Functions Explained

### 🔄 Smart Dynamic Mode

  * **Accessibility Service Integration** - Detects app switches via the Accessibility Service.
  * **App Rule Management** - Set dedicated performance strategies for different apps.
  * **Real-time Mode Switching** - Automatically adjusts performance mode based on the currently running app.
  * **Global Default Mode** - Provides a default performance mode for apps without specific rules.

### 🎮 Floating Window Control

  * **Quick Mode Switching** - Adjust performance without returning to the app.
  * **Real-time Info Display** - Shows the current app and performance mode.
  * **Drag to Position** - The floating window's position can be freely adjusted.
  * **Theme Sync** - The floating window's appearance follows the app's theme.

### 📊 System Monitoring

  * **Persistent Foreground Notification** - The notification bar displays the current status in real-time.
  * **App Info Display** - Shows the name of the currently running application.
  * **Mode Status Indicator** - Clearly indicates the currently active performance mode.
  * **Click Interaction** - Tap the notification to quickly open the floating window.

### 🔧 Advanced Features

  * **Detailed Configuration Editing** - Built-in YAML configuration file editor.
  * **Real-time Log Viewer** - View logs from the YukiCpuScheduler daemon.
  * **Script Management** - Independent control switches for system optimization scripts.
  * **Automatic Core Detection** - Intelligently identifies the device's CPU core architecture.

-----

### 🛠️ Scheduling Core (YukiCpuScheduler)

The core of YukiCtrl is driven by a rust daemon, **YukiCpuScheduler**. It is responsible for executing all low-level system tuning commands, achieving efficient performance control with extremely low resource consumption.

#### Core Features

  * **High-Performance rust Implementation**: Extremely low system resource usage and minimal power consumption.
  * **Real-time Configuration Monitoring**: Supports hot-reloading for configuration (`config.yaml`) and mode (`mode.txt`) files, allowing mode switches without a reboot.
  * **Multi-level Optimization Strategy**: Comprehensive tuning from CPU frequency to bus speed.
  * **Smart App Launch Boost**: Monitors `top-app` cgroup changes to provide a temporary performance boost during app launch, speeding up loading times.

#### Scheduling Functions

| Feature Module | Description |
| :--- | :--- |
| **CPU Frequency Control** | Dynamically adjusts the min/max frequency for each core cluster. |
| **Governor Management** | Supports fine-grained tuning of various governors like schedutil, walt, and their internal parameters. |
| **Core Allocation (Cpuset)** | Assigns appropriate CPU cores to different task groups (foreground, background, etc.), key for managing power and performance. |
| **Bus Frequency Optimization** | Finely controls the frequency of the SoC's internal data bus (LLCC cache/DDR memory), significantly impacting system responsiveness and power consumption. |
| **I/O Scheduler Optimization** | Optimizes storage device access policies, allows for custom I/O schedulers, and can disable iostats. |
| **EAS Scheduler Tuning** | Advanced parameter optimization for kernels that support Energy Aware Scheduling (EAS). |
| **Core Binding Optimization (AffinitySetter)** | Automatically creates `yuki` and `Rubbish` cgroups. Binds critical system processes (e.g., `systemui`, `surfaceflinger`) to the `yuki` group and isolates interfering processes (e.g., `kswapd0`, `logcat`) to the `Rubbish` group, significantly improving UI smoothness. |
| **Conflict Management** | Automatically disables most common userspace and kernel-level performance boosters (like FEAS, except in Fast mode) to ensure the scheduler's policy is the single source of truth. |

-----

### ⚙️ Advanced Configuration (`config.yaml` Explained)

YukiCtrl uses a YAML-formatted configuration file, allowing for deep customization.

#### 1️⃣ Metadata (`meta`)

This section defines the basic behavior of the daemon.

```yaml
meta:
  loglevel: "INFO"
  language: "en"
```

| Field | Type | Description |
| :--- | :--- | :--- |
| `loglevel` | string | Log level detail. Options: `DEBUG`, `INFO`, `WARN`, `ERROR`. |
| `language` | string | Daemon log language. Currently supports `en` (English) and `zh` (Chinese). |

#### 2️⃣ Function Toggles (`function`)

This section contains the master switches for all major features.

```yaml
function:
  AffinitySetter: true
  CpuIdleScaling_Governor: false
  EasScheduler: true
  cpuset: true
  LoadBalancing: true
  EnableFeas: false
  AdjIOScheduler: true
  AppLaunchBoost: true
```

| Function | Description |
| :--- | :--- |
| `AffinitySetter` | **(Recommended)** **Do not enable on HyperOS 3**. Enables core binding optimization (`yuki` and `Rubbish` cgroups). |
| `CpuIdleScaling_Governor`| Whether to allow custom CPU Idle governors (see `CpuIdle` section). |
| `EasScheduler` | If the kernel supports **EAS**, enabling this will apply optimized parameters (see `EasSchedulerValue` section). |
| `cpuset` | **(Recommended)** Enables the Cpuset feature to assign different task groups to appropriate CPU cores (see `Cpuset` section). |
| `LoadBalancing` | Enables CFS load balancing optimizations for more rational task distribution across cores. |
| `EnableFeas` | Whether to attempt enabling the kernel's FEAS feature in **Fast mode**. |
| `AdjIOScheduler` | Whether to allow custom I/O schedulers (see `IO_Settings` section). |
| `AppLaunchBoost` | **(Recommended)** Enables app launch acceleration to speed up loading times (see `AppLaunchBoostSettings` section). |

#### 3️⃣ App Launch Boost (`AppLaunchBoostSettings`)

Requires `function.AppLaunchBoost` to be `true`.

```yaml
AppLaunchBoostSettings:
  FreqMulti: 1.2
  BoostRateMs: 200
```

| Field | Type | Description |
| :--- | :--- | :--- |
| `FreqMulti` | float | On launch, the CPU max frequency is multiplied by this factor based on the **current mode**. `1.2` means a 20% boost. |
| `BoostRateMs`| int | Duration of the launch boost (in milliseconds). |

#### 4️⃣ Core Framework & Allocation (`CoreFramework` & `CoreAllocation`)

This section defines your device's physical core architecture and is the foundation for all frequency and core control functions. **It must be configured correctly\!**

  * **Core Framework (`CoreFramework`)**: Tells the program which `policy` path corresponds to each core cluster (can be found in the `/sys/devices/system/cpu/cpufreq/` directory).
    ```yaml
    CoreFramework:
      SmallCorePath: 0
      MediumCorePath: 2
      BigCorePath: 5
      SuperBigCorePath: 7
    ```
  * **Core Allocation (`CoreAllocation`)**: Provides parameters for the `AffinitySetter` feature, specifying the core range to which critical system processes (`yuki` cgroup) will be bound.
    ```yaml
    CoreAllocation:
      CpuSetCore: "2-7"
    ```

#### 5️⃣ I/O Scheduling (`IO_Settings`)

```yaml
IO_Settings:
  Scheduler: "none"
  Io_optimization: true
```

| Field | Type | Description |
| :--- | :--- | :--- |
| `Scheduler` | string | **(Requires \`AdjIOScheduler\` to be on)** Sets the I/O scheduler, e.g., "mq-deadline", "none". |
| `Io_optimization` | bool | Whether to disable `iostats` and `nomerges`, etc., to optimize I/O performance. |

#### 6️⃣ EAS Scheduler (`EasSchedulerValue`)

Requires `function.EasScheduler` to be `true`.

```yaml
EasSchedulerValue:
  sched_min_granularity_ns: "1000000"
  sched_nr_migrate: "32"
  sched_wakeup_granularity_ns: "1000000"
  sched_schedstats: "0"
```

#### 7️⃣ CPU Idle (`CpuIdle`)

Requires `function.CpuIdleScaling_Governor` to be `true`.

```yaml
CpuIdle:
  current_governor: "ladder"
```

  * `current_governor`: Sets the CPU Idle governor.

#### 8️⃣ Cpuset (Core Grouping)

Requires `function.cpuset` to be `true`. This restricts different types of task groups to run on specified CPU cores.

```yaml
Cpuset:
  top_app: "0-7"
  foreground: "0-7"
  background: "0-3"
  system_background: "0-2"
  restricted: "0-1"
```

| Field | Description | Recommended Value |
| :--- | :--- | :--- |
| `top_app` | The application currently running in the foreground. | Should be assigned all cores, e.g., `"0-7"`. |
| `foreground` | Foreground services and visible applications. | Should also be assigned all or most cores. |
| `background` | Applications and services running in the background. | **Should be restricted to efficiency cores**, e.g., `"0-3"`, to save power. |
| `system_background` | System background services. | Should also be restricted to efficiency cores. |
| `restricted` | Background apps that are restricted by the system. | Should be assigned the minimum number of cores. |

#### 9️⃣ Bus Frequency Control (`Bus_dcvs_Path` & `Bus_dcvs`)

This feature allows for fine-grained control over the SoC's internal data bus (LLCC cache/DDR memory) frequency. Configuration is a two-step process:

1.  **Global Path Definition (`Bus_dcvs_Path`)**: A **one-time** setup to tell the program where the system files for controlling bus frequency are located. The program intelligently detects which paths you've filled and only acts on those.
    ```yaml
    # Qualcomm platform example
    Bus_dcvs_Path:
      CPUllccminPath: "/sys/devices/system/cpu/bus_dcvs/L3/soc:qcom,memlat:l3:prime/min_freq"
      CPUllccmaxPath: "/sys/devices/system/cpu/bus_dcvs/L3/soc:qcom,memlat:l3:prime/max_freq"
      CPUddrminPath: "/sys/devices/system/cpu/bus_dcvs/DDR/soc:qcom,memlat:ddr:prime/min_freq"
      CPUddrmaxPath: "/sys/devices/system/cpu/bus_dcvs/DDR/soc:qcom,memlat:ddr:prime/max_freq"
    ```
2.  **Per-Mode Value Setting (`Bus_dcvs`)**: Within **each performance mode**, set the specific frequency values to be written.
    ```yaml
    # performance mode example
    performance:
      Bus_dcvs:
        CPUllccmin: 1555000
        CPUddrmax: 3196000
    ```

#### 🔟 Dynamic Governor Parameters (`pGovPath` & `Govsets`)

This feature allows for fine-tuning the internal parameters of the CPU governor. This is also a two-step process:

1.  **Define Available Parameters (`pGovPath`)**: Create a "parameter dictionary," grouped by **governor name**, defining the **file names** of all parameters you might want to use.
    ```yaml
    pGovPath:
      schedutil:
        path1: "up_rate_limit_us"
      walt:
        path1: "target_loads"
    ```
2.  **Set Parameter Values in Modes (`Govsets`)**: Within **each performance mode**, also grouped by **governor name**, use the **keys** defined in `pGovPath` to set specific **values**. The program will intelligently apply these settings only to the cores currently using that governor.
    ```yaml
    # performance mode example
    performance:
      Govsets:
        schedutil:
          path1: "0"      # Corresponds to up_rate_limit_us
        walt:
          path1: "95"     # Corresponds to target_loads
    ```

#### 1️⃣1️⃣ Power Model Explained (using `performance` mode as an example)

A complete performance mode is defined by the combination of the following six modules. You can mix and match them to create the perfect mode for your needs.

```yaml
performance:
  Governor: { ... } # Governor: Determines how CPU frequency responds to load
  Freq: { ... }     # CPU Frequency: Defines min/max frequency for each core cluster
  Uclamp: { ... }   # Uclamp: Provides hints to the scheduler about performance needs (0-100)
  Bus_dcvs: { ... } # Bus Frequency: Sets the internal SoC data bus frequency
  Govsets: { ... }  # Governor Parameters: Fine-tunes the behavior of the current governor
  Other: { ... }    # Other settings
```

**Detailed Explanation:**

  * **`Governor` (Governor)**:
      * `Global`: "schedutil" (Global default)
      * `SmallCore`: "" (Uses global if empty)
      * ... (Other core clusters)
  * **`Freq` (CPU Frequency)**:
      * `SmallCoreMinFreq`: 0 (or "min")
      * `SmallCoreMaxFreq`: 9999999 (or "max")
      * ... (Other core clusters)
      * **Note**: Frequency fields support `"min"` and `"max"` strings, which the daemon will convert to `0` and `9999999` (or `10000000`) respectively.
  * **`Uclamp` (Uclamp Settings)**:
      * `UclampTopAppMin`: "0"
      * `UclampTopAppMax`: "100"
      * `UclampTopApplatency_sensitive`: "0"
      * `UclampForeGroundMin`: "0"
      * `UclampForeGroundMax`: "70"
      * `UclampBackGroundMin`: "0"
      * `UclampBackGroundMax`: "50"
  * **`Bus_dcvs` (Bus Frequency)**:
      * `CPUllccmin`: ""
      * `CPUllccmax`: ""
      * ...
  * **`Govsets` (Governor Parameters)**:
      * (Structure as described above)
  * **`Other` (Other Settings)**:
      * `UfsClkGate`: false (Whether to disable UFS clock gating)

## 📥 Installation Instructions

### Prerequisites

1.  **Obtain Root Access**

### Installation Steps

1.  **Download the App** - Download the latest APK from the [Releases](https://github.com/imacte/YukiCtrl/releases) page.
2.  **Install the App** - Allow installation from unknown sources.
3.  **First Run** - The app will automatically request Root access and initialize the system.
4.  **Configure Permissions** - Follow the in-app prompts to grant necessary permissions like the Accessibility Service.

## 🚀 Performance Optimization Suggestions

### Daily Use

1.  **Use Balance Mode** - Provides the best performance/power balance for most apps.
2.  **Set App Rules** - Set gaming apps to Performance or Fast mode.

### Gaming Optimization

1.  **Use Performance/Fast Mode** - Unleash maximum performance for gaming.
2.  **Enable App Launch Boost** - Reduce game loading times.
3.  **Adjust Core Allocation** - Ensure the game process has sufficient CPU resources.
4.  **Monitor Temperature** - Pay attention to device temperature during extended high-performance sessions.

### Power Saving Optimization

1.  **Use Powersave Mode** - Maximize battery life in low-load scenarios.
2.  **Restrict Background Apps** - Use `Cpuset` to limit CPU usage for background apps.
3.  **Optimize I/O Scheduler** - Reduce power consumption from storage access.
4.  **Disable Unneeded Features** - Turn off advanced features as needed to save power.

## 🔍 Troubleshooting

### Frequently Asked Questions

**Q: The app can't get Root access?**

  * Ensure your device is properly rooted and Magisk is installed.
  * Check your Magisk settings to ensure it has granted YukiCtrl's Root request.
  * Try reinstalling the app or restarting the device.

**Q: Smart Dynamic Mode isn't working?**

  * Verify that app rules are configured correctly.
  * Verify that the yuki-daemon module is installed and running correctly.

**Q: Performance modes aren't switching?**

  * Verify that the yuki-daemon module is installed and running correctly.
  * View the yuki-daemon module logs to identify specific error messages.
  * Verify the configuration file format is correct (`config.yaml` is case-sensitive).



## 📊 Project Statistics

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=imacte/YukiCtrl&type=Date)](https://star-history.com/#imacte/YukiCtrl&Date)

</div>

## 📮 Contact Us

  * **GitHub Issues** - [For project issues and suggestions](https://github.com/imacte/YukiCtrl/issues)
  * **Telegram** - [Join TG Channel](https://t.me/+gp4adLJAsXYzMjc1)

-----

<div align="center">

<sub>📅 Document Updated: 2025-10-11</sub><br>
<sub>🚀 YukiCtrl - Giving every Android device the best performance experience</sub>

</div>