<!--
  src/views/config/ConfigIo.vue — 读写子页 (橙色)
  问题 4 修复落地页: 键名 IO_Settings.Scheduler (大写 S), read_ahead_kb/nomerges 保持字符串.
-->
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import yaml from 'js-yaml'
import { useSchedulerStore } from '@/stores/scheduler'
import { Bridge } from '@/utils/bridge'
import { IO_PARAMS, IO_OPT_DESC } from '@/config/moduleSpecs'
import ParamRow from '@/components/ParamRow.vue'
import DescLines from '@/components/DescLines.vue'

const router = useRouter()
const store = useSchedulerStore()

const mainCfg = ref<any>(null)
const loading = ref(true)
const errMsg = ref('')
const okMsg = ref('')

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

async function saveMain() {
  try {
    await Bridge.saveMainConfig(yaml.load(yaml.dump(mainCfg.value)))
    store.reportSave(true)
    okMsg.value = '已保存, 约 1 秒内热生效'
    setTimeout(() => { okMsg.value = '' }, 2500)
  } catch (e) {
    store.reportSave(false)
    errMsg.value = String(e)
  }
}

onMounted(async () => {
  try { mainCfg.value = await Bridge.getMainConfig() }
  catch (e) { errMsg.value = String(e); mainCfg.value = {} }
  finally { loading.value = false }
})
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
          <button class="save-btn" :disabled="loading" @click="saveMain">保存</button>
        </div>
        <p class="cfg-intro">影响应用打开速度、滑动加载速度; 保存后约 1 秒热生效。</p>

        <div class="switch-row" style="margin-top: 4px;">
          <div><b>读写优化总开关</b><small>关闭后本页其余参数全部不生效</small></div>
          <van-switch size="22px" :model-value="!!getP('function.IOOptimization')"
                      @update:model-value="(v: boolean) => setP('function.IOOptimization', v)" />
        </div>
        <DescLines :desc="IO_OPT_DESC" />

        <ParamRow
          v-for="p in IO_PARAMS" :key="p.path"
          :spec="p" :value="getP(p.path)"
          @update="(v) => setP(p.path, v)"
        />
      </section>
    </div>
  </div>
</template>