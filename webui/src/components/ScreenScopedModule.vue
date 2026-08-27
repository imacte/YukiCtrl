<!--
  src/components/ScreenScopedModule.vue — 亮/息屏双套模块配置卡 (通用)

  数据源: config.yaml modules.<moduleKey>.{screen_on,screen_off}
  行为: 亮/息屏 chip 切换编辑目标套; 600ms 防抖自动保存; daemon inotify
        热重载 + 屏幕切换时按套应用 (modules_ctrl.rs).
  恢复: 两套一并恢复 MODULE_SCOPED_DEFAULTS[moduleKey].
  实时读数区由父页面通过默认 slot 注入。
-->
<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import yaml from 'js-yaml'
import { useSchedulerStore } from '@/stores/scheduler'
import { Bridge } from '@/utils/bridge'
import { SCREEN_SCOPES, MODULE_SCOPED_DEFAULTS, type ParamSpec } from '@/config/moduleSpecs'
import ParamRow from '@/components/ParamRow.vue'
import ResetDefaultsBtn from '@/components/ResetDefaultsBtn.vue'

const props = defineProps<{
  moduleKey: 'gpu' | 'touch' | 'swap'
  params: ParamSpec[]
}>()

const store = useSchedulerStore()
const mainCfg = ref<any>(null)
const scope = ref<'screen_on' | 'screen_off'>('screen_on')
const loading = ref(true)
const errMsg = ref('')
const okMsg = ref('')
let saveTimer: number | null = null

function getP(path: string): any {
  let cur = mainCfg.value
  for (const k of path.split('.')) { cur = cur?.[k]; if (cur === undefined) return undefined }
  return cur
}
function setP(path: string, v: any) {
  if (!mainCfg.value) return
  const keys = path.split('.')
  let cur = mainCfg.value
  for (let i = 0; i < keys.length - 1; i++) {
    if (cur[keys[i]] === undefined || cur[keys[i]] === null) cur[keys[i]] = {}
    cur = cur[keys[i]]
  }
  cur[keys[keys.length - 1]] = v
}

const base = () => `modules.${props.moduleKey}.${scope.value}`

/** 确保双套结构存在 (缺省补默认, 避免写回时丢段) */
function ensureStructure() {
  if (!mainCfg.value.modules) mainCfg.value.modules = {}
  const m = mainCfg.value.modules
  if (!m[props.moduleKey]) m[props.moduleKey] = {}
  for (const sc of ['screen_on', 'screen_off']) {
    if (!m[props.moduleKey][sc]) m[props.moduleKey][sc] = {}
  }
}

function persist() {
  if (saveTimer !== null) window.clearTimeout(saveTimer)
  saveTimer = window.setTimeout(async () => {
    try {
      await Bridge.saveMainConfig(yaml.load(yaml.dump(mainCfg.value)))
      store.reportSave(true)
      okMsg.value = '已自动保存并生效'
      setTimeout(() => { okMsg.value = '' }, 2000)
    } catch (e) {
      store.reportSave(false)
      errMsg.value = String(e)
    }
  }, 600)
}

/** 恢复本模块默认: 两套一并恢复 */
function resetDefaults() {
  if (!mainCfg.value) return
  ensureStructure()
  const defs = MODULE_SCOPED_DEFAULTS[props.moduleKey]
  for (const [sc, kv] of Object.entries(defs)) {
    for (const [k, v] of Object.entries(kv)) {
      mainCfg.value.modules[props.moduleKey][sc][k] = v
    }
  }
  if (saveTimer !== null) { window.clearTimeout(saveTimer); saveTimer = null }
  Bridge.saveMainConfig(yaml.load(yaml.dump(mainCfg.value))).then(() => {
    store.reportSave(true)
    okMsg.value = '已恢复默认值并生效'
    setTimeout(() => { okMsg.value = '' }, 2500)
  }).catch((e: unknown) => { errMsg.value = String(e) })
}

onMounted(async () => {
  try {
    mainCfg.value = await Bridge.getMainConfig()
    ensureStructure()
  } catch (e) {
    errMsg.value = String(e)
    mainCfg.value = {}
  } finally { loading.value = false }
})
onUnmounted(() => { if (saveTimer !== null) window.clearTimeout(saveTimer) })

function rowVal(p: ParamSpec): any {
  const v = getP(`${base()}.${p.path}`)
  if (v !== undefined) return v
  const fb = MODULE_SCOPED_DEFAULTS[props.moduleKey]?.[scope.value]?.[p.path]
  return fb !== undefined ? fb : undefined
}
function rowUpd(p: ParamSpec, v: unknown) { setP(`${base()}.${p.path}`, v); persist() }
</script>

<template>
  <div>
    <div v-if="errMsg" class="cfg-banner err">⚠ {{ errMsg }}</div>
    <div v-if="okMsg" class="cfg-banner ok">{{ okMsg }}</div>
    <div v-if="loading" class="cfg-banner">读取配置中...</div>

    <slot />

    <div class="scope-chip-row">
      <button v-for="(n, k) in SCREEN_SCOPES" :key="k"
              class="scope-chip" :class="{ on: scope === k }" @click="scope = k as string">
        {{ n }}
      </button>
    </div>
    <p class="cfg-intro">两套独立记忆: 切到对应状态页签修改, 保存后亮/息屏切换时自动应用对应值。</p>

    <ParamRow
      v-for="p in params" :key="p.path"
      :spec="p" :value="rowVal(p)"
      @update="(v) => rowUpd(p, v)"
    />

    <ResetDefaultsBtn @reset="resetDefaults" />
  </div>
</template>

<style scoped>
.scope-chip-row { display: flex; gap: 8px; margin: 12px 0 4px; }
.scope-chip {
  flex: 1; padding: 9px 0; border-radius: 10px; font-size: 14px; font-weight: 600;
  border: 1px solid var(--border-strong); background: var(--bg-card);
  color: var(--text-secondary);
}
.scope-chip.on { border-color: var(--accent); color: var(--accent); background: color-mix(in srgb, var(--accent) 8%, transparent); }
</style>
