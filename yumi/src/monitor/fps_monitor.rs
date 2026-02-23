/*
 * Copyright (C) 2026 yuki
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

// 引入 include_bytes_aligned 宏来解决 ARM64 内存对齐导致的 ELF 解析错误
use aya::{Ebpf, include_bytes_aligned, programs::UProbe, maps::perf::AsyncPerfEventArray};
use aya::util::online_cpus;
use bytes::BytesMut;
use std::sync::mpsc::Sender;
use std::time::Instant;
use crate::common::DaemonEvent;
use crate::monitor::app_detect;
use log::{info, debug};

pub async fn start_fps_loop(tx: Sender<DaemonEvent>) -> Result<(), anyhow::Error> {
    // 1. 加载嵌入的 eBPF 字节码 (由 build.rs 编译并传递路径)
    // 【核心修复】使用 Aya 官方的 include_bytes_aligned! 替代标准的 include_bytes!
    // 这能确保嵌入的 ELF 字节数组在 ARM64 设备上满足严格的 8 字节内存对齐要求
    static BPF_DATA: &[u8] = include_bytes_aligned!(env!("BPF_OBJ_PATH"));
    
    info!("Initializing eBPF FPS monitor...");

    // 2. 加载 eBPF 程序
    // 使用 Box::leak 是为了满足 aya 的 'static 生命周期要求，
    // 因为这个程序在守护进程整个生命周期内都要存在。
    let bpf = Box::leak(Box::new(Ebpf::load(BPF_DATA)?));

    // 3. 挂载 Uprobe (用户态探针)
    // 我们要挂钩的是 libgui.so 中的 queueBuffer 函数，这是安卓绘制画面的必经之路
    let program: &mut UProbe = bpf.program_mut("handle_frame").unwrap().try_into()?;
    program.load()?;
    
    // 安卓 libgui.so 的符号名 (针对不同安卓版本可能略有不同，这里列出了最常见的两个)
    // _ZN7android7Surface11queueBuffer... 是 C++ Name Mangling 后的名字
    let syms = [
        "_ZN7android7Surface11queueBufferEP19ANativeWindowBufferi", 
        "_ZN7android7Surface11queueBufferEP19ANativeWindowBufferiPNS_24SurfaceQueueBufferOutputE"
    ];
    
    let mut attached_count = 0;
    for sym in syms {
        // 尝试挂载到 /system/lib64/libgui.so
        // 如果系统是 32 位，可能需要改成 /system/lib/libgui.so
        match program.attach(Some(sym), 0, "/system/lib64/libgui.so", None) {
            Ok(_) => {
                info!("Attached uprobe to symbol: {}", sym);
                attached_count += 1;
            },
            Err(e) => {
                debug!("Failed to attach to {}: {}", sym, e);
            }
        }
    }

    if attached_count == 0 {
        return Err(anyhow::anyhow!("Failed to attach any Uprobe symbols! Check libgui.so path or symbols."));
    }

    // 4. 获取 Perf Event Map 用于内核态向用户态传递数据
    let map = bpf.map_mut("frame_events").expect("frame_events map not found in BPF object");
    let mut perf_array = AsyncPerfEventArray::try_from(map)?;

    // 5. 为每个在线 CPU 核心开启监听任务
    for cpu_id in online_cpus().map_err(|e| anyhow::anyhow!("CPU access error: {:?}", e))? {
        let mut buf = perf_array.open(cpu_id, None)?;
        let tx_clone = tx.clone();

        // 启动异步任务处理每个 CPU 的缓冲区
        tokio::spawn(async move {
            // 预分配缓冲区
            let mut buffers = (0..10).map(|_| BytesMut::with_capacity(1024)).collect::<Vec<_>>();

            let mut last_send_time = Instant::now();
            let mut worst_delta_ns: u64 = 0;
            
            loop {
                if let Ok(events) = buf.read_events(&mut buffers).await {
                    for i in 0..events.read {
                        let data = &buffers[i];
                        if data.len() >= 8 {
                            let delta = u64::from_ne_bytes(data[0..8].try_into().unwrap());
                            let target_pid = app_detect::get_current_pid();
                            
                            if delta > 0 && target_pid > 0 {
                                // 1. 吸收积压：记录本周期内的“最差帧” (耗时最长的帧)
                                if delta > worst_delta_ns {
                                    worst_delta_ns = delta;
                                }

                                // 2. 限流发送：如果距离上次发送超过了 12ms (上限约 83 次/秒)
                                // 如果游戏是 30fps (33.3ms)，这个条件也会立即满足，保证低帧率 0 延迟
                                if last_send_time.elapsed().as_millis() >= 12 {
                                    let fps = 1_000_000_000.0 / (worst_delta_ns as f64);
                                    
                                    if let Err(_) = tx_clone.send(DaemonEvent::FrameUpdate {
                                        package_name: app_detect::get_current_package(),
                                        fps: fps as f32,
                                        frame_delta_ns: worst_delta_ns,
                                    }) {
                                        return; // 主线程崩溃时退出
                                    }

                                    // 3. 发送后重置状态
                                    worst_delta_ns = 0;
                                    last_send_time = Instant::now();
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    info!("eBPF FPS monitor started successfully.");
    
    // 保持主异步函数挂起，只要不返回，上面的 tokio::spawn 就会一直运行
    std::future::pending::<()>().await;
    Ok(())
}