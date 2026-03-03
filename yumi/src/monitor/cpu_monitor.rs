use aya::{Ebpf, include_bytes_aligned, programs::TracePoint};
use aya::maps::PerCpuArray;
use aya::maps::HashMap as BpfHashMap;
use aya::util::online_cpus;
use std::sync::mpsc::Sender;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use crate::common::DaemonEvent;
use crate::monitor::app_detect;
use log::{info, warn, debug};
use std::time::Instant;

/// 从 /proc/{pid}/task/ 读取前台进程的所有线程 TID
fn get_thread_tids(pid: u32) -> Vec<u32> {
    let task_dir = format!("/proc/{}/task", pid);
    let mut tids = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&task_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(tid) = name.parse::<u32>() {
                    tids.push(tid);
                }
            }
        }
    }
    tids
}

pub async fn start_cpu_loop(tx: Sender<DaemonEvent>) -> Result<(), anyhow::Error> {
    static BPF_DATA: &[u8] = include_bytes_aligned!(env!("BPF_CPU_OBJ_PATH"));
    
    // Box::leak 将 Ebpf 提升为 'static 生命周期，
    // 使得从中借出的 Map 引用可以安全地 move 进 tokio::spawn
    // （与 fps_monitor.rs 中的做法一致）
    let bpf = Box::leak(Box::new(Ebpf::load(BPF_DATA)?));
    let program: &mut TracePoint = bpf.program_mut("handle_sched_switch").unwrap().try_into()?;
    program.load()?;
    program.attach("sched", "sched_switch")?;
    info!("eBPF System Load monitor started.");

    let cpus = online_cpus().map_err(|e| anyhow::anyhow!("Failed to get online CPUs: {:?}", e))?;
    let num_cpus = cpus.len();
    // 防御：Rust 侧不超过合理上限
    let num_cpus = num_cpus.min(16);
    info!("Detected {} online CPU cores for monitoring.", num_cpus);

    // 先取裸指针，再通过 unsafe 分别获取各个 map
    // 这样避免对 bpf 产生多次 &mut 借用冲突
    // 安全性：每个 map 是独立的内核对象，内存不重叠，且后续不再直接访问 bpf 本体
    let bpf_ptr = bpf as *mut Ebpf;

    // core_idle_time: BPF_MAP_TYPE_PERCPU_ARRAY
    let core_idle_map: PerCpuArray<_, u64> = PerCpuArray::try_from(
        unsafe { &mut *bpf_ptr }.map_mut("core_idle_time").unwrap()
    )?;

    // core_busy_time: BPF_MAP_TYPE_PERCPU_ARRAY
    // 与 idle 对称，用于区分"真正空闲"和"深度休眠"
    let core_busy_map: PerCpuArray<_, u64> = PerCpuArray::try_from(
        unsafe { &mut *bpf_ptr }.map_mut("core_busy_time").unwrap()
    )?;

    let thread_run_map: BpfHashMap<_, u32, u64> = BpfHashMap::try_from(
        unsafe { &mut *bpf_ptr }.map_mut("thread_run_time").unwrap()
    )?;

    // 追踪前台 PID，用于查询其线程的运行时间
    let shared_pid = Arc::new(AtomicU32::new(app_detect::get_current_pid() as u32));
    let pid_arc = shared_pid.clone();

    // 独立轻量任务：定期同步前台 PID
    tokio::spawn(async move {
        let mut last_pid: u32 = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let current_pid = app_detect::get_current_pid() as u32;
            if current_pid != last_pid && current_pid > 0 {
                pid_arc.store(current_pid, Ordering::Relaxed);
                debug!("CPU monitor: foreground PID updated {} \u{2192} {}", last_pid, current_pid);
                last_pid = current_pid;
            }
        }
    });

    tokio::spawn(async move {
        // 全局核心历史数据
        let mut last_idle_times = vec![0u64; num_cpus];
        // busy 时间历史快照
        let mut last_busy_times = vec![0u64; num_cpus];
        let mut last_check_time = Instant::now();
        
        // 前台线程历史数据：TID -> 上次采样的累计运行时间
        let mut last_thread_run: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();

        // debug 日志计数器，每 25 个 tick (~5秒) 输出一次
        let mut log_counter: u32 = 0;

        // 轮询周期：200ms
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        
        loop {
            interval.tick().await;
            let now = Instant::now();
            let real_delta_ns = now.duration_since(last_check_time).as_nanos() as u64;
            last_check_time = now;

            if real_delta_ns == 0 { continue; }

            let mut core_utils = Vec::with_capacity(num_cpus);

            // 1. 从 PERCPU_ARRAY 读取所有 CPU 的空闲时间和忙碌时间
            let zero_key: u32 = 0;
            let per_cpu_idle_values = core_idle_map.get(&zero_key, 0);
            let per_cpu_busy_values = core_busy_map.get(&zero_key, 0);

            for cpu_id in 0..num_cpus {
                let current_idle = per_cpu_idle_values.as_ref()
                    .ok()
                    .and_then(|vals| vals.get(cpu_id).copied())
                    .unwrap_or(0);

                let current_busy = per_cpu_busy_values.as_ref()
                    .ok()
                    .and_then(|vals| vals.get(cpu_id).copied())
                    .unwrap_or(0);

                let last_idle = last_idle_times[cpu_id];
                let last_busy = last_busy_times[cpu_id];

                // 用 busy / (busy + idle) 代替 (wall - idle) / wall
                // 这样当核心处于深度休眠时，busy_delta 和 idle_delta 都为 0，
                // observed_total == 0，利用率正确地返回 0% 而不是 100%
                let idle_delta = current_idle.saturating_sub(last_idle);
                let busy_delta = current_busy.saturating_sub(last_busy);
                let observed_total = idle_delta + busy_delta;

                let util = if observed_total > 0 {
                    // 有 sched_switch 事件覆盖的时间段，用直接比值
                    (busy_delta as f32 / observed_total as f32).clamp(0.0, 1.0)
                } else {
                    // idle 和 busy 都没增长 -> 核心在深度休眠 (C3/power collapse) -> 0%
                    0.0
                };

                core_utils.push(util);
                last_idle_times[cpu_id] = current_idle;
                last_busy_times[cpu_id] = current_busy;
            }

            // 2. 计算前台应用最重线程的利用率
            let foreground_max_util = {
                let fg_pid = shared_pid.load(Ordering::Relaxed);
                if fg_pid == 0 {
                    0.0_f32
                } else {
                    let tids = get_thread_tids(fg_pid);
                    let mut max_util: f32 = 0.0;
                    let mut current_thread_run = std::collections::HashMap::with_capacity(tids.len());

                    for tid in &tids {
                        let current_run = thread_run_map.get(tid, 0).unwrap_or(0);
                        current_thread_run.insert(*tid, current_run);

                        if let Some(&last_run) = last_thread_run.get(tid) {
                            if current_run >= last_run {
                                let thread_delta = current_run - last_run;
                                let util = (thread_delta as f32 / real_delta_ns as f32).clamp(0.0, 1.0);
                                if util > max_util {
                                    max_util = util;
                                }
                            }
                        }
                        // 如果是新出现的线程（没有 last_run），这个周期跳过，下一个周期才有基线
                    }
                    
                    // 替换为本轮的快照，用于下次 diff
                    last_thread_run = current_thread_run;
                    max_util
                }
            };

            // 3. 周期性 debug 日志（每 25 个 tick ~5秒）
            log_counter += 1;
            if log_counter % 25 == 0 {
                debug!("CPU monitor: cores=[{}] fg_pid={} fg_max_util={:.1}% threads_tracked={} delta={}ms",
                    core_utils.iter()
                        .map(|u| format!("{:.0}%", u * 100.0))
                        .collect::<Vec<_>>()
                        .join(", "),
                    shared_pid.load(Ordering::Relaxed),
                    foreground_max_util * 100.0,
                    last_thread_run.len(),
                    real_delta_ns / 1_000_000);
            }

            // 4. 将全局状态发给调度器
            if tx.send(DaemonEvent::SystemLoadUpdate {
                core_utils,
                foreground_max_util,
            }).is_err() {
                warn!("CPU monitor: channel closed, exiting loop.");
                break;
            }
        }
    });

    std::future::pending::<()>().await;
    Ok(())
}