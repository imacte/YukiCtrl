#!/system/bin/sh

write_value() {
    local file_path="$1"
    local value="$2"
    
    if [ -e "$file_path" ]; then
        chmod 644 "$file_path" 2>/dev/null
        echo "$value" > "$file_path"
        chmod 444 "$file_path" 2>/dev/null
    fi
}

adj_walt_params() {
    if [ -d "/proc/sys/walt" ]; then
        echo "正在应用WALT调度器参数..."
        write_value "/sys/kernel/msm_performance/parameters/cpu_min_freq" "0:0 1:0 2:0 3:0 4:0 5:0 6:0 7:0"
        write_value "/sys/kernel/msm_performance/parameters/cpu_max_freq" "0:9999999 1:9999999 2:9999999 3:9999999 4:9999999 5:9999999 6:9999999 7:9999999"
        write_value "/proc/sys/walt/sched_busy_hyst_ns" "0"
        write_value "/proc/sys/walt/sched_group_upmigrate" "100"
        write_value "/proc/sys/walt/sched_asymcap_boost" "1"
        write_value "/proc/sys/walt/sched_force_lb_enable" "1"
        write_value "/proc/sys/walt/sched_boost" "0"
    else
        echo "WALT调度器路径 /proc/sys/walt 不存在，跳过参数应用。"
    fi
}

main() {
    echo "--- 开始调整WALT参数 ---"
    adj_walt_params
    echo "--- WALT参数调整完毕 ---"
}

main