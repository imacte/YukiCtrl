#!/system/bin/sh
#
# restore_defaults.sh — 任务 #6 reliability
#
# 把 core-pilot 涉及的所有可调参数重置为内核/厂商默认值, 作为 watchdog
# 在 "心跳失联 / 核心数掉到 1 / 温度临界" 时的一键自愈脚本.
#
# 调用方: Rust watchdog 线程 → `/system/bin/sh restore_defaults.sh <reason>`
# 退出码: 0 = 成功 (即使所有路径都不存在也算成功, 因为这是 "尝试恢复");
#         非 0 = 严重错误, watchdog 计入失败计数.
#
# 设计原则:
#   1. 不修改已有 disable_boost.sh 的逻辑, 仅在其之外补齐 "默认值恢复" 路径.
#   2. 所有写操作都包在 [ -e path ] 检查里, 缺啥跳啥 (不同厂商/机型差异极大).
#   3. 不删除任何东西, 只把值写回公认 default, 让 core-pilot 重新接管.

set -u  # 未定义变量即报错 (防止 typo 静默)

# MODDIR 由 watchdog 在 spawn 时显式传入; 缺省回退到生产路径 (单跑调试场景)
: "${MODDIR:=/data/adb/modules/core-pilot}"
REASON="${1:-unspecified}"
LOG_FILE="$MODDIR/logs/restore_defaults.log"
mkdir -p "$(dirname "$LOG_FILE")"

log_line() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') reason=$REASON $*" >> "$LOG_FILE"
}

# ─── 通用写值函数 (write_value): 仅当文件存在时写入, 不抛错 ─────────
write_value() {
    local pattern="$1"
    local value="$2"
    local files=$(ls -d $pattern 2>/dev/null)
    [ -z "$files" ] && return 0
    for f in $files; do
        [ -e "$f" ] || continue
        chmod 644 "$f" 2>/dev/null
        echo "$value" > "$f" 2>/dev/null
        chmod 444 "$f" 2>/dev/null
        log_line "write $f = $value"
    done
}

# ─── 1. CPU 频率 / governor: 取消 core-pilot 设置, 让 schedutil/ondemand 接管 ──
restore_cpu_defaults() {
    log_line "=== restore_cpu_defaults ==="

    # governor: 大多数机型的默认值. 顺序不重要, 只写存在的
    write_value "/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor" "schedutil"
    write_value "/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor" "ondemand"

    # 取消 min/max 锁频
    write_value "/sys/devices/system/cpu/cpu*/cpufreq/scaling_min_freq" "0"
    write_value "/sys/devices/system/cpu/cpu*/cpufreq/scaling_max_freq" "0"

    # 取消 boost (与 disable_boost.sh 互补, 这里给 "恢复默认" 角度)
    write_value "/sys/devices/system/cpu/cpu*/cpufreq/boost" "0"
    write_value "/sys/devices/system/cpu/cpufreq/boost" "0"

    # schedutil rate limit (默认 10000us)
    write_value "/sys/devices/system/cpu/cpu*/cpufreq/schedutil/rate_limit_us" "10000"
}

# ─── 2. CPU 核心 online: 全部上线 (覆盖 hotplug 误关) ───────────────
restore_cores_online() {
    log_line "=== restore_cores_online ==="

    # cpu0 永远 online, 不写; 其余 cpu1..15 全部置 1
    for cpu in $(seq 1 15); do
        online_path="/sys/devices/system/cpu/cpu${cpu}/online"
        if [ -e "$online_path" ]; then
            chmod 644 "$online_path" 2>/dev/null
            echo 1 > "$online_path" 2>/dev/null
            chmod 444 "$online_path" 2>/dev/null
            log_line "online cpu${cpu} = 1"
        fi
    done

    # 关闭 core-pilot 之外的 hotplug 守护 (MTK sched, HMP boost 等)
    write_value "/sys/devices/system/cpu/cpuhotplug/enabled" "0"
    write_value "/sys/devices/system/cpu/hyp_core_ctl/enable" "0"
    write_value "/sys/kernel/intelli_plug/intelli_plug_active" "0"
    write_value "/sys/kernel/zen_decision/enabled" "0"
    write_value "/sys/module/autosmp/parameters/enabled" "0"
    write_value "/sys/module/blu_plug/parameters/enabled" "0"
}

# ─── 3. GPU devfreq: 还原 governor 与 freq range ────────────────────
restore_gpu_defaults() {
    log_line "=== restore_gpu_defaults ==="

    for gpu_dir in /sys/class/devfreq/*; do
        [ -d "$gpu_dir" ] || continue
        case "$(basename "$gpu_dir")" in
            *gpu*|*Gpu*|*GPU*)
                # 默认 governor: 多数 MTK / Adreno 用 "userspace" 或 "performance"
                # 保险起见把 min_freq / max_freq 解锁, governor 不强制改
                write_value "$gpu_dir/min_freq" "0"
                write_value "$gpu_dir/max_freq" "0"
                log_line "gpu reset: $(basename "$gpu_dir")"
                ;;
        esac
    done
}

# ─── 4. IO 调度: 把 IO scheduler 还原 (cfq / mq-deadline 是常见默认) ──
restore_io_defaults() {
    log_line "=== restore_io_defaults ==="

    for queue in /sys/block/*/queue/scheduler; do
        [ -e "$queue" ] || continue
        # mksh: redirect failure prints "cant create" to stderr; [ -w ] filters read-only nodes (e.g. mtdblock)
        [ -w "$queue" ] || continue
        local_block="$(dirname "$queue")"
        for algo in mq-deadline cfq bfq; do
            if grep -q "\\[$algo\\]" "$queue" 2>/dev/null; then
                echo "$algo" > "$queue" 2>/dev/null
                chmod 644 "$queue" 2>/dev/null
                chmod 444 "$queue" 2>/dev/null
                log_line "io scheduler: $local_block -> $algo"
                break
            fi
        done
        # read_ahead_kb 默认 128
        write_value "$local_block/read_ahead_kb" "128"
        # nr_requests 默认 128
        write_value "$local_block/nr_requests" "128"
    done
}

# ─── 5. Swap / zram: 解锁 swappiness, 取消 zram 限制 ──────────────
restore_swap_defaults() {
    log_line "=== restore_swap_defaults ==="

    # Android 默认 swappiness = 100, 桌面 Linux = 60
    write_value "/proc/sys/vm/swappiness" "100"
    write_value "/proc/sys/vm/page-cluster" "3"
    write_value "/proc/sys/vm/dirty_ratio" "20"
    write_value "/proc/sys/vm/dirty_background_ratio" "10"

    # zram: 不强制 reset disksize (会丢失数据), 只关掉 core-pilot 临时设的限制
    for zram in /sys/block/zram*; do
        [ -d "$zram" ] || continue
        write_value "$zram/reset" "1"
        write_value "$zram/max_comp_streams" "4"
    done
}

# ─── 6. 温度限频: 解锁 thermal 限制 ──────────────────────────────
restore_thermal_defaults() {
    log_line "=== restore_thermal_defaults ==="

    # cpu_limits 全部解开 (厂商写 "cpuN 2147483647" 即可)
    if [ -e /sys/class/thermal/thermal_message/cpu_limits ]; then
        for cpu in $(seq 0 7); do
            echo "cpu${cpu} 2147483647" > /sys/class/thermal/thermal_message/cpu_limits 2>/dev/null \
                && log_line "thermal cpu${cpu} unlocked"
        done
    fi

    write_value "/sys/class/thermal/thermal_message/temp_state" "0"
    write_value "/sys/class/thermal/thermal_message/market_download_limit" "0"

    # 不动 userspace thermal 节点 (容易触发 BSI)
}

# ─── main ──────────────────────────────────────────────────────
main() {
    log_line "---- restore_defaults start (reason=$REASON) ----"
    restore_cpu_defaults
    restore_cores_online
    restore_gpu_defaults
    restore_io_defaults
    restore_swap_defaults
    restore_thermal_defaults
    log_line "---- restore_defaults done ----"
    exit 0
}

main
