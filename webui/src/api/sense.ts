// src/api/sense.ts
//
// 任务 #5 / ticket-09: 读取 daemon 写的 sense/snapshot.yaml.
//
// 数据源: src/sensor/snapshot_writer.rs (200ms tick 写盘)
// WebUI 通过 KSU exec cat 读取 (沿用 bridge.ts 风格).

import { Bridge } from '@/utils/bridge'

const SENSE_PATH = '/data/adb/modules/core-pilot/sense/snapshot.yaml'

/**
 * 与后端 SenseSnapshot + snapshot_writer.rs 输出字段一一对应.
 * 字段缺失 → safe-default (0 / false / [] / "").
 */
export interface SenseSnapshot {
  /** 8 核 CPU util % (0..=100), 不足 8 个补 0 */
  cpu_utils_pct: number[]
  /** GPU 负载 % (0..=100); NaN = 不可读 */
  gpu_load_pct: number
  /** IO 压力 some=10 % (0..=100) */
  io_some_pct: number
  /** IO 压力 full=10 % (0..=100) */
  io_full_pct: number
  /** 内存 PSI full=10 % (0..=100) */
  mem_full_pct: number
  /** 内存 PSI full=10 绝对时间 (us, /proc/pressure/memory) */
  mem_full_us: number
  /** zram 已用 (MB); 0 = 没 zram */
  swap_used_mb: number
  /** SoC 温度 (°C) */
  temp_c: number
  /** 屏幕 FPS (0 = 不可读 / 屏幕关闭) */
  fps: number
  /** 屏幕是否亮起 */
  screen_on: boolean
  /** 触摸是否按下 */
  touch_down: boolean
  /** 触摸事件距今毫秒 */
  touch_age_ms: number
  /** 当前前台包名 */
  current_pkg: string
  /** 更新时间 (unix ms) */
  updated_at_unix_ms: number
}

const DEFAULT_SNAPSHOT: SenseSnapshot = {
  cpu_utils_pct: [0, 0, 0, 0, 0, 0, 0, 0],
  gpu_load_pct: 0,
  io_some_pct: 0,
  io_full_pct: 0,
  mem_full_pct: 0,
  mem_full_us: 0,
  swap_used_mb: 0,
  temp_c: 0,
  fps: 0,
  screen_on: false,
  touch_down: false,
  touch_age_ms: 9999,
  current_pkg: '',
  updated_at_unix_ms: 0,
}

/** 读取 daemon 200ms tick 写的 snapshot.yaml. */
export async function fetchSenseSnapshot(): Promise<SenseSnapshot> {
  try {
    const raw = await Bridge.readFile(SENSE_PATH)
    return parseSnapshotYaml(raw, DEFAULT_SNAPSHOT)
  } catch {
    return DEFAULT_SNAPSHOT
  }
}

function parseSnapshotYaml(raw: string, fallback: SenseSnapshot): SenseSnapshot {
  const result: SenseSnapshot = { ...fallback }
  for (const line of raw.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const m = trimmed.match(/^([\w_]+):\s*(.+)$/)
    if (!m) continue
    const [, key, valueRaw] = m
    const value = valueRaw.trim()
    switch (key) {
      case 'cpu_utils_pct': {
        // [12.0,5.5,...]
        const inner = value.replace(/^\[/, '').replace(/\]$/, '')
        const arr = inner.split(',').map(s => parseFloat(s.trim()) || 0)
        for (let i = 0; i < 8; i++) result.cpu_utils_pct[i] = arr[i] ?? 0
        break
      }
      case 'gpu_load_pct':
        result.gpu_load_pct = parseFloat(value) || 0
        break
      case 'io_some_pct':
        result.io_some_pct = parseFloat(value) || 0
        break
      case 'io_full_pct':
        result.io_full_pct = parseFloat(value) || 0
        break
      case 'mem_full_pct':
        result.mem_full_pct = parseFloat(value) || 0
        break
      case 'mem_full_us':
        result.mem_full_us = parseInt(value, 10) || 0
        break
      case 'swap_used_mb':
        result.swap_used_mb = parseInt(value, 10) || 0
        break
      case 'temp_c':
        result.temp_c = parseFloat(value) || 0
        break
      case 'fps':
        result.fps = parseInt(value, 10) || 0
        break
      case 'screen_on':
        result.screen_on = value === 'true'
        break
      case 'touch_down':
        result.touch_down = value === 'true'
        break
      case 'touch_age_ms':
        result.touch_age_ms = parseInt(value, 10) || 9999
        break
      case 'current_pkg':
        result.current_pkg = unquoteYaml(value)
        break
      case 'updated_at_unix_ms':
        result.updated_at_unix_ms = parseInt(value, 10) || 0
        break
    }
  }
  return result
}

function unquoteYaml(v: string): string {
  if (v.startsWith('"') && v.endsWith('"')) {
    return v.slice(1, -1).replace(/\\"/g, '"').replace(/\\\\/g, '\\')
  }
  return v
}