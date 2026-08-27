<!--
  src/views/config/ConfigTemp.vue — 温度保护子页 (深红)
  需求升级: 单温度线 → 亮屏/息屏双套 × 软/硬双阈值.
  数据源: hotplug/config.yaml 的 temp_{on,off}_{soft,hard}_c + 实时温度.
-->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useSchedulerStore } from '@/stores/scheduler'
import { fetchHotplugConfig, saveHotplugConfig, type HotplugConfig } from '@/api/hotplug'
import { fetchSenseSnapshot, type SenseResult } from '@/api/sense'
import { TEMP_DUAL_PARAMS, SCREEN_SCOPES, HOTPLUG_DEFAULTS, type ParamSpec } from '@/config/moduleSpecs'
import ParamRow from '@/components/ParamRow.vue'
import ResetDefaultsBtn from '@/components/ResetDefaultsBtn.vue'

const router = useRouter()
const store = useSchedulerStore()

const hpCfg = ref<HotplugConfig | null>(null)
const senseRes = ref<SenseResult | null>(null)
const loading = ref(true)
const errMsg = ref('')
const okMsg = ref('')
const scope = ref<'screen_on' | 'screen_off'>('screen_on')

let saveTimer: number | null = null
let pollTimer: number | null = null

const tempText = computed(() => {
  const t = senseRes.value?.data.temp_c ?? 0
  return t > 0 ? t.toFixed(1) : '--'
})

function persist() {
  if (saveTimer !== null) window.clearTimeout(saveTimer)
  saveTimer = window.setTimeout(async () => {
    if (!hpCfg.value) return
    try {
      await saveHotplugConfig(hpCfg.value)
      store.reportSave(true)
      okMsg.value = '温度保护已自动保存并生效'
      setTimeout(() => { okMsg.value = '' }, 2000)
    } catch (e) {
      store.reportSave(false)
      errMsg.value = String(e)
    }
  }, 600)
}

/** 当前 scope 的 soft/hard 键名 */
function keyOf(leaf: string): 'temp_on_soft_c' | 'temp_on_hard_c' | 'temp_off_soft_c' | 'temp_off_hard_c' {
  return (scope.value === 'screen_on' ? 'temp_on_' : 'temp_off_') + leaf as 'temp_on_soft_c'
}

function rowVal(p: ParamSpec): number {
  if (!hpCfg.value) return Number(p.fb)
  const v = (hpCfg.value as any)[keyOf(p.path.replace('_c', ''))]
  return v !== undefined ? v : Number(p.fb)
}
function rowUpd(p: ParamSpec, v: unknown) {
  if (!hpCfg.value) return
  (hpCfg.value as any)[keyOf(p.path.replace('_c', ''))] = Number(v)
  persist()
}

/** 恢复本模块默认: 双套 soft/hard 全恢复 */
function resetModuleDefaults() {
  if (hpCfg.value) {
    hpCfg.value.temp_on_soft_c = HOTPLUG_DEFAULTS.temp_on_soft_c
    hpCfg.value.temp_on_hard_c = HOTPLUG_DEFAULTS.temp_on_hard_c
    hpCfg.value.temp_off_soft_c = HOTPLUG_DEFAULTS.temp_off_soft_c
    hpCfg.value.temp_off_hard_c = HOTPLUG_DEFAULTS.temp_off_hard_c
    persist()
  }
}

onMounted(async () => {
  try {
    hpCfg.value = await fetchHotplugConfig()
    const refresh = async () => { try { senseRes.value = await fetchSenseSnapshot() } catch { /* 保持 */ } }
    refresh()
    pollTimer = window.setInterval(refresh, 1500)
  } catch (e) {
    errMsg.value = String(e)
  } finally { loading.value = false }
})
onUnmounted(() => {
  if (pollTimer !== null) window.clearInterval(pollTimer)
  if (saveTimer !== null) window.clearTimeout(saveTimer)
})
</script>

<template>
  <div class="sub-page">
    <van-nav-bar title="温度保护" left-arrow left-text="返回" @click-left="router.push('/config')" />

    <div class="sub-body">
      <div v-if="errMsg" class="cfg-banner err">⚠ {{ errMsg }}</div>
      <div v-if="okMsg" class="cfg-banner ok">{{ okMsg }}</div>
      <div v-if="loading" class="cfg-banner">读取配置中...</div>

      <section class="cfg-card" :style="{ borderLeft: '4px solid #991b1b' }">
        <div class="cfg-card-head">
          <span class="cfg-card-name">温度保护</span>
          <span class="live-tag">改动自动生效</span>
        </div>

        <div class="readout">
          <div><span>当前温度</span><b>{{ tempText }}°C</b></div>
          <div><span>保护动作</span><b>暂停关核</b></div>
        </div>

        <div class="scope-chip-row">
          <button v-for="(n, k) in SCREEN_SCOPES" :key="k"
                  class="scope-chip" :class="{ on: scope === k }" @click="scope = k as string">
            {{ n }}
          </button>
        </div>
        <p class="cfg-intro">两套独立记忆: 硬阈值达到即强制全核在线 (未配置时沿用 70°C);
        软阈值仅预警日志。</p>

        <ParamRow
          v-for="p in TEMP_DUAL_PARAMS" :key="p.path"
          :spec="p" :value="rowVal(p)"
          @update="(v) => rowUpd(p, v)"
        />

        <ResetDefaultsBtn @reset="resetModuleDefaults" />
      </section>
    </div>
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
