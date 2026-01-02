# --- Main & Monitor ---
yuki-daemon-starting = Yuki Daemon 统一启动中...
scheduler-module-started = 调度器模块已启动
scheduler-module-start-failed = 启动调度器模块失败: { $error }
monitor-module-crashed = 监控模块崩溃: { $error }
monitor-module-started = 监控模块已启动
monitor-starting = 正在启动 yuki-monitor 模块...

# Boot
boot-scripts-running = [Boot] 正在运行启动脚本...
boot-script-applying = [Boot] 正在应用脚本: { $path }
boot-script-success = [Boot] 脚本 { $name } 应用成功
boot-script-failed = [Boot] 脚本 { $name } 失败: { $error }
boot-script-exec-failed = [Boot] 执行脚本 { $name } 失败: { $error }
boot-scripts-finished = [Boot] 启动脚本执行完成

# Power
power-cpu-temp-found = [Power] 成功找到 CPU 温度传感器: { $path }
power-cpu-temp-not-found = [Power] 无法找到 CPU 温度路径: { $error }. 将使用 0.0 作为温度
power-loop-started = [Power] 功耗监控循环已启动
power-screen-off-skip = [Power] 屏幕关闭，跳过功耗轮询
power-charging-stopped = [Power] 充电停止。检查会话限制...
power-trim-failed = [Power] 清理旧会话失败: { $error }
power-new-session = [Power] 开始新会话: { $id }
power-db-write-failed = [Power] 写入功耗日志到数据库失败: { $error }
power-read-failed = [Power] 读取电压或电流失败: { $error }
power-status-read-failed = [Power] 读取充电状态失败: { $error }

# DB
db-initialized = [DB] 数据库初始化于 { $path }
db-logged-raw = [DB] 记录原始数据: { $vol }uV, { $cur }uA ({ $pkg })
db-session-limit-exceeded = [DB] 会话数量 ({ $count }) 超过限制 ({ $limit })。正在清理 { $trim } 个旧会话...
db-trimmed-entries = [DB] 已清理 { $rows } 条日志 (来自 { $sessions } 个旧会话)
db-session-limit-ok = [DB] 会话数量 ({ $count }) 在限制内 ({ $limit })。无需清理

# AppDetect
app-detect-config-watch = [AppDetect] 开始监控配置文件: { $path }
app-detect-change-detected = [AppDetect] 检测到变更，正在防抖 (100ms)...
app-detect-reloading = [AppDetect] 防抖结束。正在重载配置...
app-detect-load-failed = [AppDetect] 失败: { $error }。使用默认值
app-detect-reload-success = [AppDetect] 配置重载成功
app-detect-loop-started = [AppDetect] 应用检测循环已启动 (3000ms 轮询)
app-detect-screen-changed = [AppDetect] 屏幕状态变更: { $old } -> { $new }
app-detect-mode-change = [AppDetect] 模式变更: { $old } -> { $new }
app-detect-mode-change-pkg = [AppDetect] 模式变更: { $old } -> { $new } ({ $pkg })

# ScreenDetect
screen-state-change-detected = [Screen] 通过 '{ $source }' 检测到状态变更
screen-state-changed-value = [Screen] 屏幕状态已变更: { $state }
screen-netlink-started = [Screen] 已启动 netlink-sys 套接字监听器

# --- Scheduler (Additions) ---
scheduler-ipc-started = [Scheduler] IPC 通道监听器已启动
scheduler-mode-change-request = [Scheduler] 模式变更请求: { $old } -> { $new } (包名: { $pkg }, 温度: { $temp })
scheduler-boost-active-ignore = [Scheduler] 加速生效中，忽略模式应用
scheduler-apply-failed = [Scheduler] 应用设置失败: { $error }
scheduler-channel-closed = [Scheduler] 通道已关闭！线程退出
config-apply-mode-failed = 应用重载的模式设置失败: { $error }
config-apply-tweaks-failed = 应用重载的系统微调失败: { $error }
app-launch-watch-failed = 监控应用启动失败: { $error }
boost-apply-failed = 应用加速频率失败: { $error }
boost-restore-freq-failed = 恢复频率失败: { $error }
boost-mode-changed = 加速期间模式变更 ({ $old } -> { $new })，正在应用所有设置
boost-mode-apply-failed = 加速后应用新模式设置失败: { $error }
boost-get-mode-failed = 加速循环中无法获取当前模式: { $error }
pidof-failed = 执行 pidof '{ $name }' 失败: { $error }
process-not-found = 进程 '{ $name }' 未找到，跳过
cpuset-write-failed = 写入 cpuset ({ $name }) 失败: { $error }
cpuctl-write-failed = 写入 cpuctl ({ $name }) 失败: { $error }
thread-core-allocation-log = 线程核心分配已完成
main-config-watch-thread-create = 主配置监控线程已创建
applaunch-detected-boosting-frequencies = 检测到应用启动，正在提升频率...
boost-finished-restoring-settings = 加速结束，正在恢复设置
appLaunchboost-thread-created = 应用启动加速 (AppLaunchBoost) 线程已创建
apply-settings-for-mode = 正在应用模式: { $mode }
settings-applied-success = 模式 '{ $mode }' 的设置已成功应用

# --- Logger ---
log-level-updated = 日志级别已更新为: { $level }