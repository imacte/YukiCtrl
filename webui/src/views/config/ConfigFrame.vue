<!--
  src/views/config/ConfigFrame.vue — 帧平滑子页 (粉色)
  数据源: rules.yaml fas_rules (daemon inotify → 即时热重载)
-->
<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useSchedulerStore } from '@/stores/scheduler'
import { Bridge } from '@/utils/bridge'
import { FAS_PARAMS, FAS_GEARS_DESC, MODE_NAMES } from '@/config/moduleSpecs'
import ParamRow from '@/components/ParamRow.vue'
import DescLines from '@/components/DescLines.vue'

const router = useRouter()
const store = useSchedulerStore()

const rulesCfg = ref<any>(null)
const loading = ref(true)
const errMsg = ref('')
const okMsg = ref('')
const fasGearsText = ref('30,60,90,120')

function getFas(path: string): any {
  let cur = rulesCfg.value?.fas_rules
  for (const k of path.split('.')) { cur = cur?.[k]; if (cur === undefined) return undefined }
  return cur
}
function setFas(path: string, v: any) {
  if (!rulesCfg.value.fas_rules) rulesCfg.value.fas_rules = {}
  const keys = path.split('.')
  let cur = rulesCfg.value.fas_rules
  for (let i = 0; i < keys.length - 1; i++) {
    if (!cur[keys[i]]) cur[keys[i]] = {}
    cur = cur[keys[i]]
  }
  cur[keys[keys.length - 1]] = v
}
function syncGearsText() {
  const gears = getFas('fps_gears')
  if (Array.isArray(gears)) fasGearsText.value = gears.join(',')
}
function applyFasGears() {
  const arr = fasGearsText.value
    .split(',').map(s => parseFloat(s.trim()))
    .filter(n => Number.isFinite(n) && n > 0)
  if (arr.length === 0) { syncGearsText(); return }
  setFas('fps_gears', arr)
}

async function saveFas() {
  try {
    applyFasGears() // 档位文本框一并落盘
    await Bridge.saveRulesConfig(rulesCfg.value)
    store.reportSave(true)
    okMsg.value = '帧平滑参数已生效'
    setTimeout(() => { okMsg.value = '' }, 2500)
  } catch (e) {
    store.reportSave(false)
    errMsg.value = String(e)
  }
}

onMounted(async () => {
  try {
    if (!store.modeLoaded) await store.initData()
    rulesCfg.value = await Bridge.getRulesConfig()
    syncGearsText()
  } catch (e) {
    errMsg.value = String(e)
    rulesCfg.value = {}
  } finally { loading.value = false }
})

const fasOnlyHint = computed(() =>
  `仅对规则页中标记为"帧率自适应"的前台应用生效。当前调度档位:${MODE_NAMES[store.currentMode] ?? store.currentMode}`)
</script>

<template>
  <div class="sub-page">
    <van-nav-bar title="帧平滑" left-arrow left-text="返回" @click-left="router.push('/config')" />

    <div class="sub-body">
      <div v-if="errMsg" class="cfg-banner err">⚠ {{ errMsg }}</div>
      <div v-if="okMsg" class="cfg-banner ok">{{ okMsg }}</div>
      <div v-if="loading" class="cfg-banner">读取配置中...</div>

      <section class="cfg-card" :style="{ borderLeft: '4px solid #ec4899' }">
        <div class="cfg-card-head">
          <span class="cfg-card-name">帧平滑</span>
          <button class="save-btn" :disabled="loading" @click="saveFas">保存</button>
        </div>
        <p class="cfg-intro">{{ fasOnlyHint }}</p>

        <div class="param">
          <div class="param-head"><span class="p-label">目标帧率档位</span></div>
          <input type="text" class="gears-input" v-model="fasGearsText"
                 placeholder="如 30,60,90,120" @change="applyFasGears" />
          <DescLines :desc="FAS_GEARS_DESC" />
        </div>

        <ParamRow
          v-for="p in FAS_PARAMS" :key="p.path"
          :spec="p" :value="getFas(p.path)"
          @update="(v) => setFas(p.path, v)"
        />
      </section>
    </div>
  </div>
</template>

<style scoped>
.param { margin-top: 16px; padding-top: 12px; border-top: 1px dashed var(--border); }
.param-head { display: flex; justify-content: space-between; align-items: baseline; }
.p-label { font-size: 14.5px; font-weight: 600; color: var(--text-primary); }
</style>