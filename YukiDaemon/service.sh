#!/system/bin/sh
#
# YukiDaemon 模块启动脚本 (service.sh)
#

# 1. 等待系统启动完成
until [ "$(getprop sys.boot_completed)" = "1" ]; do
  sleep 1
done

# 2. 定义路径
[ -z "$MODDIR" ] && MODDIR=${0%/*}

DAEMON_PATH="$MODDIR/core/bin/yuki-daemon"
SCRIPTS_DIR="$MODDIR/scripts"
LOG_DIR="$MODDIR/logs"

# 确保日志目录存在
mkdir -p "$LOG_DIR"

# 3. 清理旧进程
killall -9 yuki-daemon > /dev/null 2>&1

# 4. 设置权限
chmod 755 "$DAEMON_PATH"
if [ -d "$SCRIPTS_DIR" ]; then
  chmod -R 755 "$SCRIPTS_DIR"
fi

# 方式 A: 生产模式 (不记录启动日志，节省 I/O)
nohup "$DAEMON_PATH" > /dev/null 2>&1 &

# 方式 B: 调试模式 (如果启动不起来，用这个看报错，输出到 logs/boot_error.log)
# nohup "$DAEMON_PATH" > "$LOG_DIR/boot_error.log" 2>&1 &