<!--
  src/views/config/ConfigHotplug.vue — 核心开关子页 (红色)
  数据源: hotplug/config.yaml, 600ms 防抖写盘, daemon 200ms tick 拾取即生效.
-->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useSchedulerStore } from '@/stores/scheduler'
import {
  fetchHotplugState, fetchHotplugConfig, saveHotplugConfig,
  sanitizeKeepCores, type HotplugState, type HotplugConfig,
} from '@/api/hotplug'
import { HOTPLUG_PARAMS, KEEP_DESC, LOCKSCREEN_DESC, SCREENS_OFF_DESC, HOTPLUG_DEFAULTS, type ParamSpec } from '@/config/moduleSpecs'
import ParamRow from '@/components/ParamRow.vue'
import DescLines from '@/components/DescLines.vue'
import ResetDefaultsBtn from '@/components/ResetDefaultsBtn.vue'

const router = useRouter()
const store = useSchedulerStore()

const hpState = ref<HotplugState | null>(null)
const hpCfg = ref<HotplugConfig>({
  lockscreen_onoff: true, screens_onoff: true,
  off_threshold_idle_pct: 95, on_threshold_util_pct: 30,
  min_online_cores: 4, thermal_force_all_on_c: 70,
  screen_on_keep_cores: [0, 1, 2, 3, 4, 5], screen_off_keep_cores: [0, 1],
})
const loading = ref(true)
const errMsg = ref('')
const okMsg = ref('')

let pollTimer: number | null = null
let hpSaveTimer: number | null = null

function flashOk(msg: string) {
  okMsg.value = msg
  setTimeout(() => { okMsg.value = '' }, 2000)
}

/** 防抖保存: 改完 600ms 自动写盘, daemon 下个 tick 生效 */
function persistHp() {
  if (hpSaveTimer !== null) window.clearTimeout(hpSaveTimer)
  hpSaveTimer = window.setTimeout(async () => {
    try {
      await saveHotplugConfig(hpCfg.value)
      store.reportSave(true)
      flashOk('核心开关已自动保存并生效')
    } catch (e) {
      store.reportSave(false)
      errMsg.value = String(e)
    }
  }, 600)
}

const keepGroups = [
  { key: 'screen_on_keep_cores' as const, title: '亮屏时保留的核心', hint: '屏幕亮着时这些核心永不关闭, 其余核心按负载动态休眠' },
  { key: 'screen_off_keep_cores' as const, title: '息屏时保留的核心', hint: '黑屏待机时只保底这些核心, 更省电' },
]

function toggleKeep(group: 'screen_on_keep_cores' | 'screen_off_keep_cores', core: number) {
  const list = hpCfg.value[group]
  const idx = list.indexOf(core)
  if (idx >= 0) {
    if (core === 0) return // cpu0 启动核心必留
    list.splice(idx, 1)
    if (list.length < 2) list.push(list.includes(1) ? 0 : 1)
    hpCfg.value[group] = sanitizeKeepCores([...list])
  } else {
    list.push(core)
    hpCfg.value[group] = sanitizeKeepCores([...list])
  }
  persistHp()
}

const onlineCount = computed(() => {
  if (!hpState.value) return null
  let n = 0
  for (let i = 0; i < 8; i++) if (hpState.value.online_mask & (1 << i)) n++
  return n
})
const activeKeepNums = computed<string>(() => hpState.value?.active_keep_cores ?? '--')

onMounted(async () => {
  try {
    hpCfg.value = await fetchHotplugConfig()
    hpState.value = await fetchHotplugState()
    pollTimer = window.setInterval(async () => {
      try { hpState.value = await fetchHotplugState() } catch { /* 忽略单次失败 */ }
    }, 1500)
  } catch (e) {
    errMsg.value = String(e)
  } finally { loading.value = false }
})
onUnmounted(() => {
  if (pollTimer !== null) window.clearInterval(pollTimer)
  if (hpSaveTimer !== null) window.clearTimeout(hpSaveTimer)
})

function rowSpec(p: ParamSpec) { return p }
function rowVal(p: ParamSpec) { return (hpCfg.value as any)[p.path] }
function rowUpd(p: ParamSpec, v: unknown) { (hpCfg.value as any)[p.path] = v; persistHp() }

/** 恢复本模块默认: 核心开关全部参数 (含亮/息屏保留核心) */
function resetModuleDefaults() {
  hpCfg.value = JSON.parse(JSON.stringify(HOTPLUG_DEFAULTS))
  persistHp()
}
</script>

<template>
  <div class="sub-page">
    <van-nav-bar title="核心开关" left-arrow left-text="返回" @click-left="router.push('/config')" />

    <div class="sub-body">
      <div v-if="errMsg" class="cfg-banner err">⚠ {{ errMsg }}</div>
      <div v-if="okMsg" class="cfg-banner ok">{{ okMsg }}</div>
      <div v-if="loading" class="cfg-banner">读取配置中...</div>

      <section class="cfg-card" :style="{ borderLeft: '4px solid #ef4444' }">
        <div class="cfg-card-head">
          <span class="cfg-card-name">核心开关</span>
          <span class="live-tag">改动自动生效</span>
        </div>

        <div v-for="g in keepGroups" :key="g.key" class="keep-block">
          <div class="keep-title">{{ g.title }}</div>
          <div class="keep-hint">{{ g.hint }}</div>
          <div class="keep-grid">
            <button v-for="c in 8" :key="c - 1"
                    class="keep-btn"
                    :class="{ on: hpCfg[g.key].includes(c - 1), locked: c - 1 === 0 }"
                    @click="toggleKeep(g.key, c - 1)">
              核心{{ c }}<small v-if="c - 1 === 0">必留</small>
            </button>
          </div>
        </div>
        <DescLines :desc="KEEP_DESC" />

        <ParamRow
          v-for="p in HOTPLUG_PARAMS" :key="p.path"
          :spec="rowSpec(p)" :value="rowVal(p)"
          @update="(v) => rowUpd(p, v)"
        />

        <div class="switch-row">
          <div><b>锁屏时允许关核</b><small>锁屏界面仍动态休眠闲置核心</small></div>
          <van-switch v-model="hpCfg.lockscreen_onoff" size="22px" @change="persistHp" />
        </div>
        <DescLines :desc="LOCKSCREEN_DESC" />

        <div class="switch-row">
          <div><b>灭屏时允许关核</b><small>配合息屏保留核心工作, 推荐开启</small></div>
          <van-switch v-model="hpCfg.screens_onoff" size="22px" @change="persistHp" />
        </div>
        <DescLines :desc="SCREENS_OFF_DESC" />

        <div class="state-line">
          当前状态: {{ hpState?.screen_on ? '亮屏' : '息屏' }} · 在线 {{ onlineCount ?? '--' }}/8 ·
          生效白名单 [{{ activeKeepNums }}] · 温度 {{ hpState?.thermal_c?.toFixed(1) ?? '--' }}°C
        </div>

        <ResetDefaultsBtn @reset="resetModuleDefaults" />
      </section>
    </div>
  </div>
</template>