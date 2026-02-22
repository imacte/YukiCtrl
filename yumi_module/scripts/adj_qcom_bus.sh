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

adj_qcom_bus_dcvs() {
    echo "正在应用高通 DDR/L3/LLCC 总线频率..."

    # DDR Frequencies
    write_value "/sys/devices/system/cpu/bus_dcvs/DDR/soc:qcom,memlat:ddr:silver/max_freq" "1555000"
    write_value "/sys/devices/system/cpu/bus_dcvs/DDR/19091000.qcom,bwmon-ddr/max_freq" "2736000"
    write_value "/sys/devices/system/cpu/bus_dcvs/DDR/soc:qcom,memlat:ddr:prime/max_freq" "3196000"
    write_value "/sys/devices/system/cpu/bus_dcvs/DDR/soc:qcom,memlat:ddr:prime-latfloor/max_freq" "3196000"
    write_value "/sys/devices/system/cpu/bus_dcvs/DDR/soc:qcom,memlat:ddr:gold-compute/max_freq" "1555000"
    write_value "/sys/devices/system/cpu/bus_dcvs/DDR/soc:qcom,memlat:ddr:gold/max_freq" "3196000"

    # L3 Cache Frequencies
    write_value "/sys/devices/system/cpu/bus_dcvs/L3/soc:qcom,memlat:l3:silver/max_freq" "1708800"
    write_value "/sys/devices/system/cpu/bus_dcvs/L3/soc:qcom,memlat:l3:prime/max_freq" "1708800"
    write_value "/sys/devices/system/cpu/bus_dcvs/L3/soc:qcom,memlat:l3:gold/max_freq" "1708800"
    write_value "/sys/devices/system/cpu/bus_dcvs/L3/soc:qcom,memlat:l3:prime-compute/max_freq" "1708800"

    # DDRQOS Frequencies
    write_value "/sys/devices/system/cpu/bus_dcvs/DDRQOS/soc:qcom,memlat:ddrqos:gold/max_freq" "1"
    write_value "/sys/devices/system/cpu/bus_dcvs/DDRQOS/soc:qcom,memlat:ddrqos:prime-latfloor/max_freq" "1"

    # LLCC (Last Level Cache Controller) Frequencies
    write_value "/sys/devices/system/cpu/bus_dcvs/LLCC/soc:qcom,memlat:llcc:gold-compute/max_freq" "600000"
    write_value "/sys/devices/system/cpu/bus_dcvs/LLCC/190b6400.qcom,bwmon-llcc/max_freq" "806000"
    write_value "/sys/devices/system/cpu/bus_dcvs/LLCC/soc:qcom,memlat:llcc:silver/max_freq" "600000"
    write_value "/sys/devices/system/cpu/bus_dcvs/LLCC/soc:qcom,memlat:llcc:gold/max_freq" "1066000"
}

main() {
    echo "--- 开始应用高通总线频率硬编码值 ---"
    adj_qcom_bus_dcvs
    echo "--- 高通总线频率应用完毕 ---"
}

main