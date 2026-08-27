# Ticket 03 — Per-CPU 8路感知 + 用户区间寻优

## 目标 (源自 session 上方的 "personalize" 想法, 但 ticket 02 完成时未拍板)

1. **8路感知** — 把 yumi 从 `per-cluster` 频率策略升级到 `per-cpu` (PerCpuArray 已经支持, 上层没用上).
2. **用户区间寻优** — 让用户能在 WebUI 上指定 min_freq / max_freq (而非当前 hardcode 在 config.yaml 的全局值), 运行中寻优.

## 当前现状 (读代码后的事实)

| 文件 | 现状 |
|---|---|
| `src/monitor/cpu_monitor.rs:50` | 已经是 PerCpuArray (CORE_IDLE_TIME / CORE_BUSY_TIME / CORE_LAST_TIME / CORE_CURRENT_TID), 8路支持已就位 |
| `src/scheduler/cpu_load_governor.rs:42-49` | ClusterState 是 per-cluster (`policy_id` + `affected_cpus`), 把多个 cpu 当作一个 cluster 整体调频 |
| `src/scheduler/cpu_load_governor.rs:max_writer / min_writer` | 写 sysfs 的 fastwriter 是 per-cluster 的, 不区分 cluster 内不同 cpu |
| `src/scheduler/config.rs` | 全局 CpuLoadGovernorConfig, 没有 min_freq / max_freq 用户区间配置 |

## 方案 (待 grill-me 验证, 这里仅 spec)

### 3.1 Per-CPU 调度
- 把 ClusterState 拆开: 每 cpu 一个 CpuState.
- BPF 已能给出 per-cpu 的 idle / busy 时间, 上层 scheduler 不需要新 BPF 改动.
- 改 `cpu_load_governor.rs`:
  - 取消 cluster 抽象, 改为直接 per-policy (因为 cluster 内部的 min/max 通常一致, 但可让用户在 config 里 override)
  - 或者保留 cluster 但每 cluster 内分 core-level PID + jank detection
- PerCpuFastWriter: 写 sysfs 时每个 cpu id 一个 `scaling_min_freq` / `scaling_max_freq` writer, 串行写避免文件 io 阻塞

### 3.2 用户区间寻优 (WebUI 端)
- 新加 `/api/range` endpoint, 接受 min/max 区间 (kHz)
- 新加 `/api/optimize` endpoint, 触发后台寻优 (先 sweep 出 P-state 边界, 再二分最佳 perf/power 拐点)
- WebUI 新页面: "区间设置" + "寻优面板" (进度条 / 实时 perf/watt / 当前 freq)
- 后端 daemon 接受参数后存到 CpuLoadGovernorConfig, 重启时 reload
- **注意**: 关核 (ticket 04) 后, 寻优需要避开被关的核心

### 3.3 配置文件格式
- `config/config.yaml` 加 `cpus:` 列表, 每个 cpu 可以单独指定 min/max:
  ```yaml
  cpus:
    - id: 0-3   # cluster 0
      min_freq: 1800000   # kHz
      max_freq: 2800000
    - id: 4-7   # cluster 1
      min_freq: 2200000
      max_freq: 3200000
  ```

## 风险 / 边界

1. **per-cpu 写 sysfs 在 8 核手机上不慢, 但 16 核 (桌面) 可能成瓶颈** — 加 batch writer.
2. **写 min_freq 提升瞬时功耗** — 需 warning 提示 (区间上限 > idle 时 GPU 抖动).
3. **寻优算法** — 简单二分足够; ML/启发式过头.
4. **rollback** — 必须保留原 ClusterState 模式作为 fallback, config 中 `mode: per-cluster | per-cpu`.

## 测试 seam (RED → GREEN 顺序)

1. **RED**: 写 `tests/cpu_load_governor_percpu.rs` — mock BPF map (现成 fixture from aya), 注入 8 核 idle, 断言 freq 写到每个 cpu 各自的 sysfs 路径.
2. **GREEN**: 改 cpu_load_governor.rs, 让 ClusterState 内部存 Vec<CpuState> + Vec<FastWriter>.
3. **REFACTOR**: 抽 `PerCpuGovernor` trait, 让 per-cluster 模式通过新结构 back-compat.

## 决策点 (grill-me 必问)

1. **per-cluster vs per-cpu 边界** — 同一个 cluster 里 4 个 cpu 是否要一样? 不同应用 hot cpu 0-1, 其他可以更低?
2. **寻优算法** — 二分 vs 爬山 vs 让用户自己手动调?
3. **寻优频率** — 每次启动跑一次 vs 周期性 (热环境会变)?
4. **WebUI 形态** — 单独页面 vs 主页面加 tab?

## 当前状态
- ✅ ticket-02 baseline build 通 (zip 在 output/)
- ✅ code survey 完成
- ❌ 未开始改源码 — 等用户拍板 4 个决策点
- ❌ 未开始写测试 — 等决策