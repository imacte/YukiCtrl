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

use std::collections::HashMap;
use std::error::Error;
use std::process::Command;
use log::{info, warn};
use super::config::{self, BootScriptsConfig};

use crate::i18n::{t, t_with_args};
use crate::fluent_args;

pub fn run_boot_scripts() -> Result<(), Box<dyn Error>> {
    info!("{}", t("boot-scripts-running"));
    
    let path = config::get_boot_scripts_path();
    let path_str = path.to_str().ok_or("Invalid path")?;

    let config: BootScriptsConfig =
        config::read_config(path_str).unwrap_or(BootScriptsConfig {
            scripts: HashMap::new(),
        });

    let scripts_dir = config::get_scripts_dir();

    for (script_name, enabled) in config.scripts {
        if enabled {
            let script_path = scripts_dir.join(format!("{}.sh", script_name));
            let script_path_str = script_path.to_str().unwrap_or("");

            info!("{}", t_with_args("boot-script-applying", &fluent_args!("path" => script_path_str)));
            match Command::new("sh").arg(script_path_str).output() {
                Ok(output) => {
                    if output.status.success() {
                        info!("{}", t_with_args("boot-script-success", &fluent_args!("name" => script_name.clone())));
                    } else {
                        warn!("{}", t_with_args("boot-script-failed", &fluent_args!("name" => script_name.clone(), "error" => String::from_utf8_lossy(&output.stderr).to_string())));
                    }
                }
                Err(e) => warn!("{}", t_with_args("boot-script-exec-failed", &fluent_args!("name" => script_name.clone(), "error" => e.to_string()))),
            }
        }
    }
    info!("{}", t("boot-scripts-finished"));
    Ok(())
}