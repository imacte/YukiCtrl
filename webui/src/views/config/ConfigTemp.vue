<!--
  src/views/config/ConfigTemp.vue — 温度保护子页 (深红)
  数据源: hotplug/config.yaml 的 thermal_force_all_on_c + sense 快照实时温度.
-->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useSchedulerStore } from '@/stores/scheduler'
import { fetchHotplugConfig, saveHotplugConfig, type HotplugConfig } from '@/api/hotplug'
import { fetchSenseSnapshot, type SenseResult } from '@/api/sense'
import { TEMP_PARAM } from '@/config/moduleSpecs'
import ParamRow from '@/components/ParamRow.vue'

const router = useRouter()
const store = useSchedulerStore()

const hpCfg = ref<HotplugConfig | null>(null)
const senseRes = ref<SenseResult | null>(null)
const loading = ref(true)
const errMsg = ref('')
const okMsg = ref('')

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
      okMsg.value = '温度保护线已生效'
      setTimeout(() => { okMsg.value = '' }, 2000)
    } catch (e) {
      store.reportSave(false)
      errMsg.value = String(e)
    }
  }, 600)
}

function rowVal(p: typeof TEMP_PARAM) { return (hpCfg.value as any)?.[p.path] }
function rowUpd(v: unknown) { if (hpCfg.value) { (hpCfg.value as any)[TEMP_PARAM.path] = v; persist() } }

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
          <span class="live-tag">改动即生效</span>
        </div>

        <div class="readout">
          <div><span>当前温度</span><b>{{ tempText }}°C</b></div>
          <div><span>保护动作</span><b>暂停关核</b></div>
        </div>

        <ParamRow
          v-if="hpCfg"
          :spec="TEMP_PARAM" :value="rowVal(TEMP_PARAM)"
          @update="rowUpd"
        />

        <p class="cfg-intro">触发后守护进程暂停一切关核动作直到温度回落; 该保护优先级最高。</p>
      </section>
    </div>
  </div>
</template>