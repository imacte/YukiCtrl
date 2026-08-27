<!--
  src/views/ConfigListView.vue — 调度页 (问题 5 拆分后)

  8 个模块入口卡, 每张卡: 主题色图标 + 模块名 + 一句话介绍 + 当前关键值摘要.
  点击卡片进入 /config/<key> 子页面做具体配置.
  当前模式显示读全局 store (问题 3): 首页切换后这里立即同步.
-->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useSchedulerStore } from '@/stores/scheduler'
import { MODULES, MODE_NAMES, IO_PARAMS } from '@/config/moduleSpecs'
import { Bridge } from '@/utils/bridge'
import { fetchHotplugState, type HotplugState } from '@/api/hotplug'
import { fetchSenseSnapshot, type SenseResult } from '@/api/sense'

const router = useRouter()
const store = useSchedulerStore()

const mainCfg = ref<any>(null)
const rulesCfg = ref<any>(null)
const hpState = ref<HotplugState | null>(null)
const senseRes = ref<SenseResult | null>(null)

let pollTimer: number | null = null
let tickTimer: number | null = null
const tick = ref(0)

onMounted(async () => {
  // 全局模式同步: 尚未加载时拉全量, 已加载则轻量刷新 (问题 3)
  if (!store.modeLoaded) await store.initData()
  else store.refreshMode()
  try { mainCfg.value = await Bridge.getMainConfig() } catch { mainCfg.value = {} }
  try { rulesCfg.value = await Bridge.getRulesConfig() } catch { rulesCfg.value = {} }
  refreshLive()
  pollTimer = window.setInterval(refreshLive, 1500)
  tickTimer = window.setInterval(() => { tick.value++ }, 1000)
})
onUnmounted(() => {
  if (pollTimer !== null) window.clearInterval(pollTimer)
  if (tickTimer !== null) window.clearInterval(tickTimer)
})

async function refreshLive() {
  try { hpState.value = await fetchHotplugState() } catch { /* 保持上次 */ }
  try { senseRes.value = await fetchSenseSnapshot() } catch { /* 保持上次 */ }
}

const snap = computed(() => senseRes.value?.data ?? null)
const onlineCount = computed(() => {
  if (!hpState.value) return null
  let n = 0
  for (let i = 0; i < 8; i++) if (hpState.value.online_mask & (1 << i)) n++
  return n
})

function deepGet(obj: any, path: string): any {
  let cur = obj
  for (const k of path.split('.')) { cur = cur?.[k]; if (cur === undefined) return undefined }
  return cur
}

/** 每张入口卡的当前关键值摘要 */
function summaryOf(key: string): string {
  switch (key) {
    case 'hotplug':
      return onlineCount.value === null ? '读取中…' : `当前在线 ${onlineCount.value}/8 核`
    case 'cpu': {
      const mode = MODE_NAMES[store.currentMode] ?? store.currentMode
      const up = deepGet(mainCfg.value, `${store.currentMode}.cpu_load_governor.up_threshold`)
      const pct = Number.isFinite(Number(up)) ? Math.round(Number(up) * 100) : 80
      return `${mode}档 · 升频阈值 ${pct}%`
    }
    case 'gpu': {
      const g = deepGet(mainCfg.value, 'modules.gpu.screen_on')
      const max = Number(g?.max_pct ?? 100)
      const boost = Number(g?.boost_util_pct ?? 0)
      return boost > 0 ? `上限 ${max}% · 加速 ${boost}%` : `上限 ${max}% · 亮/息屏双套`
    }
    case 'touch': {
      const t = deepGet(mainCfg.value, 'modules.touch.screen_on')
      const en = t?.enabled !== false
      const extra = Number(t?.extra_cores ?? 8)
      return en ? `开启 · 唤醒 +${extra} 核` : '已关闭 · 亮/息屏双套'
    }
    case 'frame': {
      const gears = deepGet(rulesCfg.value, 'fas_rules.fps_gears')
      return Array.isArray(gears) && gears.length ? `目标档位 ${gears.join('/')}` : '档位待读取'
    }
    case 'io': {
      const v = String(deepGet(mainCfg.value, 'IO_Settings.Scheduler') ?? '')
      const opt = IO_PARAMS[0].options?.find(o => o.v === v)
      return opt ? `算法 ${opt.n}` : '算法保持内核默认'
    }
    case 'swap': {
      const s = deepGet(mainCfg.value, 'modules.swap.screen_on')
      const sw = Number(s?.swappiness ?? 100)
      const mb = snap.value?.swap_used_mb
      return `倾向 ${sw} · 交换 ${mb !== undefined ? mb + ' MB' : '--'}`
    }
    case 'temp': {
      const t = senseRes.value?.data.temp_c ?? 0
      const hard = deepGet(mainCfg.value, '') // 占位 (temp 双套在 hotplug config, 摘要用实测温度)
      void hard
      return t > 0 ? `当前 ${t.toFixed(1)}°C · 双阈值` : '温度待读取'
    }
    default: return ''
  }
}

/** 最近一次保存反馈 (全局 store, 所有页面共享) */
const saveHint = computed(() => {
  void tick.value
  if (store.lastSaveOk === null) return ''
  if (Date.now() - store.lastSavedAt > 60_000) return ''
  return store.lastSaveOk ? '最近保存:成功' : '最近保存:失败'
})
</script>

<template>
  <div class="cfg-page">
    <header class="page-head">
      <span class="page-title">调度设置</span>
      <span class="page-sub">
        当前档位:{{ MODE_NAMES[store.currentMode] ?? store.currentMode }}
        <span v-if="saveHint" class="save-hint" :class="{ bad: !store.lastSaveOk }">· {{ saveHint }}</span>
      </span>
    </header>

    <div class="mod-list">
      <button
        v-for="m in MODULES"
        :key="m.key"
        class="mod-card"
        :style="{ borderLeft: `4px solid ${m.color}` }"
        @click="router.push(m.route)"
      >
        <span class="mod-icon" :style="{ background: m.color + '1a', color: m.color }">
          <van-icon :name="m.icon" size="22" />
        </span>
        <span class="mod-main">
          <span class="mod-name">{{ m.name }}<i class="mod-tag" :style="{ color: m.color }">{{ m.tag }}</i></span>
          <span class="mod-brief">{{ m.brief }}</span>
          <span class="mod-summary">{{ summaryOf(m.key) }}</span>
        </span>
        <van-icon name="arrow" color="#c3c8d0" />
      </button>
    </div>

    <footer class="foot">所有修改写入设备后由守护进程热加载, 无需重启</footer>
  </div>
</template>

<style scoped>
.cfg-page { padding: 16px; max-width: 600px; margin: 0 auto; }
.page-head { display: flex; flex-direction: column; gap: 2px; padding: 6px 4px 14px; }
.page-title { font-size: 22px; font-weight: 700; }
.page-sub { font-size: 12.5px; color: var(--text-muted); }
.save-hint { color: var(--success); }
.save-hint.bad { color: var(--danger); }

.mod-list { display: flex; flex-direction: column; gap: 10px; }
.mod-card {
  display: flex; align-items: center; gap: 12px;
  width: 100%; text-align: left;
  background: var(--bg-card); border: 1px solid var(--border);
  border-radius: 12px; padding: 13px 12px;
  transition: transform .12s;
}
.mod-card:active { transform: scale(.985); background: var(--bg-card-hover); }
.mod-icon {
  width: 44px; height: 44px; border-radius: 11px; flex-shrink: 0;
  display: flex; align-items: center; justify-content: center;
}
.mod-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.mod-name { font-size: 15px; font-weight: 700; color: var(--text-primary); display: flex; align-items: center; gap: 8px; }
.mod-tag { font-size: 10.5px; font-weight: 500; font-style: normal; opacity: .85; }
.mod-brief { font-size: 12px; color: var(--text-secondary); line-height: 1.45; }
.mod-summary {
  font-size: 12.5px; font-weight: 600; color: var(--accent);
  font-variant-numeric: tabular-nums; margin-top: 2px;
}
.foot { text-align: center; font-size: 11px; color: var(--text-muted); padding: 14px 0 8px; }
</style>
