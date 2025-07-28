# YukiCtrl - 智能 CPU 调度控制器

<div align="center">

[![Android](https://img.shields.io/badge/platform-Android-3DDC84.svg?style=for-the-badge&logo=android)](https://developer.android.com/)
[![Kotlin](https://img.shields.io/badge/language-Kotlin-7F52FF.svg?style=for-the-badge&logo=kotlin)](https://kotlinlang.org/)
[![C++](https://img.shields.io/badge/core-C++-%23f34b7d.svg?style=for-the-badge&logo=cplusplus)](https://en.wikipedia.org/wiki/C++)
[![AArch64](https://img.shields.io/badge/arch-AArch64-FF6B6B.svg?style=for-the-badge)](https://en.wikipedia.org/wiki/AArch64)
[![Root Required](https://img.shields.io/badge/Root-Required-FF5722.svg?style=for-the-badge)](https://magiskmanager.com/)

**🚀 智能 CPU 调度系统 - 现代化 Android 应用 + 高性能 C++ 守护进程**

</div>

---

## 📋 项目介绍

**YukiCtrl** 是一个功能强大的 Android CPU 调度控制应用，由现代化的 **Kotlin + Jetpack Compose** 用户界面和高性能的 **C++ 守护进程 (YukiCpuScheduler)** 组成。通过先进的调度算法和高度可配置的性能模型，它能够根据不同的使用场景动态调整 CPU 频率、总线速度、核心分配策略以及精细的调速器参数，实现最佳的性能与能效平衡。

### ✨ 主要特性

- 🎨 **现代化 Material 3 设计** - 遵循 MIUI 设计语言的精美界面
- 🔄 **智能动态模式切换** - 根据当前应用自动调整性能模式
- ⚡ **实时性能监控** - 通过通知栏实时显示当前模式和应用信息
- 🎮 **悬浮窗快速控制** - 无需返回应用即可快速调整性能模式
- 📱 **应用规则管理** - 为不同应用设置专属的性能策略
- 🎯 **多种调度核心支持** - 支持 YukiCpuScheduler 和通用脚本模式
- 🌈 **丰富的主题系统** - 多种配色方案和自定义背景支持
- 🔧 **高级配置编辑** - 内置 YAML 配置编辑器和日志查看器

## 🔧 系统要求

- **Android 版本**: Android 8.0 (API 26) 及以上
- **架构支持**: ARM64 (AArch64)
- **权限要求**: Root 权限

## 🎯 性能模式

YukiCtrl 提供四种性能模式：

| 模式 | 图标 | 特点 | 适用场景 |
|------|------|------|----------|
| **省电 (Powersave)** | 🔋 | 最大化续航，降低性能释放 | 待机、轻度使用、阅读 |
| **均衡 (Balance)** | ⚖️ | 性能与功耗的最佳平衡点 | 日常使用、社交应用 |
| **性能 (Performance)** | ⚡ | 优先性能，适度增加功耗 | 大型应用、轻度游戏 |
| **极速 (Fast)** | 🚀 | 最大性能释放，忽略功耗 | 重度游戏、性能测试 |

## 📱 应用功能详解

### 🔄 智能动态模式

- **无障碍服务集成** - 通过无障碍服务检测应用切换
- **应用规则管理** - 为不同应用设置专属性能策略
- **实时模式切换** - 根据当前运行的应用自动调整性能模式
- **全局默认模式** - 为未设置规则的应用提供默认性能模式

### 🎮 悬浮窗控制

- **快速模式切换** - 无需返回应用即可调整性能
- **实时信息显示** - 显示当前应用和性能模式
- **拖拽自由定位** - 悬浮窗位置可自由调整
- **主题跟随** - 悬浮窗外观跟随应用主题

### 📊 系统监控

- **持续前台通知** - 通知栏实时显示当前状态
- **应用信息展示** - 显示当前运行应用的名称
- **模式状态指示** - 清晰显示当前激活的性能模式
- **点击交互** - 点击通知可快速打开悬浮窗

### 🔧 高级功能

- **详细配置编辑** - 内置 YAML 配置文件编辑器
- **实时日志查看** - 查看 YukiCpuScheduler 守护进程日志
- **脚本管理** - 独立的系统优化脚本开关控制
- **自动核心检测** - 智能识别设备的 CPU 核心架构

## 🛠️ 调度核心 (YukiCpuScheduler)

### 核心特性

- **高性能 C++ 实现** - 极低的系统资源占用
- **实时配置监听** - 支持配置文件热重载
- **多层次优化策略** - 从 CPU 频率到总线速度的全方位调优
- **智能应用启动加速** - 应用启动时的临时性能提升

### 调度功能

| 功能模块 | 描述 |
|----------|------|
| **CPU 频率控制** | 动态调整各核心簇的最小/最大频率 |
| **调速器管理** | 支持 schedutil、walt 等多种调速器 |
| **核心分配 (Cpuset)** | 为不同任务组分配合适的 CPU 核心 |
| **总线频率优化** | 调整 DDR 内存和 LLCC 缓存频率 |
| **I/O 调度优化** | 优化存储设备的访问策略 |
| **EAS 调度器调优** | 针对支持 EAS 的内核进行参数优化 |

## 📥 安装说明

### 前置要求

1. **获取 Root 权限** - 推荐使用 [Magisk](https://github.com/topjohnwu/Magisk)
2. **启用开发者选项** - 用于授予应用必要的系统权限

### 安装步骤

1. **下载应用** - 从 [Releases](https://github.com/imacte/YukiCtrl/releases) 下载最新版本的 APK
2. **安装应用** - 允许来自未知来源的应用安装
3. **首次运行** - 应用会自动请求 Root 权限并初始化系统
4. **权限配置** - 根据应用提示完成无障碍服务等权限的配置

### 配置建议

1. **开启无障碍服务** - 启用智能动态模式功能
2. **授予悬浮窗权限** - 启用悬浮窗快速控制功能
3. **设置应用规则** - 为常用应用配置专属的性能策略
4. **调整通知设置** - 确保状态通知始终显示

## 🎨 界面与主题

### Material 3 设计

YukiCtrl 采用最新的 Material 3 设计规范，提供现代化的用户体验：

- **动态颜色系统** - 支持系统动态颜色主题
- **自适应布局** - 完美适配不同屏幕尺寸
- **流畅动画** - 精心设计的过渡动画和交互反馈
- **无障碍支持** - 完整的无障碍功能支持

### 多主题支持

- **MIUI 默认** - 经典的 MIUI 蓝色主题
- **MIUI 拿铁** - 温暖的拿铁咖啡色调
- **MIUI 森绿** - 清新的森林绿色主题
- **自定义背景** - 支持设置个人图片作为应用背景

### 自定义选项

- **基础模式** - 浅色/深色/跟随系统
- **背景效果** - 可调整背景模糊程度和遮罩透明度
- **悬浮窗主题** - 悬浮窗外观跟随应用主题设置

## ⚙️ 高级配置

### 配置文件结构

YukiCtrl 使用 YAML 格式的配置文件，支持以下主要配置项：

```yaml
# 基本信息
meta:
  name: "YukiCpuScheduler Profile"
  author: "yuki"
  configVersion: 19

# 功能开关
function:
  DisableQcomGpu: true
  AffinitySetter: true
  EasScheduler: true
  AppLaunchBoost: true

# 核心架构定义
CoreFramework:
  SmallCorePath: 0
  BigCorePath: 4

# 性能模式配置
balance:
  Governor:
    global: "schedutil"
  Freq:
    SmallCoreMaxFreq: "1804800"
    BigCoreMaxFreq: "2419200"
```

### 脚本系统

YukiCtrl 支持独立的系统优化脚本：

- **CFS 调度器优化** - `adj_cfs.sh`
- **WALT 调度器优化** - `adj_walt.sh`
- **高通 GPU 优化** - `adj_qcom_gpu.sh`
- **厂商 Boost 禁用** - `disable_boost.sh`
- **高通总线调整** - `adj_qcom_bus.sh` 

## 🚀 性能优化建议

### 日常使用

1. **使用均衡模式** - 为大部分应用提供最佳的性能功耗平衡
2. **设置应用规则** - 为游戏应用设置性能或极速模式
3. **启用智能切换** - 让系统根据应用自动调整性能
4. **合理使用悬浮窗** - 在需要时快速调整性能模式

### 游戏优化

1. **使用性能/极速模式** - 为游戏提供最大性能释放
2. **启用应用启动加速** - 减少游戏加载时间
3. **调整核心分配** - 确保游戏进程获得足够的 CPU 资源
4. **监控温度** - 长时间高性能使用时注意设备温度

### 省电优化

1. **使用省电模式** - 在低负载场景下最大化续航
2. **限制后台应用** - 通过 Cpuset 限制后台应用的 CPU 使用
3. **优化 I/O 调度** - 减少存储访问的功耗开销
4. **关闭不需要的功能** - 根据需要禁用部分高级功能

## 🔍 故障排除

### 常见问题

**Q: 应用无法获取 Root 权限？**
- 确保设备已正确 Root 并安装 Magisk
- 检查 Magisk 设置中是否允许了 YukiCtrl 的 Root 请求
- 尝试重新安装应用或重启设备

**Q: 智能动态模式不工作？**
- 确保已开启无障碍服务权限
- 检查应用是否在省电白名单中
- 验证应用规则是否正确配置

**Q: 悬浮窗无法显示？**
- 检查是否已授予悬浮窗权限
- 确保无障碍服务正常运行
- 尝试手动开启悬浮窗功能

**Q: 性能模式切换无效？**
- 检查 YukiCpuScheduler 守护进程是否正常运行
- 查看应用日志以确定具体错误信息
- 验证配置文件格式是否正确

## 📊 项目统计

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=imacte/YukiCtrl&type=Date)](https://star-history.com/#imacte/YukiCtrl&Date)

</div>

## 📮 联系我们

- **GitHub Issues** - [项目问题和建议](https://github.com/imacte/YukiCtrl/issues)
- **QQ 群 1** - 1036909137 (用户交流)
- **QQ 群 2** - 1055174076 (技术讨论)
- **酷安** - [@yuki](https://www.coolapk.com/u/yuki) (项目动态)

---

<div align="center">

**🌟 如果这个项目对您有帮助，请给我们一个 Star！**

<sub>让更多的 Android 用户体验到智能调度带来的性能提升</sub>

---

<sub>📅 文档更新时间：2025年7月28日</sub><br>
<sub>🚀 YukiCtrl - 让每一台 Android 设备都拥有最佳的性能体验</sub>

</div>
