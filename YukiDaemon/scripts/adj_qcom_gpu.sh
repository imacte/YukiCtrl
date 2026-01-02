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

disable_qcom_gpu_boost() {
    if [ ! -d "/sys/class/kgsl/kgsl-3d0" ]; then
        echo "高通GPU路径 /sys/class/kgsl/kgsl-3d0 不存在，跳过操作。"
        return
    fi

    echo "正在禁用高通GPU Boost..."

    if [ -e "/sys/class/kgsl/kgsl-3d0/num_pwrlevels" ]; then
        num_pwrlevels=$(cat /sys/class/kgsl/kgsl-3d0/num_pwrlevels)
        if [ -n "$num_pwrlevels" ] && [ "$num_pwrlevels" -gt 0 ]; then
            min_pwrlevel=$((num_pwrlevels - 1))
            write_value "/sys/class/kgsl/kgsl-3d0/default_pwrlevel" "$min_pwrlevel"
            write_value "/sys/class/kgsl/kgsl-3d0/min_pwrlevel" "$min_pwrlevel"
        fi
    fi
    
    write_value "/sys/class/kgsl/kgsl-3d0/max_pwrlevel" "0"
    write_value "/sys/class/kgsl/kgsl-3d0/thermal_pwrlevel" "0"
    write_value "/sys/class/kgsl/kgsl-3d0/throttling" "0"
    write_value "/sys/class/kgsl/kgsl-3d0/force_bus_on" "0"
    write_value "/sys/class/kgsl/kgsl-3d0/force_clk_on" "0"
    write_value "/sys/class/kgsl/kgsl-3d0/force_no_nap" "0"
    write_value "/sys/class/kgsl/kgsl-3d0/force_rail_on" "0"
    write_value "/sys/class/kgsl/kgsl-3d0/max_clock_mhz" "999"
    write_value "/sys/class/kgsl/kgsl-3d0/max_gpuclk" "999000000"
    write_value "/sys/class/kgsl/kgsl-3d0/min_clock_mhz" "0"
    write_value "/sys/class/kgsl/kgsl-3d0/devfreq/min_freq" "0"
    write_value "/sys/class/kgsl/kgsl-3d0/devfreq/max_freq" "999000000"
}

main() {
    echo "--- 开始禁用高通GPU Boost ---"
    disable_qcom_gpu_boost
    echo "--- 高通GPU Boost禁用完毕 ---"
}

main