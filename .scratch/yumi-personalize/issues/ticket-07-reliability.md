# Ticket 07 — 可靠性增强 (Phase 5)

## 状态

📝 **草稿占位** — 详见 `.scratch/yumi-personalize/spec.md` Section 3 (Phase 5) + Section 7 (风险登记).

## 目标

保证模块稳定运行, 异常时自动恢复. 让用户在生产环境可以放心用 24/7.

## 核心任务

| 任务 | 文件 | 说明 |
|---|---|---|
| Watchdog 线程 | `src/watchdog.rs` 新增 | 5s 检查心跳/BPF map/核心数 |
| restore_defaults.sh | `scripts/restore_defaults.sh` 扩展 | 恢复所有 sysfs 参数 |
| 开机自启 | module 脚本 | 先恢复再启动 (避免上次崩溃残留) |
| 用户通知 | Android 通知 | 异常时弹通知 (KSU WebUI banner 或系统通知) |

## Watchdog 检查项 (来自 spec.md Section 7 风险登记)

| 检查项 | 检查方法 | 触发动作 |
|---|---|---|
| 自身心跳 | 内部 AtomicU64, 5s+1 | 重启 daemon |
| BPF map 存活 | 检查 4 个 anon_inode:bpf-map FD | 重载 eBPF 程序 |
| 核心数 | 读 /sys/devices/system/cpu/online | 与上次的 diff > 1 → 重新初始化 hotplug 状态 |
| eBPF 程序响应 | 读 idle/busy delta, 与 wall time 对比 | delta 异常 → 重载 eBPF |
| 关键 sysfs 可写 | 试写一次 scaling_min_freq | 写失败 → 提示 KSU allowlist 未配置 |
| 温度 | 读 /sys/class/thermal/thermal_zone*/temp | > 95°C → 紧急降频 |
| 内存 | 读 /proc/self/status RSS | > 100MB → 触发 leak 警告 |

## 开机自启顺序 (与 KSU 模块加载顺序协调)

baseline 实测:
1. `service.sh` 在 `late_start` 阶段被 KSU 调用
2. 启动 daemon 前, 必须**先**调用 `restore_defaults.sh` 把 sysfs 还原 (避免上次崩溃残留)
3. 然后启动 daemon (它会读取用户配置, 不会从默认值开始)

```bash
# module/service.sh (伪代码)
#!/system/bin/sh
# 1. 恢复 sysfs 默认
sh /data/adb/modules/yumi/scripts/restore_defaults.sh
# 2. 启动 daemon
/data/adb/modules/yumi/core/bin/yumi &
# 3. 记录 PID
echo $! > /data/adb/modules/yumi/run/yumi.pid
```

## 用户通知方案

- 正常运行时, 不发通知 (避免打扰)
- 异常恢复时, 通过 KSU WebUI banner 提示 (红条)
- 致命异常 (daemon 退出码非 0) 时, 发 Android 系统通知 (需要 KSU allowlist `cmd notification` 权限)

## 验收标准

- kill 模块进程能被拉起 (Watchdog 自身重启 OR KSU module manager 检测)
- 崩溃后参数恢复默认 (`restore_defaults.sh` 在 service.sh 启动前调用)
- 重启手机自动运行 (`customize.sh` + `service.sh` + `late_start`)
- 异常情况有通知 (WebUI banner + 关键时系统通知)

## 与其他 ticket 的依赖

- **后置依赖**: 所有其他 ticket 完成后再做可靠性, 否则会反复改

## 待 grill-me 确认

1. **Watchdog 自愈 vs 重启 daemon** — 优先尝试 self-heal (重载 BPF) 还是直接重启 daemon?
2. **崩溃检测** — daemon 进程消失 vs KSU 进程死锁检测?
3. **通知渠道** — 仅 WebUI banner (无系统通知权限) 还是申请 KSU 通知权限?
4. **遥测上传** — 是否上传崩溃日志到作者服务器 (privacy 顾虑)?
5. **测试方法** — 怎么测"崩溃后恢复"? SIGKILL daemon + 等 10s?

## 当前状态

- ❌ 未开始
- ⏭️ 最后做的 ticket (依赖前面所有)