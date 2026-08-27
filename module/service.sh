#!/system/bin/sh
#
# 核心领航员 (core-pilot) Magisk 模块启动脚本 (service.sh)
#

# 1. 等待系统启动完成
until [ "$(getprop sys.boot_completed)" = "1" ]; do
  sleep 1
done

# 2. 定义路径
[ -z "$MODDIR" ] && MODDIR=${0%/*}

DAEMON_PATH="$MODDIR/core/bin/yumi"
SCRIPTS_DIR="$MODDIR/scripts"
LOG_DIR="$MODDIR/logs"
LOG_FILE="$LOG_DIR/service.log"

# 确保日志目录存在
mkdir -p "$LOG_DIR"

# 任务 #6 reliability: daemon 启动前先恢复默认值, 保证 core-pilot 接管时环境干净
if [ -x "$SCRIPTS_DIR/restore_defaults.sh" ]; then
  echo "$(date '+%Y-%m-%d %H:%M:%S'): pre-start restore_defaults" >> "$LOG_FILE"
  sh "$SCRIPTS_DIR/restore_defaults.sh" "pre_start" >> "$LOG_FILE" 2>&1
else
  echo "$(date '+%Y-%m-%d %H:%M:%S'): restore_defaults.sh missing, skip" >> "$LOG_FILE"
fi

# 3. 清理旧进程
killall -9 yumi > /dev/null 2>&1

# 4. 设置权限
chmod 755 "$DAEMON_PATH"
if [ -d "$SCRIPTS_DIR" ]; then
  chmod -R 755 "$SCRIPTS_DIR"
fi

# 5. 启动核心领航员守护进程
# 方式 A: 生产模式 (不记录启动日志, 节省 I/O)
nohup "$DAEMON_PATH" > /dev/null 2>&1 &

# 方式 B: 调试模式 (如果启动不起来, 用这个看错误, 输出到 logs/boot_error.log)
# nohup "$DAEMON_PATH" > "$LOG_DIR/boot_error.log" 2>&1 &
