<!--
  src/views/config/ConfigCpu.vue — 处理器子页 (蓝色)

  问题 3 修复核心: 档位 chip 直接绑定全局 store.currentMode.
  首页切"省电"后进本页, chip 立即显示"省电"; 在本页切档 = 全局切档 (写盘生效).
-->
<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import yaml from 'js-yaml'
import { useSchedulerStore } from '@/stores/scheduler'
import { Bridge } from '@/utils/bridge'
import { CLG_PARAMS, MODE_NAMES, type ParamSpec } from '@/config/moduleSpecs'
import ParamRow from '@/components/ParamRow.vue'

const router = useRouter()
const store = useSchedulerStore()

const mainCfg = ref<any>(null)
const loading = ref(true)
const errMsg = ref('')
const okMsg = ref('')

const clgBase = computed(() => `${store.currentMode}.cpu_load_governor`)
const modeNames = MODE_NAMES

function getP(path: string): any {
  let cur = mainCfg.value
  for (const k of path.split('.')) { cur = cur?.[k]; if (cur === undefined) return undefined }
  return cur
}
function setP(path: string, v: any) {
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

async function saveMain() {
  try {
    await Bridge.saveMainConfig(yaml.load(yaml.dump(mainCfg.value)))
    store.reportSave(true)
    okMsg.value = `已保存, ${MODE_NAMES[store.currentMode] ?? ''}档约 1 秒内生效`
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
  } catch (e) {
    errMsg.value = String(e)
    mainCfg.value = {}
  } finally { loading.value = false }
})

function rowSpec(p: ParamSpec) { return p }
function rowVal(p: ParamSpec) { return getP(clgBase.value + '.' + p.path) }
function rowUpd(p: ParamSpec, v: unknown) { setP(clgBase.value + '.' + p.path, v) }
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
          <button class="save-btn" :disabled="loading" @click="saveMain">保存</button>
        </div>
        <p class="cfg-intro">按档位独立记忆: 先选档位再调参数, 保存后约 1 秒生效。
          当前全局档位与首页同步。</p>

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
    </div>
  </div>
</template>