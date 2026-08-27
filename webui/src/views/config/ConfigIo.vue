<!--
  src/views/config/ConfigIo.vue — 读写子页 (橙色)
  问题 4 修复落地页: 键名 IO_Settings.Scheduler (大写 S), read_ahead_kb/nomerges 保持字符串.
-->
<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import yaml from 'js-yaml'
import { useSchedulerStore } from '@/stores/scheduler'
import { Bridge } from '@/utils/bridge'
import { IO_PARAMS, IO_OPT_DESC, IO_DEFAULTS, IO_OFF_PARAMS, MODULE_SCOPED_DEFAULTS } from '@/config/moduleSpecs'
import ParamRow from '@/components/ParamRow.vue'
import DescLines from '@/components/DescLines.vue'
import ResetDefaultsBtn from '@/components/ResetDefaultsBtn.vue'

const router = useRouter()
const store = useSchedulerStore()

const mainCfg = ref<any>(null)
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

/** 防抖自动保存: 改完 600ms 自动写盘, daemon inotify 约 1 秒热生效 */
function persistMain() {
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

/** 恢复本模块默认: 读写三参数 + 总开关 (值必须保持字符串) + 息屏套 */
function resetModuleDefaults() {
  if (!mainCfg.value) return
  if (!mainCfg.value.IO_Settings) mainCfg.value.IO_Settings = {}
  for (const [k, v] of Object.entries(IO_DEFAULTS)) mainCfg.value.IO_Settings[k] = v
  setP('function.IOOptimization', true)
  if (!mainCfg.value.modules) mainCfg.value.modules = {}
  if (!mainCfg.value.modules.io) mainCfg.value.modules.io = {}
  if (!mainCfg.value.modules.io.screen_off) mainCfg.value.modules.io.screen_off = {}
  for (const [k, v] of Object.entries(MODULE_SCOPED_DEFAULTS.io.screen_off)) {
    mainCfg.value.modules.io.screen_off[k] = v
  }
  persistMain()
}

/** 息屏套读写 (modules.io.screen_off.*) */
function offVal(p: { path: string; fb?: string }) {
  return getP('modules.io.screen_off.' + p.path) ?? MODULE_SCOPED_DEFAULTS.io.screen_off[p.path] ?? p.fb
}
function offUpd(p: { path: string }, v: unknown) { setP('modules.io.screen_off.' + p.path, v); persistMain() }

onMounted(async () => {
  try { mainCfg.value = await Bridge.getMainConfig() }
  catch (e) { errMsg.value = String(e); mainCfg.value = {} }
  finally { loading.value = false }
})
onUnmounted(() => { if (saveTimer !== null) window.clearTimeout(saveTimer) })
</script>

<template>
  <div class="sub-page">
    <van-nav-bar title="读写" left-arrow left-text="返回" @click-left="router.push('/config')" />

    <div class="sub-body">
      <div v-if="errMsg" class="cfg-banner err">⚠ {{ errMsg }}</div>
      <div v-if="okMsg" class="cfg-banner ok">{{ okMsg }}</div>
      <div v-if="loading" class="cfg-banner">读取配置中...</div>

      <section class="cfg-card" :style="{ borderLeft: '4px solid #f59e0b' }">
        <div class="cfg-card-head">
          <span class="cfg-card-name">读写设置</span>
          <span class="live-tag">改动自动生效</span>
        </div>
        <p class="cfg-intro">影响应用打开速度、滑动加载速度; 修改自动保存, 约 1 秒热生效。</p>

        <div class="switch-row" style="margin-top: 4px;">
          <div><b>读写优化总开关</b><small>关闭后本页其余参数全部不生效</small></div>
          <van-switch size="22px" :model-value="!!getP('function.IOOptimization')"
                      @update:model-value="(v: boolean) => { setP('function.IOOptimization', v); persistMain() }" />
        </div>
        <DescLines :desc="IO_OPT_DESC" />

        <ParamRow
          v-for="p in IO_PARAMS" :key="p.path"
          :spec="p" :value="getP(p.path)"
          @update="(v) => { setP(p.path, v); persistMain() }"
        />

        <div class="off-section">
          <div class="off-title">息屏时 (独立记忆)</div>
          <p class="cfg-intro">黑屏待机时自动切换到这套值; 亮屏恢复上方设置。</p>
          <ParamRow
            v-for="p in IO_OFF_PARAMS" :key="'off-' + p.path"
            :spec="p" :value="offVal(p)"
            @update="(v) => offUpd(p, v)"
          />
        </div>

        <ResetDefaultsBtn @reset="resetModuleDefaults" />
      </section>
    </div>
  </div>
</template>

<style scoped>
.off-section { margin-top: 18px; padding-top: 14px; border-top: 1px dashed var(--border); }
.off-title { font-size: 14px; font-weight: 700; color: var(--accent); margin-bottom: 2px; }
</style>