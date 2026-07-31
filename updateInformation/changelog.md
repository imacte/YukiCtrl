# 🚀 更新日志 | Changelog

## 📦 v2.0.2

### 🎯 新增 (New Features)
* **[CLG]** 新增 8 个可调参数，全面开放调频行为定制：
  - `headroom_ramp`：headroom 在 up_threshold 附近的过渡带宽度（默认 0.15）
  - `up_jump_threshold`：快速升频通道的跳变幅度阈值（默认 0.35）
  - `slow_up_scale`：滞回带内升频的最低速率基准（默认 0.02）
  - `slow_down_scale`：滞回带内降频的缩放系数（默认 0.5）
  - `down_fast_threshold` / `down_fast_mult`：极低负载快速降频的触发阈值与放大倍数
  - `spike_jump_threshold` / `spike_decay`：单 tick 尖峰抑制的跳变阈值与衰减比例

### ⚡️ 优化与特性 (Optimizations & Features)
* **[CLG]** CPU 负载调速器全面重构：
  - **降频不再锁死高位**：滞回带内（down_threshold ~ up_threshold）目标低于当前即可降频，按慢速/正常/快速三档平滑回落
  - **headroom 平滑过渡**：在 up_threshold 附近线性渐变，消除负载临界时的频率振荡
  - **中等负载升频提速**：滞回带内升频速率随 util 接近 up_threshold 线性提升，负载回升时不再以 0.008/tick 缓慢爬升
  - **尖峰抑制**：单 tick 负载跳升超过阈值时衰减其增量，孤立瞬时尖峰（如单核 0↔100%）不再瞬间拉满性能；持续负载下一 tick 即全量生效
  - **降频加速**：极低负载（< down_fast_threshold）跳过降频确认期立即快速回落，消除尖峰消失后频率悬停高位
  - **热重载优先**：模式切换/息屏亮屏仅热重载配置，不再全量重建 sysfs writer
* **[AppDetect]** 同模式 ModeChange 去重：配置重载/亮屏恢复不再产生 `balance -> balance` 冗余事件

### 🐛 修复 (Bug Fixes)
* **[CLG]** 修复配置段不生效：`Mode` 的 `PascalCase` 反序列化与 YAML `cpu_load_governor` 键不匹配，用户配置（含 `enabled`）此前从未生效，现全部正确应用
* **[CLG]** 修复 `perf_floor > perf_ceil` 或 NaN/±Inf 配置导致的 `f32::clamp` panic（配置自动规范化并回退默认值）
* **[CLG]** 修复 `release()` 后不恢复 governor/频率：新增接管前状态快照，退出时按序恢复；读取失败不写退化值、恢复失败自动保留重试
* **[Scheduler]** 修复 IPC 线程 panic 后静默死亡：事件循环包 `catch_unwind`，panic 可见并自动释放 CPU 控制权；config watcher 失败增加退避，消除忙循环
* **[Scheduler]** 修复未知/空模式名意外启用 CLG（现默认禁用，不再用默认参数接管 CPU）
* **[i18n]** 修复非法 language 标签导致启动早期 panic（自动回退 en）
* **[FAS]** 修复配置级 clamp panic：perf floor 超过低 ceil、fast_decay 步长边界反转等场景全部规范化
* **[FAS]** 修复 floor-rescue 自救失效：死锁救援不再被 max_inc 截断，可真正跳出 perf_floor
* **[FAS]** 修复 PID 系数被非法 target_fps（0/负/NaN）污染；per-app 帧率档位过滤非法值

## 📦 v2.0.1

### 🎯 新增 (New Features)
* **[FPS 监控]** eBPF FPS 探针重构为单实例多 PID attach 架构，PID 切换零延迟、零丢帧

### ⚡️ 优化与特性 (Optimizations & Features)
* **[CLG]** CPU 负载调速器升频阻尼优化：
  - 新增 `up_rate_limit_ticks` 升频速率限制（连续 N tick 高负载才升频，默认 2）
  - 跳变阈值从 0.20 提高到 0.35，减少瞬时毛刺响应
  - 小幅 creep 系数 0.05 → 0.02，低负载波动几乎不升频
  - headroom 仅在 util ≥ up_threshold 时生效，低负载不放大
