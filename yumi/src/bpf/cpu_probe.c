#include "bpf_abi.h"

struct sched_switch_args {
    unsigned long long pad;
    char prev_comm[16];
    int prev_pid;
    int prev_prio;
    long long prev_state;
    char next_comm[16];
    int next_pid;
    int next_prio;
};

// Map 1: 记录每个核心最后一次切换的时间戳
struct bpf_map_def SEC("maps") core_last_time = {
    .type = BPF_MAP_TYPE_PERCPU_ARRAY, // PERCPU 数组，极其高效
    .key_size = sizeof(__u32),
    .value_size = sizeof(__u64),
    .max_entries = 1,
};

// Map 2: 记录每个核心累计的 Idle (空闲) 时间 (纳秒)
// PERCPU_ARRAY：每个 CPU 有独立的副本，消除跨核读取的缓存一致性问题
struct bpf_map_def SEC("maps") core_idle_time = {
    .type = BPF_MAP_TYPE_PERCPU_ARRAY,
    .key_size = sizeof(__u32),   // 固定 key=0
    .value_size = sizeof(__u64), // 累计空闲时间
    .max_entries = 1,            // 只需 1 个 entry，每个 CPU 自动有独立副本
};

// Map 3: 记录每个核心累计的 Busy (忙碌) 时间 (纳秒)
// 与 idle 对称，用于解决深度休眠核心被误判为 100% 利用率的问题
// 当 busy_delta 和 idle_delta 在一个采样窗口内均为 0 时，
// 说明该核心处于深度休眠 (C3/power collapse)，利用率应为 0% 而非 100%
struct bpf_map_def SEC("maps") core_busy_time = {
    .type = BPF_MAP_TYPE_PERCPU_ARRAY,
    .key_size = sizeof(__u32),   // 固定 key=0
    .value_size = sizeof(__u64), // 累计忙碌时间
    .max_entries = 1,
};

// Map 4: 依然保留线程的运行时间（供游戏 FAS 读取特定线程用）
struct bpf_map_def SEC("maps") thread_run_time = {
    .type = BPF_MAP_TYPE_HASH,
    .key_size = sizeof(__u32),
    .value_size = sizeof(__u64),
    .max_entries = 8192,
};

SEC("tracepoint/sched/sched_switch")
int handle_sched_switch(struct sched_switch_args *ctx) {
    __u64 now = bpf_ktime_get_ns();
    __u32 zero_key = 0;

    __u32 prev_tid = ctx->prev_pid;
    __u32 next_tid = ctx->next_pid;

    // --- 1. 全局单核利用率统计 ---
    __u64 *last_ts = bpf_map_lookup_elem(&core_last_time, &zero_key);
    if (last_ts) {
        __u64 delta = now - *last_ts;
        if (delta > 0 && delta < 10000000000ULL) {
            if (prev_tid == 0) {
                // 刚被剥夺执行权的是 PID 0 (Idle 进程)，说明这段时间 CPU 是空闲的
                // 累加到 idle 计数器
                __u64 *idle_total = bpf_map_lookup_elem(&core_idle_time, &zero_key);
                if (idle_total) {
                    __u64 new_idle = *idle_total + delta;
                    bpf_map_update_elem(&core_idle_time, &zero_key, &new_idle, BPF_ANY);
                } else {
                    bpf_map_update_elem(&core_idle_time, &zero_key, &delta, BPF_ANY);
                }
            } else {
                // 不是 PID 0，说明这段时间是某个真实线程在跑
                // [新增] 累加到 per-CPU busy 计数器
                __u64 *busy_total = bpf_map_lookup_elem(&core_busy_time, &zero_key);
                if (busy_total) {
                    __u64 new_busy = *busy_total + delta;
                    bpf_map_update_elem(&core_busy_time, &zero_key, &new_busy, BPF_ANY);
                } else {
                    bpf_map_update_elem(&core_busy_time, &zero_key, &delta, BPF_ANY);
                }

                // 线程级 Map：供 FAS 查询特定线程运行时间
                __u64 *thread_total = bpf_map_lookup_elem(&thread_run_time, &prev_tid);
                if (thread_total) {
                    __u64 new_total = *thread_total + delta;
                    bpf_map_update_elem(&thread_run_time, &prev_tid, &new_total, BPF_ANY);
                } else {
                    bpf_map_update_elem(&thread_run_time, &prev_tid, &delta, BPF_ANY);
                }
            }
        }
    }
    
    // 更新当前核心的最后切换时间
    bpf_map_update_elem(&core_last_time, &zero_key, &now, BPF_ANY);

    return 0;
}

char _license[] SEC("license") = "GPL";