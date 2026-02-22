#include "bpf_abi.h"

// 定义 Map：记录上一帧时间
struct bpf_map_def SEC("maps") last_timestamps = {
    .type = BPF_MAP_TYPE_HASH,
    .key_size = sizeof(__u32),   // PID
    .value_size = sizeof(__u64), // 时间戳 ns
    .max_entries = 1024,
};

// 定义 Map：输出帧间隔数据
struct bpf_map_def SEC("maps") frame_events = {
    .type = BPF_MAP_TYPE_PERF_EVENT_ARRAY,
    .key_size = sizeof(__u32),
    .value_size = sizeof(__u32), 
    .max_entries = 32,
};

SEC("uprobe/queueBuffer")
int handle_frame(void *ctx) {
    // 1. 获取上下文信息
    __u64 pid_tgid = bpf_get_current_pid_tgid();
    __u32 pid = (__u32)(pid_tgid >> 32);
    __u64 now = bpf_ktime_get_ns();

    // 2. 查找该 PID 上一次的记录
    __u64 *prev_ts = bpf_map_lookup_elem(&last_timestamps, &pid);
    
    if (prev_ts) {
        // 3. 计算帧间隔 (Delta)
        __u64 delta = now - *prev_ts;
        
        // 4. 如果间隔合理（防止溢出或极其离谱的数据），发送到用户态
        // 0xffffffffULL 代表当前 CPU
        bpf_perf_event_output(ctx, &frame_events, 0xffffffffULL, &delta, sizeof(delta));
    }

    // 5. 更新时间戳
    // 0 代表 BPF_ANY
    bpf_map_update_elem(&last_timestamps, &pid, &now, 0);
    
    return 0;
}

// 必须包含 License，否则内核拒绝加载
char _license[] SEC("license") = "GPL";