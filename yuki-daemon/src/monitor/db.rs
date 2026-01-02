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

use rusqlite::Connection;
use std::error::Error;
use log::{info};
use chrono::Utc;

use crate::common;
use crate::i18n::{t, t_with_args};
use crate::fluent_args;

fn get_db_path() -> std::path::PathBuf {
    common::get_module_root().join("yuki_power.db")
}

pub fn open_db_connection() -> Result<Connection, rusqlite::Error> {
    let db_path = get_db_path();
    let conn = Connection::open(&db_path)?;
    
    // 保持 WAL 优化
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA wal_autocheckpoint = 100;"
    )?;
    
    Ok(conn)
}

pub fn init_db() -> Result<(), Box<dyn Error>> {
    let conn = open_db_connection()?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS power_log (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            sessionId INTEGER NOT NULL,
            timestamp INTEGER NOT NULL,
            packageName TEXT NOT NULL,
            voltageMicrovolts REAL NOT NULL,
            currentMicroamps REAL NOT NULL,
            temperatureCelsius REAL NOT NULL,
            batteryPercentage INTEGER NOT NULL
        )",
        [],
    )?;
    info!("{}", t_with_args("db-initialized", &fluent_args!("path" => get_db_path().display().to_string())));
    Ok(())
}

pub fn insert_power_log(
    conn: &Connection, 
    session_id: i64,
    pkg: &str,
    voltage_uv: f64,
    current_ua: f64,
    temp_c: f64,
    battery_pct: i32,
) -> Result<(), rusqlite::Error> {
    let timestamp = Utc::now().timestamp_millis();

    // [极致优化] 使用 prepare_cached 缓存 SQL 语句对象
    // 这避免了每 2.5s 重复解析相同的 SQL 字符串，降低 CPU 解析开销
    let mut stmt = conn.prepare_cached(
        "INSERT INTO power_log (
            sessionId, timestamp, packageName, 
            voltageMicrovolts, currentMicroamps,
            temperatureCelsius, batteryPercentage
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    )?;

    stmt.execute((
        session_id, timestamp, pkg,
        voltage_uv, current_ua, temp_c, battery_pct,
    ))?;

    // 保持 debug 日志注释状态以节省性能
    /*
    debug!("{}", t_with_args("db-logged-raw", &fluent_args!(
        "vol" => voltage_uv, 
        "cur" => current_ua, 
        "pkg" => pkg
    )));
    */
    
    Ok(())
}

pub fn trim_old_sessions(conn: &Connection, limit: u32) -> Result<(), Box<dyn Error>> {
    let num_sessions: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT sessionId) FROM power_log",
        [],
        |row| row.get(0),
    )?;

    if num_sessions > limit as i64 {
        let safe_limit = if limit == 0 { 1 } else { limit };
        let num_to_delete = num_sessions - (safe_limit as i64);
        
        info!("{}", t_with_args("db-session-limit-exceeded", &fluent_args!(
            "count" => num_sessions, "limit" => limit, "trim" => num_to_delete
        )));
        
        let rows_deleted = conn.execute(
            "DELETE FROM power_log WHERE sessionId IN (
                SELECT sessionId FROM (
                    SELECT sessionId FROM power_log 
                    GROUP BY sessionId 
                    ORDER BY MIN(timestamp) ASC 
                    LIMIT ?1
                )
            )",
            (num_to_delete,),
        )?;
        info!("{}", t_with_args("db-trimmed-entries", &fluent_args!(
            "rows" => rows_deleted, "sessions" => num_to_delete
        )));
    }
    Ok(())
}