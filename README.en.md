[阅读中文文档](README.md)

# yumi - Intelligent CPU Scheduling Controller

<div align="center">

[![Android](https://img.shields.io/badge/platform-Android-3DDC84.svg?style=for-the-badge&logo=android)](https://developer.android.com/)
[![Rust](https://img.shields.io/badge/core-Rust-%23dea584.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![WebUI](https://img.shields.io/badge/UI-WebUI-4FC08D.svg?style=for-the-badge&logo=html5)](https://developer.mozilla.org/en-US/docs/Web/HTML)
[![AArch64](https://img.shields.io/badge/arch-AArch64-FF6B6B.svg?style=for-the-badge)](https://en.wikipedia.org/wiki/AArch64)
[![Root Required](https://img.shields.io/badge/Root-Required-FF5722.svg?style=for-the-badge)](https://magiskmanager.com/)

**🚀 An Intelligent CPU Scheduling System - Lightweight WebUI + High-Performance Rust Daemon + Built-in FAS Frame-Aware Scheduling**

</div>

-----

## 📋 About The Project

**yumi** is a powerful Android CPU scheduling control system, consisting of a lightweight **WebUI** management interface and a high-performance **Rust daemon (yumi)**. Through advanced scheduling algorithms and highly configurable performance models, it dynamically adjusts CPU frequency, core allocation strategies, and fine-tunes governor parameters for different usage scenarios to achieve the optimal balance between performance and power efficiency. The built-in **FAS (Frame Aware Scheduling)** engine analyzes game frame times in real time to dynamically adjust CPU frequency on a per-frame basis, maximizing power savings while maintaining smooth gameplay.

### ✨ Key Features

  * 🔄 **Smart Dynamic Mode Switching** - Automatically adjusts performance modes based on the current application.
  * 🎯 **FAS Frame-Aware Scheduling** - Built-in frame time analysis engine for per-frame dynamic frequency scaling, balancing smoothness and power in gaming scenarios.
  * 🌐 **Lightweight WebUI** - No extra app required; manage all scheduling settings directly from your browser.
  * 📱 **App Rule Management** - Set dedicated performance strategies for different applications.
  * ⚡ **App Launch Boost** - Monitors cgroup changes to provide a temporary performance boost during app launch.
  * 🔧 **Highly Configurable** - YAML configuration files support deep customization with hot-reload — no restart needed.

## 🔧 System Requirements

  * **Android Version**: Android 8.0 (API 26) and above.
  * **Architecture Support**: ARM64 (AArch64).
  * **Permissions Required**: Root access.

## 🎯 Performance Modes

yumi offers five performance modes:

| Mode | Icon | Characteristics | Use Case |
| :--- | :--- | :--- | :--- |
| **Powersave** | 🔋 | Maximizes battery life, reduces performance output. | Standby, light use, reading. |
| **Balance** | ⚖️ | The optimal balance between performance and power consumption. | Daily use, social apps. |
| **Performance** | ⚡ | Prioritizes performance with a moderate increase in power consumption. | Large applications, light gaming. |
| **Fast** | 🚀 | Unleashes maximum performance, ignoring power consumption. | Heavy gaming, performance testing. |
| **FAS (Frame-Aware Scheduling)** | 🎯 | Analyzes frame times in real time, dynamically scales frequency per frame, and automatically switches gear levels. | Gaming scenarios — balances smoothness and power saving. |

## 🌐 WebUI Management Interface

yumi includes a lightweight built-in WebUI. All management operations can be performed through a browser — no extra app installation needed.

  * **Mode Switching** - Switch performance modes in real time.
  * **App Rule Management** - Configure dedicated performance strategies for different apps.
  * **Configuration Editing** - Edit YAML configuration files online.
  * **Log Viewer** - View yumi daemon logs in real time.

-----

### 🛠️ Scheduling Core (yumi)

The core of yumi is driven by a Rust daemon, **yumi**. It is responsible for executing all low-level system tuning commands, achieving efficient performance control with extremely low resource consumption.

#### Core Features

  * **High-Performance Rust Implementation**: Extremely low system resource usage and minimal power consumption.
  * **Real-time Configuration Monitoring**: Supports hot-reloading for configuration (`config.yaml`) and mode (`mode.txt`) files, allowing mode switches without a reboot.
  * **Multi-level Optimization Strategy**: Comprehensive tuning from CPU frequency to I/O scheduling.
  * **Smart App Launch Boost**: Monitors `top-app` cgroup changes to provide a temporary performance boost during app launch, speeding up loading times.
  * **Built-in FAS Engine**: Frame-aware scheduling with per-frame dynamic frequency scaling — no external modules required.

#### Scheduling Functions

| Feature Module | Description |
| :--- | :--- |
| **CPU Frequency Control** | Dynamically adjusts the min/max frequency for each core cluster. |
| **FAS Frame-Aware Scheduling** | Built-in frame time analysis engine that monitors real-time frame intervals and maps them to CPU frequency via `perf_index`, enabling per-frame dynamic frequency scaling. |
| **Governor Management** | Supports fine-grained tuning of various governors like schedutil, walt, and their internal parameters. |
| **Core Allocation (Cpuset)** | Assigns appropriate CPU cores to different task groups (foreground, background, etc.), key for managing power and performance. |
| **I/O Scheduler Optimization** | Iterates over all block devices with customizable I/O schedulers, read-ahead size, merge policy, and iostats parameters. |
| **EAS Scheduler Tuning** | Advanced parameter optimization for kernels that support Energy Aware Scheduling (EAS). |
| **Core Binding Optimization (AffinitySetter)** | Automatically creates `yumi` and `Rubbish` cgroups. Binds critical system processes (e.g., `surfaceflinger`) to the `yumi` group and isolates interfering processes (e.g., `kswapd0`, `logcat`) to the `Rubbish` group, significantly improving UI smoothness. |
| **Conflict Management** | Automatically disables most common userspace and kernel-level performance boosters (like FEAS, except in Fast mode) to ensure the scheduler's policy is the single source of truth. |

-----

### 🎯 FAS Frame-Aware Scheduling — In Depth

FAS (Frame Aware Scheduling) is yumi's built-in frame-aware dynamic frequency scaling engine, designed specifically for gaming scenarios. Unlike traditional static modes, FAS precisely controls CPU frequency by analyzing the rendering time of every frame in real time, minimizing power consumption while ensuring smoothness.

#### How It Works

The FAS engine maintains a **perf_index** (performance index, range 0–1000) and adjusts it based on real-time frame time feedback:

  * **Frame time exceeds budget** → perf_index rises → CPU frequency increases
  * **Frame time meets budget** → perf_index slowly falls → CPU frequency decreases
  * **perf_index is mapped to actual frequency steps for each core cluster via linear interpolation**

#### Core Mechanisms

  * **Automatic Frame Rate Gear Switching**: Supports multiple frame rate targets (e.g., 30/60/90/120/144 fps) with automatic up/downshift based on actual rendering capability. Before downshifting, the engine first attempts a frequency boost to confirm whether a downshift is truly necessary, preventing false downshifts.
  * **Loading Scene Detection**: Automatically identifies game loading screens (sustained heavy frames). Upon entering loading state, it locks to mid-to-high frequencies and resumes normal scheduling with protection after loading ends. Supports both hard loading (true heavy frames) and soft loading (sudden frame rate drop without reaching the heavy-frame threshold).
  * **Scene Transition Awareness**: Detects scene transitions (e.g., map changes) using the coefficient of variation of frame times. During transitions, frequency adjustment amplitude is reduced to prevent severe fluctuations.
  * **Frequency Hysteresis**: Hysteresis bands are set between adjacent frequency steps to prevent rapid toggling at boundaries.
  * **Jank Cooldown**: After a severe frame drop, the engine enters a cooldown period during which it maintains a higher frequency to avoid triggering a chain of stutters by immediately reducing frequency.
  * **External Lock Detection**: Detects whether the frequency has been overridden by external factors (e.g., thermal throttling). If a persistent mismatch is detected, it temporarily backs off and yields control, automatically recovering after the cooldown period.
  * **Windowed Mode Support**: The FAS state supports suspend/resume. After a brief interruption (e.g., windowed mode operation), scheduling can resume quickly without re-initialization.

#### FAS Configuration (`rules.yaml`)

FAS parameters are configured in the `fas_rules` section of `config/rules.yaml`:

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

| Parameter | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `fps_gears` | float[] | [20,24,30,45,60,90,120,144] | Supported frame rate gear list; FAS automatically switches between these levels. |
| `fps_margin` | string | "3.0" | Frame rate margin (fps). EMA budget = 1000 / (target − margin), providing a tolerance buffer. |
| `heavy_frame_threshold_ms` | float | 150.0 | Heavy frame threshold (ms). Frames exceeding this value are treated as loading frames. |
| `loading_cumulative_ms` | float | 2500.0 | Enters loading state when cumulative heavy frame duration exceeds this value. |
| `post_loading_ignore_frames` | int | 5 | Number of frames to ignore after loading ends, used to filter transition noise. |
| `post_loading_perf_min` | float | 500.0 | Minimum perf_index after loading ends. |
| `post_loading_perf_max` | float | 800.0 | Maximum perf_index after loading ends. |
| `instant_error_threshold_ms` | float | 4.0 | Instantaneous error threshold. Triggers emergency frequency boost when frame time exceeds budget by more than this value. |
| `perf_floor` | float | 150.0 | Minimum perf_index, preventing the frequency from dropping too low. |
| `freq_hysteresis` | float | 0.015 | Frequency hysteresis coefficient, preventing frequent toggling between adjacent steps. |

-----

### ⚙️ Advanced Configuration (`config.yaml` Explained)

yumi uses a YAML-formatted configuration file, allowing for deep customization.

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
  CpuIdleScalingGovernor: false
  EasScheduler: true
  cpuset: true
  LoadBalancing: true
  EnableFeas: false
  IOOptimization: true
  AppLaunchBoost: true
```

| Function | Description |
| :--- | :--- |
| `AffinitySetter` | **(Recommended)** **Do not enable on HyperOS 3**. Enables core binding optimization (`yumi` and `Rubbish` cgroups). |
| `CpuIdleScalingGovernor`| Whether to allow custom CPU Idle governors (see `CpuIdle` section). |
| `EasScheduler` | If the kernel supports **EAS**, enabling this will apply optimized parameters. |
| `cpuset` | **(Recommended)** Enables the Cpuset feature to assign different task groups to appropriate CPU cores (see `Cpuset` section). |
| `LoadBalancing` | Enables CFS load balancing optimizations for more rational task distribution across cores. |
| `EnableFeas` | Whether to attempt enabling the kernel's FEAS feature in **Fast mode**. |
| `IOOptimization` | Enables I/O optimization, iterating over all block devices to apply scheduler and parameter settings (see `IO_Settings` section). |
| `AppLaunchBoost` | **(Recommended)** Enables app launch acceleration to speed up loading times (see `AppLaunchBoostSettings` section). |

#### 3️⃣ App Launch Boost (`AppLaunchBoostSettings`)

Requires `function.AppLaunchBoost` to be `true`.

```yaml
AppLaunchBoostSettings:
  BoostRateMs: 600
  SmallCoreBoostFreq: "max"
  MediumCoreBoostFreq: "max"
  BigCoreBoostFreq: "max"
  SuperBigCoreBoostFreq: "max"
```

| Field | Type | Description |
| :--- | :--- | :--- |
| `BoostRateMs` | int | Duration of the launch boost (in milliseconds). |
| `SmallCoreBoostFreq` | string/int | Boost frequency for small cores. Supports `"min"`, `"max"`, or a specific value. Leave empty for `"max"`. |
| `MediumCoreBoostFreq` | string/int | Boost frequency for medium cores. Same as above. |
| `BigCoreBoostFreq` | string/int | Boost frequency for big cores. Same as above. |
| `SuperBigCoreBoostFreq` | string/int | Boost frequency for super-big cores. Same as above. |

#### 4️⃣ Core Framework & Allocation (`CoreFramework` & `CoreAllocation`)

This section defines your device's physical core architecture and is the foundation for all frequency and core control functions. **It must be configured correctly!**

  * **Core Framework (`CoreFramework`)**: Tells the program which `policy` path corresponds to each core cluster (can be found in the `/sys/devices/system/cpu/cpufreq/` directory). Set to `-1` if the core cluster does not exist.
    ```yaml
    CoreFramework:
      SmallCorePath: 0
      MediumCorePath: 2
      BigCorePath: 5
      SuperBigCorePath: 7
    ```
  * **Core Allocation (`CoreAllocation`)**: Provides parameters for the `AffinitySetter` feature, specifying the core range to which critical system processes (`yumi` cgroup) will be bound.
    ```yaml
    CoreAllocation:
      CpuSetCore: "2-7"
    ```

#### 5️⃣ I/O Settings (`IO_Settings`)

Requires `function.IOOptimization` to be `true`. When enabled, iterates over all block devices under `/sys/block/*` and applies the following parameters to each (paths are checked automatically).

```yaml
IO_Settings:
  Scheduler: "none"
  read_ahead_kb: "128"
  nomerges: "2"
  iostats: "0"
```

| Field | Type | Description |
| :--- | :--- | :--- |
| `Scheduler` | string | I/O scheduler, e.g., `"none"`, `"mq-deadline"`, `"bfq"`, `"kyber"`. Leave empty to keep the system default. |
| `read_ahead_kb` | string | Read-ahead size (KB). |
| `nomerges` | string | Merge policy. `0` = allow merges, `1` = simple merges only, `2` = disable merges. |
| `iostats` | string | I/O statistics. `0` = disable (recommended, reduces overhead), `1` = enable. |

#### 6️⃣ CFS Scheduler Parameters (`CompletelyFairSchedulerValue`)

```yaml
CompletelyFairSchedulerValue:
  sched_child_runs_first: ""
  sched_rt_period_us: ""
  sched_rt_runtime_us: ""
```

Fields left empty will not be written and will retain their system default values.

#### 7️⃣ CPU Idle (`CpuIdle`)

Requires `function.CpuIdleScalingGovernor` to be `true`.

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

#### 9️⃣ Dynamic Governor Parameters (`pGovPath` & `Govsets`)

This feature allows for fine-tuning the internal parameters of the CPU governor. This is a two-step process:

1.  **Define Available Parameters (`pGovPath`)**: Create a "parameter dictionary," grouped by **governor name**, defining the **pure file names** of all parameters you might want to use.
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
          path1:
            SmallCore: "0"
            MediumCore: "500"
            BigCore: "0"
            SuperBigCore: "0"
    ```

#### 🔟 Power Model Explained (using `performance` mode as an example)

A complete performance mode is defined by the combination of the following **five modules**. You can mix and match them freely to create the perfect mode for your needs.

```yaml
performance:
  Governor: { ... } # Governor: Determines how CPU frequency responds to load
  Freq: { ... }     # CPU Frequency: Defines min/max frequency for each core cluster
  Uclamp: { ... }   # Uclamp: Provides hints to the scheduler about performance needs (0-100)
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
      * **Note**: Frequency fields support `"min"` and `"max"` strings, which the daemon will convert to `0` and `9999999` respectively.
  * **`Uclamp` (Uclamp Settings)**:
      * `UclampTopAppMin`: "0"
      * `UclampTopAppMax`: "100"
      * `UclampTopApplatency_sensitive`: "0"
      * `UclampForeGroundMin`: "0"
      * `UclampForeGroundMax`: "70"
      * `UclampBackGroundMin`: "0"
      * `UclampBackGroundMax`: "50"
  * **`Govsets` (Governor Parameters)**:
      * (Structure as described above)
  * **`Other` (Other Settings)**:
      * `ufsClkGate`: false (Whether to disable UFS clock gating)

## 📥 Installation Instructions

### Prerequisites

1.  **Obtain Root Access**

### Installation Steps

1.  **Download the Module** - Download the latest release from the [Releases](https://github.com/imacte/YukiCtrl/releases) page.
2.  **Flash the Module** - Flash the yumi module via Magisk / KernelSU.
3.  **Access the WebUI** - Once the module starts, open the WebUI in your browser to manage and configure settings.
4.  **Configure Rules** - Set performance strategies for different apps as needed.

## 🚀 Performance Optimization Suggestions

### Daily Use

1.  **Use Balance Mode** - Provides the best performance/power balance for most apps.
2.  **Set App Rules** - Set gaming apps to Performance or Fast mode.

### Gaming Optimization

1.  **Use FAS Mode** - Frame-aware scheduling automatically saves power while maintaining smoothness; recommended as the primary gaming mode.
2.  **Adjust FAS Parameters** - Tune the frame rate gears and margin in `rules.yaml` based on the game's characteristics.
3.  **Use Performance/Fast Mode** - For scenarios FAS cannot cover, switch to a static high-performance mode.
4.  **Enable App Launch Boost** - Reduce game loading times.
5.  **Monitor Temperature** - Pay attention to device temperature during extended high-performance sessions.

### Power Saving Optimization

1.  **Use Powersave Mode** - Maximize battery life in low-load scenarios.
2.  **Restrict Background Apps** - Use `Cpuset` to limit CPU usage for background apps.
3.  **Optimize I/O Scheduler** - Reduce power consumption from storage access.
4.  **Disable Unneeded Features** - Turn off advanced features as needed to save power.

## 🔍 Troubleshooting

### Frequently Asked Questions

**Q: The module can't get Root access?**

  * Ensure your device is properly rooted and Magisk / KernelSU is installed.
  * Check your Root manager settings to ensure the yumi Root request has been granted.
  * Try reflashing the module or restarting the device.

**Q: Smart Dynamic Mode isn't working?**

  * Verify that app rules are configured correctly.
  * Verify that the yumi module is installed and running correctly.

**Q: Performance modes aren't switching?**

  * Verify that the yumi module is installed and running correctly.
  * View the yumi module logs to identify specific error messages.
  * Verify the configuration file format is correct (`config.yaml` is case-sensitive).

**Q: Frame rate is unstable in FAS mode?**

  * Check that `fps_gears` in `rules.yaml` includes the target frame rate.
  * Increasing `fps_margin` provides more headroom and reduces boundary fluctuations.
  * Check the FAS heartbeat entries in the logs (output every 30 frames) to confirm the scheduling state is normal.
  * If the frequency is being overridden by thermal throttling, an "externally locked" message will appear in the logs — this is normal backoff behavior.

## 📊 Project Statistics

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=imacte/YukiCtrl&type=Date)](https://star-history.com/#imacte/YukiCtrl&Date)

</div>

## 📮 Contact Us

  * **GitHub Issues** - [For project issues and suggestions](https://github.com/imacte/YukiCtrl/issues)
  * **QQ Group** - 1036909137
  * **Telegram** - [Join TG Channel](https://t.me/+gp4adLJAsXYzMjc1)

-----

<div align="center">

<sub>📅 Document Updated: February 25, 2026</sub><br>
<sub>🚀 yumi - Giving every Android device the best performance experience</sub>

</div>