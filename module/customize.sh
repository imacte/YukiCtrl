#!/system/bin/sh
#
# ########################################################################################
#   核心领航员 (core-pilot) 模块安装脚本
#   作者: yuki
# ########################################################################################

# --- 模块路径和工具 ---
# $MODPATH 是 Magisk 传入的模块安装路径

# --- 自动检测 BusyBox (保留以备将来可能使用，当前未使用) ---
if [ -x "/data/adb/magisk/busybox" ]; then
  BUSYBOX="/data/adb/magisk/busybox"
elif [ -x "/data/adb/ksu/bin/busybox" ]; then
  BUSYBOX="/data/adb/ksu/bin/busybox"
elif [ -x "/data/adb/ap/bin/busybox" ]; then
  BUSYBOX="/data/adb/ap/bin/busybox"
fi

# --- 语言定义 ---
CURRENT_LOCALE=$(/system/bin/getprop persist.sys.locale)
if [ -z "$CURRENT_LOCALE" ]; then
    CURRENT_LOCALE=$(/system/bin/getprop ro.product.locale)
fi

LANG_CODE="en"
MSG_WELCOME="Welcome to Core Pilot!"

if echo "$CURRENT_LOCALE" | $BUSYBOX grep -qi "zh"; then
  LANG_CODE="zh"
  MSG_WELCOME="欢迎使用 核心领航员！"
fi

# --- 仅输出欢迎信息 ---
ui_print " "
ui_print "$MSG_WELCOME"
ui_print " "

# --- 结束 ---
# 保留模块默认配置不变，不进行任何文件操作
# --- Phase 2 / ticket-04 D7: KSU allowlist hint ---
ui_print " "
ui_print "[HOTPLUG-D7] needs KSU allowlist:"
ui_print "    KSU Manager - Superuser - core-pilot - module settings - allow:"
ui_print "      /sys/devices/system/cpu/cpu*/online"
ui_print "    Without this hotplug will log D7-SELinux-KSU-deny."
ui_print " "
