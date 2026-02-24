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

use aya::{Ebpf, include_bytes_aligned, programs::UProbe, maps::perf::AsyncPerfEventArray};
use aya::util::online_cpus;
use bytes::BytesMut;
use std::sync::mpsc::Sender;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use crate::common::DaemonEvent;
use crate::monitor::app_detect;
use log::{info, debug, warn};

pub async fn start_fps_loop(tx: Sender<DaemonEvent>) -> Result<(), anyhow::Error> {
    // 1. 加载嵌入的 eBPF 字节码 (由 build.rs 编译并传递路径)
    // 使用 Aya 官方的 include_bytes_aligned! 确保 ARM64 对齐
    static BPF_DATA: &[u8] = include_bytes_aligned!(env!("BPF_OBJ_PATH"));
    
    info!("Initializing eBPF FPS monitor...");

    // 2. 加载 eBPF 程序
    // Box::leak 满足 aya 的 'static 生命周期要求
    let bpf = Box::leak(Box::new(Ebpf::load(BPF_DATA)?));

    // 3. 挂载 Uprobe
    let program: &mut UProbe = bpf.program_mut("handle_frame").unwrap().try_into()?;
    program.load()?;
    
    let syms = [
        "_ZN7android7Surface11queueBufferEP19ANativeWindowBufferi", 
        "_ZN7android7Surface11queueBufferEP19ANativeWindowBufferiPNS_24SurfaceQueueBufferOutputE",
        "_ZN7android16BufferQueueProducer11queueBufferEiRKNS_10IGraphicBufferProducer10QueueBufferInputEPNS1_11QueueBufferOutputE",
        "_ZN7android16BufferQueueProducer11queueBufferEiRKNS_22IGraphicBufferProducer10QueueBufferInputEPNS1_11QueueBufferOutputE"
    ];
    
    let mut attached_count = 0;
    for sym in syms {
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

    // ── 设置内核侧 PID 过滤 ──
    // 绕过 aya 对 bpf 对象的单一可变借用限制：
    // 通过裸指针重新构造借用，从而可以同时拿到 target_pid 和 frame_events 两个 Map 的可变引用
    let bpf_ptr = bpf as *mut Ebpf;
    
    let mut target_pid_arr = if let Some(map) = unsafe { &mut *bpf_ptr }.map_mut("target_pid") {
        match aya::maps::Array::<_, u32>::try_from(map) {
            Ok(mut arr) => {
                let initial_pid = app_detect::get_current_pid() as u32;
                if initial_pid > 0 {
                    let _ = arr.set(0, initial_pid, 0);
                    info!("Kernel PID filter initialized: pid={}", initial_pid);
                }
                Some(arr)
            }
            Err(e) => {
                warn!("target_pid Array setup failed: {}", e);
                None
            }
        }
    } else {
        info!("target_pid map not found in BPF, kernel PID filter disabled");
        None
    };

    let has_kernel_filter = target_pid_arr.is_some();

    // 4. 获取 Perf Event Map（复用裸指针借用）
    let map = unsafe { &mut *bpf_ptr }.map_mut("frame_events").expect("frame_events map not found in BPF object");
    let mut perf_array = AsyncPerfEventArray::try_from(map)?;

    // ── 共享状态：PID 和包名缓存 ──
    let shared_pid = Arc::new(AtomicU32::new(app_detect::get_current_pid() as u32));
    let shared_package = Arc::new(RwLock::new(app_detect::get_current_package()));

    // ── PID 更新任务 ──
    // 独立任务周期性地：
    //   1. 刷新 AtomicU32 中的目标 PID
    //   2. 通过安全的 Array API 更新 BPF target_pid map
    //   3. 刷新包名缓存
    {
        let pid_arc = shared_pid.clone();
        let pkg_arc = shared_package.clone();

        tokio::spawn(async move {
            let mut last_pid: u32 = 0;
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                let current_pid = app_detect::get_current_pid() as u32;

                if current_pid != last_pid {
                    // 更新共享 PID（per-CPU 任务的快速路径）
                    pid_arc.store(current_pid, Ordering::Relaxed);

                    // [FIX-MON-2] 更新 BPF map（内核侧过滤）
                    if let Some(arr) = &mut target_pid_arr {
                        if current_pid > 0 {
                            if let Err(e) = arr.set(0, current_pid, 0) {
                                warn!("Failed to update kernel PID filter to {}: {}", current_pid, e);
                            } else {
                                debug!("Updated kernel PID filter: {} → {}", last_pid, current_pid);
                            }
                        }
                    }

                    // 更新包名缓存
                    let pkg = app_detect::get_current_package();
                    if let Ok(mut w) = pkg_arc.write() {
                        *w = pkg;
                    }

                    last_pid = current_pid;
                }
            }
        });
    }

    // 5. 为每个在线 CPU 核心开启监听任务
    for cpu_id in online_cpus().map_err(|e| anyhow::anyhow!("CPU access error: {:?}", e))? {
        let mut buf = perf_array.open(cpu_id, None)?;
        let tx_clone = tx.clone();
        let pid_arc = shared_pid.clone();
        let pkg_arc = shared_package.clone();

        tokio::spawn(async move {
            // 预分配缓冲区
            let mut buffers = (0..10).map(|_| BytesMut::with_capacity(1024)).collect::<Vec<_>>();

            loop {
                match buf.read_events(&mut buffers).await {
                    Ok(events) => {
                        // 记录丢失的事件（perf buffer 溢出）
                        if events.lost > 0 {
                            warn!("FPS perf buffer lost {} events on CPU {}", events.lost, cpu_id);
                        }

                        for i in 0..events.read {
                            let data = &buffers[i];
                            
                            // packed 结构体 (12 bytes): [0..4] u32 pid, [4..12] u64 delta
                            if data.len() < 12 { continue; }

                            let event_pid = u32::from_ne_bytes(data[0..4].try_into().unwrap());
                            let delta = u64::from_ne_bytes(data[4..12].try_into().unwrap());

                            if delta == 0 { continue; }

                            // 使用缓存的 PID 而非每帧调用 get_current_pid()
                            let target_pid = pid_arc.load(Ordering::Relaxed);

                            // 用户态 PID 过滤（内核过滤的安全网 + PID 变更时的过渡期保护）
                            if event_pid != target_pid { continue; }

                            // 直接转发每一帧给 FAS 控制器
                            // 不做 worst_delta 聚合，不做 12ms 限流。
                            // FAS 自己有完善的窗口过滤和 EMA 平滑，
                            // monitor 的职责只是忠实传递数据。
                            let fps = 1_000_000_000.0 / (delta as f64);

                            log::trace!("[fps_monitor] Raw frame from BPF: delta={}ns, inst_fps={:.2}", delta, fps);

                            // 使用缓存的包名
                            let package_name = match pkg_arc.read() {
                                Ok(pkg) => pkg.clone(),
                                Err(_) => continue,
                            };

                            if let Err(_) = tx_clone.send(DaemonEvent::FrameUpdate {
                                package_name,
                                fps: fps as f32,
                                frame_delta_ns: delta,
                            }) {
                                return; // 主线程崩溃时退出
                            }
                        }
                    }
                    Err(e) => {
                        warn!("FPS perf buffer read error on CPU {}: {}", cpu_id, e);
                        // 短暂等待后重试，避免忙循环
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                }
            }
        });
    }

    info!("eBPF FPS monitor started successfully (kernel PID filter: {}).",
        if has_kernel_filter { "active" } else { "disabled" });
    
    // 保持主异步函数挂起
    std::future::pending::<()>().await;
    Ok(())
}