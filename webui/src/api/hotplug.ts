// src/api/hotplug.ts
//
// Phase 2 / ticket-04 — 热插拔 (Hotplug) WebUI ↔ daemon 文件 IPC
//
// 设计: 没有 daemon HTTP server, WebUI 通过 KSU exec 直接读写 daemon 写的 state.yaml
//       和用户改的 config.yaml. 与现有 webui/src/utils/bridge.ts 风格一致 (exec shell).
//
// 通信路径:
//   daemon → WebUI: cat <module_root>/hotplug/state.yaml
//   WebUI → daemon: echo "..." > <module_root>/hotplug/config.yaml
//
// 依赖: webui/src/utils/bridge.ts::Bridge (继承 exec 封装)
//
// WebUI 调示例:
//   import { fetchHotplugState, saveHotplugConfig } from '@/api/hotplug'
//   const state = await fetchHotplugState()
//   await saveHotplugConfig({ lockscreen_onoff: true, off_threshold_idle_pct: 95 })

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
  disable_debounce_ticks: number
  updated_at_unix_ms: number
}

export interface HotplugConfig {
  lockscreen_onoff: boolean
  screens_onoff: boolean
  off_threshold_idle_pct: number
  on_threshold_util_pct: number
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
    disable_debounce_ticks: 5,
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
 * 保存用户配置 (D4/D6: 2 toggle + 2 slider).
 * WebUI → daemon: 写 config.yaml; daemon 200ms 轮询读取.
 */
export async function saveHotplugConfig(cfg: HotplugConfig): Promise<void> {
  const body =
    `# core-pilot hotplug user config (written by WebUI)\n` +
    `lockscreen_onoff: ${cfg.lockscreen_onoff}\n` +
    `screens_onoff: ${cfg.screens_onoff}\n` +
    `off_threshold_idle_pct: ${cfg.off_threshold_idle_pct}\n` +
    `on_threshold_util_pct: ${cfg.on_threshold_util_pct}\n`
  await Bridge.writeFile(`${HOTPLUG_DIR}/config.yaml`, body)
}

/**
 * 把 8-bit mask 拆成 8 个 boolean 数组 (cpu0..cpu7).
 * WebUI 8 核网格用这个.
 */
export function maskToCpuArray(mask: number): boolean[] {
  return Array.from({ length: 8 }, (_, i) => (mask & (1 << i)) !== 0)
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
      case 'disable_debounce_ticks':
        result.disable_debounce_ticks = parseInt(value, 10) || 5
        break
      case 'updated_at_unix_ms':
        result.updated_at_unix_ms = parseInt(value, 10) || 0
        break
    }
  }
  return result
}