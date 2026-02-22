#!/system/bin/sh
#
# yumi 模块启动脚本 (service.sh)
#

# 1. 等待系统启动完成
until [ "$(getprop sys.boot_completed)" = "1" ]; do
  sleep 1
done

# 禁用 OPPO/OnePlus/Realme 的 Oiface
if [ "$(getprop persist.sys.oiface.enable)" = "1" ]; then
  setprop persist.sys.oiface.enable 0
  echo "$(date): Oiface disabled." >> "$LOG_FILE"
fi

# 禁用小米的 Joyose 服务
PACKAGE_NAME="com.xiaomi.joyose"
if pm list packages -e | grep -q "$PACKAGE_NAME"; then
  # 先禁用
  pm disable-user "$PACKAGE_NAME" >/dev/null 2>&1
  # 再清除数据，确保彻底
  pm clear "$PACKAGE_NAME" >/dev/null 2>&1
  echo "$(date): Joyose service disabled and data cleared." >> "$LOG_FILE"
fi

# 2. 定义路径
[ -z "$MODDIR" ] && MODDIR=${0%/*}

DAEMON_PATH="$MODDIR/core/bin/yumi"
SCRIPTS_DIR="$MODDIR/scripts"
LOG_DIR="$MODDIR/logs"

# 确保日志目录存在
mkdir -p "$LOG_DIR"

# 3. 清理旧进程
killall -9 yumi > /dev/null 2>&1

# 4. 设置权限
chmod 755 "$DAEMON_PATH"
if [ -d "$SCRIPTS_DIR" ]; then
  chmod -R 755 "$SCRIPTS_DIR"
fi

# 方式 A: 生产模式 (不记录启动日志，节省 I/O)
nohup "$DAEMON_PATH" > /dev/null 2>&1 &

# 方式 B: 调试模式 (如果启动不起来，用这个看报错，输出到 logs/boot_error.log)
# nohup "$DAEMON_PATH" > "$LOG_DIR/boot_error.log" 2>&1 &