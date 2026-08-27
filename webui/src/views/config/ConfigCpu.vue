<!--
  src/views/config/ConfigCpu.vue — 处理器子页 (蓝色)

  问题 3 修复核心: 档位 chip 直接绑定全局 store.currentMode.
  首页切"省电"后进本页, chip 立即显示"省电"; 在本页切档 = 全局切档 (写盘生效).

  需求升级:
  - 显示当前模式 + 目标负载 + 升/降频阈值 + 上/下行平滑 (按模式独立记忆)
  - 频率护栏 (亮屏/息屏两套 min/max) 读写 config.freq_limits
  - 所有修改 600ms 防抖自动保存 (不再需要手动点保存)
  - 底部"恢复本模块默认值" (恢复当前模式五参数 + 频率护栏)
-->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import yaml from 'js-yaml'
import { useSchedulerStore } from '@/stores/scheduler'
import { Bridge } from '@/utils/bridge'
import {
  CLG_PARAMS, FREQ_LIMIT_PARAMS, MODE_NAMES, CLG_MODE_DEFAULTS, FREQ_LIMIT_DEFAULTS,
  type ParamSpec,
} from '@/config/moduleSpecs'
import ParamRow from '@/components/ParamRow.vue'
import ResetDefaultsBtn from '@/components/ResetDefaultsBtn.vue'

const router = useRouter()
const store = useSchedulerStore()

const mainCfg = ref<any>(null)
const loading = ref(true)
const errMsg = ref('')
const okMsg = ref('')

const clgBase = computed(() => `${store.currentMode}.cpu_load_governor`)
const modeNames = MODE_NAMES
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

async function pickMode(k: string) {
  if (k === store.currentMode) return
  try { await store.switchMode(k) } catch (e) { errMsg.value = String(e) }
}

/** 防抖自动保存: 改完 600ms 自动写盘, daemon inotify 约 1 秒热生效 */
function persistMain() {
  if (saveTimer !== null) window.clearTimeout(saveTimer)
  saveTimer = window.setTimeout(async () => {
    try {
      await Bridge.saveMainConfig(yaml.load(yaml.dump(mainCfg.value)))
      store.reportSave(true)
      okMsg.value = `已自动保存并生效 (${MODE_NAMES[store.currentMode] ?? ''}档)`
      setTimeout(() => { okMsg.value = '' }, 2000)
    } catch (e) {
      store.reportSave(false)
      errMsg.value = String(e)
    }
  }, 600)
}

/** 恢复本模块默认: 当前模式五参数 (升/降频阈值+双平滑+目标负载) + 频率护栏 */
async function resetModuleDefaults() {
  if (!mainCfg.value) return
  const modeDefs = CLG_MODE_DEFAULTS[store.currentMode]
  if (modeDefs) {
    for (const [k, v] of Object.entries(modeDefs)) setP(`${clgBase.value}.${k}`, v)
  }
  if (!mainCfg.value.freq_limits) mainCfg.value.freq_limits = {}
  for (const [k, v] of Object.entries(FREQ_LIMIT_DEFAULTS)) {
    mainCfg.value.freq_limits[k] = v
  }
  await persistMainNow()
}

async function persistMainNow() {
  if (saveTimer !== null) { window.clearTimeout(saveTimer); saveTimer = null }
  try {
    await Bridge.saveMainConfig(yaml.load(yaml.dump(mainCfg.value)))
    store.reportSave(true)
    okMsg.value = '已恢复默认值并生效'
    setTimeout(() => { okMsg.value = '' }, 2500)
  } catch (e) {
    store.reportSave(false)
    errMsg.value = String(e)
  }
}

onMounted(async () => {
  try {
    if (!store.modeLoaded) await store.initData()
    else store.refreshMode()
    mainCfg.value = await Bridge.getMainConfig()
    if (!mainCfg.value.freq_limits) mainCfg.value.freq_limits = { ...FREQ_LIMIT_DEFAULTS }
  } catch (e) {
    errMsg.value = String(e)
    mainCfg.value = {}
  } finally { loading.value = false }
})
onUnmounted(() => { if (saveTimer !== null) window.clearTimeout(saveTimer) })

function rowSpec(p: ParamSpec) { return p }
/** 目标负载未配置时按模式显示真实回落默认 (daemon 硬编码映射), 避免误导 */
function rowVal(p: ParamSpec) {
  const v = getP(clgBase.value + '.' + p.path)
  if (v !== undefined) return v
  if (p.path === 'target_load') return CLG_MODE_DEFAULTS[store.currentMode]?.target_load
  return undefined
}
function rowUpd(p: ParamSpec, v: unknown) { setP(clgBase.value + '.' + p.path, v); persistMain() }

function flVal(p: ParamSpec) { return getP('freq_limits.' + p.path) ?? FREQ_LIMIT_DEFAULTS[p.path] }
function flUpd(p: ParamSpec, v: unknown) { setP('freq_limits.' + p.path, v); persistMain() }
</script>

<template>
  <div class="sub-page">
    <van-nav-bar title="处理器" left-arrow left-text="返回" @click-left="router.push('/config')" />

    <div class="sub-body">
      <div v-if="errMsg" class="cfg-banner err">⚠ {{ errMsg }}</div>
      <div v-if="okMsg" class="cfg-banner ok">{{ okMsg }}</div>
      <div v-if="loading" class="cfg-banner">读取配置中...</div>

      <section class="cfg-card" :style="{ borderLeft: '4px solid #3b82f6' }">
        <div class="cfg-card-head">
          <span class="cfg-card-name">处理器设置</span>
          <span class="live-tag">改动自动生效</span>
        </div>
        <p class="cfg-intro">五参数按档位独立记忆, 切档自动加载对应值; 修改后自动保存, 约 1 秒生效。</p>

        <div class="mode-chip-row">
          <button v-for="(n, k) in modeNames" :key="k"
                  class="mode-chip" :class="{ on: store.currentMode === k }" @click="pickMode(k as string)">
            {{ n }}
          </button>
        </div>

        <ParamRow
          v-for="p in CLG_PARAMS" :key="p.path"
          :spec="rowSpec(p)" :value="rowVal(p)"
          @update="(v) => rowUpd(p, v)"
        />
      </section>

      <section class="cfg-card" :style="{ borderLeft: '4px solid #1d4ed8' }">
        <div class="cfg-card-head">
          <span class="cfg-card-name">频率护栏</span>
          <span class="live-tag">改动自动生效</span>
        </div>
        <p class="cfg-intro">处理器频率的硬性上下限 (相对最高频的百分比), 亮屏/息屏独立两套; 与上面档位参数叠加生效。</p>

        <ParamRow
          v-for="p in FREQ_LIMIT_PARAMS" :key="p.path"
          :spec="rowSpec(p)" :value="flVal(p)"
          @update="(v) => flUpd(p, v)"
        />

        <ResetDefaultsBtn @reset="resetModuleDefaults" />
      </section>
    </div>
  </div>
</template>