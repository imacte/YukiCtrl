// src/api/hotplug.ts
//
// Phase 2 / ticket-04 — 热插拔 (Hotplug) WebUI ↔ daemon 文件 IPC
// 任务 A — 可配置保留核心 (keep_cores): 配置/状态新增亮屏·息屏保留核心等字段.
//
// 设计: 没有 daemon HTTP server, WebUI 通过 KSU exec 直接读写 daemon 写的 state.yaml
//       和用户改的 config.yaml. 与现有 webui/src/utils/bridge.ts 风格一致 (exec shell).
//
// 通信路径:
//   daemon → WebUI: cat <module_root>/hotplug/state.yaml   (500ms 节流回写)
//   WebUI → daemon: echo "..." > <module_root>/hotplug/config.yaml (200ms tick 轮询生效)

import { Bridge } from '@/utils/bridge'

const HOTPLUG_DIR = '/data/adb/modules/core-pilot/hotplug'

export interface HotplugState {
  /** 8-bit bitmask, bit N = cpu N online */
  online_mask: number
  /** SoC 温度 (°C) */
  thermal_c: number
  lockscreen_onoff: boolean
  screens_onoff: boolean
  off_threshold_idle_pct: number
  on_threshold_util_pct: number
  min_online_cores: number
  thermal_force_all_on_c: number
  disable_debounce_ticks: number
  /** daemon 当前感知的屏幕状态 (决定用哪一组 keep_cores) */
  screen_on: boolean
  /** 当前实际生效的保留核心列表, 如 "0,1,2,3,4,5" */
  active_keep_cores: string
  updated_at_unix_ms: number
}

export interface HotplugConfig {
  lockscreen_onoff: boolean
  screens_onoff: boolean
  off_threshold_idle_pct: number
  on_threshold_util_pct: number
  min_online_cores: number
  thermal_force_all_on_c: number
  /** 亮屏时永不关闭的核心 */
  screen_on_keep_cores: number[]
  /** 息屏时永不关闭的核心 */
  screen_off_keep_cores: number[]
  /** 需求: 温度双套 (temp_{on,off}_{soft,hard}_c); hard<=0 时 daemon 回落 thermal_force_all_on_c */
  temp_on_soft_c: number
  temp_on_hard_c: number
  temp_off_soft_c: number
  temp_off_hard_c: number
}

/** 兜底安全约束与 daemon 一致: cpu0 恒保留; 有效保护核 < 2 时自动补 cpu1 */
export function sanitizeKeepCores(cores: number[]): number[] {
  const set = new Set(cores.filter(c => c >= 0 && c <= 7))
  set.add(0)
  if (set.size < 2) set.add(1)
  return [...set].sort((a, b) => a - b)
}

/**
 * 读取 daemon 写的 hotplug state.yaml.
 * 文件不存在 → 返回 safe-default (全 online + 默认配置).
 */
export async function fetchHotplugState(): Promise<HotplugState> {
  const defaults: HotplugState = {
    online_mask: 0xff,
    thermal_c: 0,
    lockscreen_onoff: true,
    screens_onoff: true,
    off_threshold_idle_pct: 95,
    on_threshold_util_pct: 30,
    min_online_cores: 4,
    thermal_force_all_on_c: 70,
    disable_debounce_ticks: 5,
    screen_on: true,
    active_keep_cores: '0,1,2,3,4,5',
    updated_at_unix_ms: 0
  }
  try {
    const raw = await Bridge.readFile(`${HOTPLUG_DIR}/state.yaml`)
    return parseStateYaml(raw, defaults)
  } catch {
    return defaults
  }
}

/**
 * 保存用户配置. WebUI → daemon: 写 config.yaml; daemon 200ms tick 轮询读取,
 * keep_cores / 阈值改动下一 tick 即生效, 无需重启守护进程.
 * 提交前强制过一遍安全约束 (cpu0 恒在 / 至少 2 核).
 */
export async function saveHotplugConfig(cfg: HotplugConfig): Promise<void> {
  const on = sanitizeKeepCores(cfg.screen_on_keep_cores)
  const off = sanitizeKeepCores(cfg.screen_off_keep_cores)
  const body =
    `# core-pilot hotplug user config (written by WebUI)\n` +
    `lockscreen_onoff: ${cfg.lockscreen_onoff}\n` +
    `screens_onoff: ${cfg.screens_onoff}\n` +
    `off_threshold_idle_pct: ${cfg.off_threshold_idle_pct}\n` +
    `on_threshold_util_pct: ${cfg.on_threshold_util_pct}\n` +
    `min_online_cores: ${Math.max(2, Math.min(8, Math.round(cfg.min_online_cores)))}\n` +
    `thermal_force_all_on_c: ${cfg.thermal_force_all_on_c}\n` +
    `temp_on_soft_c: ${cfg.temp_on_soft_c ?? 0}\n` +
    `temp_on_hard_c: ${cfg.temp_on_hard_c ?? 0}\n` +
    `temp_off_soft_c: ${cfg.temp_off_soft_c ?? 0}\n` +
    `temp_off_hard_c: ${cfg.temp_off_hard_c ?? 0}\n` +
    `screen_on_keep_cores: [${on.join(',')}]\n` +
    `screen_off_keep_cores: [${off.join(',')}]\n`
  await Bridge.writeFile(`${HOTPLUG_DIR}/config.yaml`, body)
}

/**
 * 把 8-bit mask 拆成 8 个 boolean 数组 (cpu0..cpu7).
 * WebUI 8 核网格用这个.
 */
export function maskToCpuArray(mask: number): boolean[] {
  return Array.from({ length: 8 }, (_, i) => (mask & (1 << i)) !== 0)
}

/** 解析 "0,1,2" / "[0,1,2]" 形式的核心列表 */
function parseCoreList(value: string): string {
  return value.replace(/^\[/, '').replace(/\]$/, '').trim()
}

function parseCoreArray(value: string): number[] {
  return parseCoreList(value)
    .split(',')
    .map(s => parseInt(s.trim(), 10))
    .filter(n => !Number.isNaN(n))
}

const DEFAULT_CONFIG: HotplugConfig = {
  lockscreen_onoff: true,
  screens_onoff: true,
  off_threshold_idle_pct: 95,
  on_threshold_util_pct: 30,
  min_online_cores: 4,
  thermal_force_all_on_c: 70,
  screen_on_keep_cores: [0, 1, 2, 3, 4, 5],
  screen_off_keep_cores: [0, 1],
  temp_on_soft_c: 0,
  temp_on_hard_c: 70,
  temp_off_soft_c: 0,
  temp_off_hard_c: 70,
}

/**
 * 读取磁盘上的 config.yaml 作为设置页编辑初值.
 * 文件不存在 (daemon 从未写过) → 返回与 daemon 一致的内置默认值.
 */
export async function fetchHotplugConfig(): Promise<HotplugConfig> {
  const fallback = { ...DEFAULT_CONFIG }
  try {
    const raw = await Bridge.readFile(`${HOTPLUG_DIR}/config.yaml`)
    const result: HotplugConfig = { ...fallback }
    for (const line of raw.split('\n')) {
      const trimmed = line.trim()
      if (!trimmed || trimmed.startsWith('#')) continue
      const m = trimmed.match(/^([\w_]+):\s*(.+)$/)
      if (!m) continue
      const [, key, valueRaw] = m
      const value = valueRaw.trim()
      switch (key) {
        case 'lockscreen_onoff': result.lockscreen_onoff = value === 'true'; break
        case 'screens_onoff': result.screens_onoff = value === 'true'; break
        case 'off_threshold_idle_pct': result.off_threshold_idle_pct = parseFloat(value) || 95; break
        case 'on_threshold_util_pct': result.on_threshold_util_pct = parseFloat(value) || 30; break
        case 'min_online_cores': result.min_online_cores = parseInt(value, 10) || 4; break
        case 'thermal_force_all_on_c': result.thermal_force_all_on_c = parseFloat(value) || 70; break
        case 'screen_on_keep_cores': result.screen_on_keep_cores = parseCoreArray(value); break
        case 'screen_off_keep_cores': result.screen_off_keep_cores = parseCoreArray(value); break
        case 'temp_on_soft_c': result.temp_on_soft_c = parseFloat(value) || 0; break
        case 'temp_on_hard_c': result.temp_on_hard_c = parseFloat(value) || 0; break
        case 'temp_off_soft_c': result.temp_off_soft_c = parseFloat(value) || 0; break
        case 'temp_off_hard_c': result.temp_off_hard_c = parseFloat(value) || 0; break
      }
    }
    return sanitizeKeepCoresAll(result)
  } catch {
    return fallback
  }
}

/** 两组保留核心都过一遍安全约束 (cpu0 恒在 / 至少 2 核), 与 daemon 行为一致 */
export function sanitizeKeepCoresAll(cfg: HotplugConfig): HotplugConfig {
  return {
    ...cfg,
    screen_on_keep_cores: sanitizeKeepCores(cfg.screen_on_keep_cores),
    screen_off_keep_cores: sanitizeKeepCores(cfg.screen_off_keep_cores),
  }
}

/** 简易 YAML key: value 解析器 (不引入 js-yaml, 性能更好, 适合小文件) */
function parseStateYaml(raw: string, fallback: HotplugState): HotplugState {
  const result = { ...fallback }
  for (const line of raw.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const m = trimmed.match(/^([\w_]+):\s*(.+)$/)
    if (!m) continue
    const [, key, valueRaw] = m
    const value = valueRaw.trim()
    switch (key) {
      case 'online_mask':
        result.online_mask = parseInt(value, 16) || 0
        break
      case 'thermal_c':
        result.thermal_c = parseFloat(value) || 0
        break
      case 'lockscreen_onoff':
        result.lockscreen_onoff = value === 'true'
        break
      case 'screens_onoff':
        result.screens_onoff = value === 'true'
        break
      case 'off_threshold_idle_pct':
        result.off_threshold_idle_pct = parseFloat(value) || 95
        break
      case 'on_threshold_util_pct':
        result.on_threshold_util_pct = parseFloat(value) || 30
        break
      case 'min_online_cores':
        result.min_online_cores = parseInt(value, 10) || 4
        break
      case 'thermal_force_all_on_c':
        result.thermal_force_all_on_c = parseFloat(value) || 70
        break
      case 'disable_debounce_ticks':
        result.disable_debounce_ticks = parseInt(value, 10) || 5
        break
      case 'screen_on':
        result.screen_on = value === 'true'
        break
      case 'active_keep_cores':
        result.active_keep_cores = parseCoreList(value)
        break
      case 'updated_at_unix_ms':
        result.updated_at_unix_ms = parseInt(value, 10) || 0
        break
    }
  }
  return result
}