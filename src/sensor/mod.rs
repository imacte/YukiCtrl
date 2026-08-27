// src/sensor/mod.rs
//
// 任务 #5 (WebUI 扩展): 把全局 SenseSnapshot 200ms 序列化到
// /data/adb/modules/core-pilot/sense/snapshot.yaml, 让 WebUI 通过
// KSU exec cat 读出来刷新 SensePanel.
//
// 设计动机:
//   - 项目没有 daemon HTTP server, WebUI 走 KSU exec IPC (沿用 bridge.ts 风格)
//   - hotplug/state.yaml 已经有相同套路, 我们沿用同样格式 (单 yaml, 500ms 防抖)
//
// 不变性:
//   - 本模块只读 SenseSnapshot + 写文件, 不引入新依赖
//   - 写文件失败仅 warn, 不影响主循环
//   - 文件不存在 / 解析失败时 WebUI 走 safe-default (前端 mock)
//
// 接入点: scheduler/mod.rs::start_scheduler_threads() 末尾 spawn 一个新线程.

pub use snapshot_writer::{init_sense_state_file, start_sense_snapshot_thread};

mod snapshot_writer;