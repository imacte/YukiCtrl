// src/api/appRules.ts
//
// 任务 #5: App 规则管理 API.
//
// 数据模型对齐后端 src/scheduler/app_rule.rs::AppRule:
//   package: string
//   rule_type: "restrict" | "boost"
//   strength: "light" | "medium" | "heavy" (默认 medium)
//   max_freq_scale?: number   (不传 = 按 strength 自动)
//   target_util_offset?: number
//   disable_burst?: boolean
//   boost_threshold_offset?: number
//
// 存储位置: /data/adb/modules/core-pilot/config/config.yaml 的根 `app_rules:` 数组.
// 加载方式: cat config.yaml → yaml.load → 读 .app_rules →
// 保存方式: 读完整 yaml → 改 .app_rules → 写回 (保留其他字段).

import yaml from 'js-yaml'
import { Bridge } from '@/utils/bridge'
import { toast } from '@/kernelsu'
import i18n from '@/i18n'

export type RuleType = 'restrict' | 'boost'
export type RuleStrength = 'light' | 'medium' | 'heavy'

export interface AppRule {
  package: string
  rule_type: RuleType
  strength?: RuleStrength
  max_freq_scale?: number
  target_util_offset?: number
  disable_burst?: boolean
  boost_threshold_offset?: number
}

const CONFIG_PATH = '/data/adb/modules/core-pilot/config/config.yaml'

/**
 * 读取 config.yaml, 返回所有 App 规则.
 * config.yaml 不存在 / 无 app_rules 字段 → 返回空数组.
 */
export async function fetchAppRules(): Promise<AppRule[]> {
  try {
    const raw = await Bridge.readFile(CONFIG_PATH)
    const cfg = (yaml.load(raw) as any) || {}
    const arr = cfg.app_rules
    return Array.isArray(arr) ? arr : []
  } catch {
    return []
  }
}

/**
 * 写回单条规则 (upsert):
 *   - 不存在则追加
 *   - 已存在 (按 package 匹配) 则替换
 *   - rule 对象若字段为空 / undefined, 不写入 (后端 default 即可)
 */
export async function saveAppRule(rule: AppRule): Promise<void> {
  const cfg = await readConfigOrEmpty()
  if (!Array.isArray(cfg.app_rules)) cfg.app_rules = []

  // 清理空字段 (避免写 noise)
  const cleaned: AppRule = {
    package: rule.package,
    rule_type: rule.rule_type,
    ...(rule.strength ? { strength: rule.strength } : {}),
    ...(rule.max_freq_scale != null ? { max_freq_scale: rule.max_freq_scale } : {}),
    ...(rule.target_util_offset != null ? { target_util_offset: rule.target_util_offset } : {}),
    ...(rule.disable_burst ? { disable_burst: true } : {}),
    ...(rule.boost_threshold_offset != null && rule.boost_threshold_offset !== 0
      ? { boost_threshold_offset: rule.boost_threshold_offset }
      : {}),
  }

  const idx = cfg.app_rules.findIndex((r: AppRule) => r.package === rule.package)
  if (idx >= 0) {
    cfg.app_rules[idx] = cleaned
  } else {
    cfg.app_rules.push(cleaned)
  }

  await writeConfig(cfg)
  toast(i18n.global.t('app_rule_saved') as string)
}

/**
 * 删除指定包的规则
 */
export async function deleteAppRule(pkg: string): Promise<void> {
  const cfg = await readConfigOrEmpty()
  if (Array.isArray(cfg.app_rules)) {
    cfg.app_rules = cfg.app_rules.filter((r: AppRule) => r.package !== pkg)
  }
  await writeConfig(cfg)
  toast(i18n.global.t('app_rule_deleted') as string)
}

async function readConfigOrEmpty(): Promise<any> {
  try {
    const raw = await Bridge.readFile(CONFIG_PATH)
    return (yaml.load(raw) as any) || {}
  } catch {
    return {}
  }
}

async function writeConfig(cfg: any): Promise<void> {
  // noRefs: 避免 &id001 这种 anchor (后端 serde_yaml 可能不接受)
  await Bridge.writeFile(CONFIG_PATH, yaml.dump(cfg, { noRefs: true, lineWidth: -1 }))
}

/**
 * strength 自动推导的默认值 (UI 显示用, 让用户看到 strength=heavy 实际偏置多少).
 * 与后端 src/scheduler/app_rule.rs::default_max_freq_scale / default_target_util_offset 一致.
 */
export function defaultsFor(t: RuleType, s: RuleStrength): { max_freq_scale: number; target_util_offset: number } {
  const mag = s === 'light' ? { scale: 0.05, util: 10 }
    : s === 'heavy' ? { scale: 0.20, util: 35 }
    : { scale: 0.10, util: 20 } // medium
  return {
    max_freq_scale: t === 'restrict' ? 1.0 - mag.scale : 1.0 + mag.scale,
    target_util_offset: t === 'restrict' ? -mag.util : mag.util,
  }
}