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

use std::error::Error;
use std::sync::{Arc, Mutex};
use log::{debug, info};
use std::process;
use std::thread;
use std::time::Duration;
use kobject_uevent::{UEvent, ActionType};
use netlink_sys::{protocols::NETLINK_KOBJECT_UEVENT, Socket, SocketAddr};

use crate::i18n::{t, t_with_args};
use crate::fluent_args;

fn update_state_if_changed(state_arc: &Arc<Mutex<bool>>, new_state: bool, source: &str) {
    let mut state_lock = state_arc.lock().unwrap();
    if *state_lock != new_state {
        info!("{}", t_with_args("screen-state-change-detected", &fluent_args!("source" => source)));
        *state_lock = new_state;
        let state_str = if new_state { "ON" } else { "OFF" };
        info!("{}", t_with_args("screen-state-changed-value", &fluent_args!("state" => state_str)));
        // 同步推送进 sense_snapshot: hotplug 决策器用它选择 keep_cores 白名单
        crate::monitor::sense_snapshot::screen_push(new_state);
    } else {
        debug!("[screen_detect] {} reported state={} (no change)", source, new_state);
    }
}

pub fn monitor_screen_state_uevent(state_arc: Arc<Mutex<bool>>) -> Result<(), Box<dyn Error>> {
    // 初始同步: daemon 启动时设备大概率亮屏, 把 Arc 初值 (true) 推给 sense_snapshot,
    // 避免首个 uevent 到达前 hotplug 读到 Default=false 而误用息屏白名单.
    let initial = *state_arc.lock().unwrap();
    crate::monitor::sense_snapshot::screen_push(initial);

    let mut socket = Socket::new(NETLINK_KOBJECT_UEVENT)?;
    let sa = SocketAddr::new(process::id(), 1);
    socket.bind(&sa)?;
    let _ = socket.set_rx_buf_sz(2 * 1024 * 1024);
    info!("{}", t("screen-netlink-started"));

    loop {
        match socket.recv_from_full() {
            Ok((buf, _)) => {
                if let Ok(event) = UEvent::from_netlink_packet(&buf) {
                    debug!(
                        "[screen_detect] uevent subsystem={} action={:?} devpath={}",
                        event.subsystem, event.action, event.devpath.display(),
                    );
                    if event.subsystem == "power" {
                         if let Some(action) = event.env.get("POWER_ACTION") {
                            if action == "early_suspend" { update_state_if_changed(&state_arc, false, "power"); }
                            else if action == "late_resume" { update_state_if_changed(&state_arc, true, "power"); }
                         }
                    } else if event.subsystem == "backlight" && event.action == ActionType::Change {
                        thread::sleep(Duration::from_millis(100));
                        let dev = event.devpath.display();
                        let bl_power = format!("/sys{}/bl_power", dev);
                        let actual = format!("/sys{}/actual_brightness", dev);

                        let new_state = crate::utils::read_i32_from_file(&bl_power).map(|v| v == 0)
                            .or_else(|_| crate::utils::read_i32_from_file(&actual).map(|v| v > 0)).ok();

                        debug!(
                            "[screen_detect] backlight change on {} -> state={:?}",
                            dev, new_state,
                        );
                        if let Some(state) = new_state {
                            update_state_if_changed(&state_arc, state, "backlight");
                        }
                    }
                }
            },
            Err(e) => {
                debug!("[screen_detect] netlink recv error: {} (sleep 1s)", e);
                thread::sleep(Duration::from_secs(1))
            },
        }
    }
}