<!--
  src/views/HomeView.vue — 首页 (任务 B 彻底重做)

  布局 (从上到下, 无重复看板):
    1. 模式切换大按钮: 省电 / 均衡 / 性能 / 极速
    2. 核心 8 宫格:   在线=绿色, 离线=灰色, 高负载(≥70%)=橙色
    3. 底部:          温度 + 当前应用

  刻意移除 (与感知页/调度页重复): CPU 趋势图、GPU/IO/内存指标卡、快捷入口.
-->
<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useSchedulerStore } from '@/stores/scheduler'
import { fetchSenseSnapshot, type SenseSnapshot } from '@/api/sense'
import { fetchHotplugState, maskToCpuArray, type HotplugState } from '@/api/hotplug'

const router = useRouter()
const store = useSchedulerStore()

const snap = ref<SenseSnapshot | null>(null)
const hotplug = ref<HotplugState | null>(null)

let snapTimer: number | null = null
let hotTimer: number | null = null

const refreshSnap = async () => {
  try { snap.value = await fetchSenseSnapshot() } catch { /* 保持上次数据 */ }
}
const refreshHot = async () => {
  try { hotplug.value = await fetchHotplugState() } catch { /* 保持上次数据 */ }
}

onMounted(() => {
  store.initData()
  refreshSnap()
  refreshHot()
  snapTimer = window.setInterval(refreshSnap, 500)
  hotTimer = window.setInterval(refreshHot, 1000)
})
onUnmounted(() => {
  if (snapTimer !== null) window.clearInterval(snapTimer)
  if (hotTimer !== null) window.clearInterval(hotTimer)
})

/* ---------- 1. 模式切换 ---------- */
interface ModeItem { key: string; name: string; desc: string; icon: string; color: string }
const modes: ModeItem[] = [
  { key: 'powersave',   name: '省电', desc: '限制频率延长续航',     icon: 'leaf-o',    color: '#10b981' },
  { key: 'balance',     name: '均衡', desc: '日常使用平衡功耗性能', icon: 'balance-o', color: '#3b82f6' },
  { key: 'performance', name: '性能', desc: '游戏重负载优先流畅',   icon: 'fire-o',    color: '#f59e0b' },
  { key: 'fast',        name: '极速', desc: '全力释放不计功耗',     icon: 'upgrade',   color: '#ef4444' },
]
const currentMode = computed(() => store.currentMode || 'balance')
const switching = ref(false)
const pickMode = async (key: string) => {
  if (switching.value || key === currentMode.value) return
  switching.value = true
  try { await store.switchMode(key) } finally { switching.value = false }
}

/* ---------- 2. 核心 8 宫格 ---------- */
interface CoreCell { id: number; label: string; online: boolean; util: number; high: boolean }
const HIGH_LOAD_PCT = 70
const cores = computed<CoreCell[]>(() => {
  const onlineArr = hotplug.value ? maskToCpuArray(hotplug.value.online_mask) : Array(8).fill(true)
  const utils = snap.value?.cpu_utils_pct ?? Array(8).fill(0)
  return Array.from({ length: 8 }, (_, i) => ({
    id: i,
    label: `核心${i + 1}`,
    online: onlineArr[i],
    util: Math.round(utils[i] ?? 0),
    high: (utils[i] ?? 0) >= HIGH_LOAD_PCT,
  }))
})
const keepList = computed<Set<number>>(() => {
  const raw = hotplug.value?.active_keep_cores ?? ''
  if (!raw) return new Set<number>()
  return new Set(raw.split(',').map(s => parseInt(s.trim(), 10)).filter(n => !Number.isNaN(n)))
})
const screenOn = computed(() => hotplug.value?.screen_on ?? true)

/* ---------- 3. 温度 + 当前应用 ---------- */
const tempText = computed(() => {
  const t = snap.value?.temp_c ?? 0
  return t > 0 ? t.toFixed(1) : '--'
})
const tempColor = computed(() => {
  const t = snap.value?.temp_c ?? 0
  if (t === 0) return 'var(--text-muted)'
  if (t > 65) return 'var(--danger)'
  if (t > 55) return 'var(--warning)'
  return 'var(--success)'
})
/** 包名去前缀展示 (com.tencent.xxx → xxx), 更易读 */
const appShort = computed(() => {
  const pkg = snap.value?.current_pkg ?? ''
  if (!pkg) return '未检测到前台应用'
  return pkg.split('.').pop() || pkg
})

const goSchedule = () => router.push('/config')
</script>
<template>
  <div class="home">
    <!-- 页头 -->
    <header class="page-head">
      <div class="app-name">核心领航员</div>
      <div class="daemon-line">
        <span class="dot" :class="store.isDaemonRunning ? 'ok' : 'down'"></span>
        {{ store.isDaemonRunning ? '守护进程运行中' : '守护进程未运行' }}
      </div>
    </header>

    <!-- 1. 模式切换 -->
    <section class="card mode-card">
      <div class="card-title-row">
        <span class="card-title">运行模式</span>
        <button class="mini-link" @click="goSchedule">详细设置 ›</button>
      </div>
      <div class="mode-grid">
        <button
          v-for="m in modes"
          :key="m.key"
          class="mode-btn"
          :class="{ active: m.key === currentMode }"
          @click="pickMode(m.key)"
        >
          <van-icon :name="m.icon" size="26" />
          <span class="mode-name">{{ m.name }}</span>
          <span class="mode-desc">{{ m.desc }}</span>
        </button>
      </div>
    </section>

    <!-- 2. 核心 8 宫格 -->
    <section class="card">
      <div class="card-title-row">
        <span class="card-title">核心状态</span>
        <span class="screen-tag">{{ screenOn ? '亮屏' : '息屏' }}</span>
      </div>
      <div class="core-grid">
        <div
          v-for="c in cores"
          :key="c.id"
          class="core-cell"
          :class="{ offline: !c.online, hot: c.online && c.high }"
        >
          <span v-if="keepList.has(c.id)" class="keep-flag">保</span>
          <div class="core-label">{{ c.label }}</div>
          <div class="core-state">{{ c.online ? c.util + '%' : '离线' }}</div>
        </div>
      </div>
      <div class="legend">
        <span><i class="lg lg-on"></i>在线</span>
        <span><i class="lg lg-hot"></i>高负载</span>
        <span><i class="lg lg-off"></i>离线</span>
        <span>「保」= 当前保留核心(永不关闭)</span>
      </div>
    </section>

    <!-- 3. 温度 + 当前应用 -->
    <section class="bottom-grid">
      <div class="card bottom-card">
        <div class="metric-label">处理器温度</div>
        <div class="metric-value" :style="{ color: tempColor }">
          {{ tempText }}<span class="unit">°C</span>
        </div>
      </div>
      <div class="card bottom-card">
        <div class="metric-label">当前应用</div>
        <div class="metric-value app-value">{{ appShort }}</div>
      </div>
    </section>
  </div>
</template>
<style scoped>
.home { padding: 16px; max-width: 600px; margin: 0 auto; }
.page-head { padding: 6px 4px 14px; }
.app-name { font-size: 22px; font-weight: 700; }
.daemon-line { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-secondary); margin-top: 4px; }
.dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
.dot.ok { background: var(--success); box-shadow: 0 0 6px rgba(16,185,129,.6); }
.dot.down { background: var(--danger); }

.card {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 14px;
  padding: 14px;
  margin-bottom: 12px;
}
.card-title-row { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
.card-title { font-size: 15px; font-weight: 600; }
.mini-link { border: none; background: none; color: var(--accent); font-size: 13px; padding: 4px; }

/* 模式 2x2 大按钮 */
.mode-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 10px; }
.mode-btn {
  border: 2px solid var(--border);
  background: var(--bg-base);
  border-radius: 12px;
  padding: 14px 6px 12px;
  display: flex; flex-direction: column; align-items: center; gap: 4px;
  transition: transform .1s;
  color: var(--text-muted);
}
.mode-btn:active { transform: scale(.96); }
.mode-btn.active { background: rgba(59,130,246,.10); }
.mode-btn .mode-name { font-size: 17px; font-weight: 600; color: var(--text-primary); }
.mode-btn .mode-desc { font-size: 11px; color: var(--text-muted); text-align: center; line-height: 1.3; }

.screen-tag {
  font-size: 11px; color: var(--accent);
  background: rgba(59,130,246,.12);
  padding: 2px 10px; border-radius: 10px;
}

/* 核心 8 宫格 */
.core-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; }
.core-cell {
  position: relative;
  background: var(--success-soft);
  border: 1.5px solid var(--success);
  border-radius: 10px;
  padding: 10px 2px 8px;
  text-align: center;
  transition: all .25s;
}
.core-cell.hot { background: var(--warning-soft); border-color: var(--warning); }
.core-cell.offline { background: var(--bg-base); border-color: var(--border); opacity: .55; }
.keep-flag {
  position: absolute; top: 3px; right: 5px;
  font-size: 9px; line-height: 1;
  color: #fff; background: var(--accent);
  padding: 2px 4px; border-radius: 5px;
}
.core-label { font-size: 12px; font-weight: 600; color: var(--text-primary); }
.core-state { font-size: 12px; color: var(--text-secondary); margin-top: 3px; font-variant-numeric: tabular-nums; }
.legend { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 10px; font-size: 11px; color: var(--text-muted); align-items: center; }
.legend span { display: inline-flex; align-items: center; gap: 4px; }
.lg { width: 10px; height: 10px; border-radius: 3px; display: inline-block; }
.lg-on { background: var(--success-soft); border: 1.5px solid var(--success); }
.lg-hot { background: var(--warning-soft); border: 1.5px solid var(--warning); }
.lg-off { background: var(--bg-base); border: 1.5px solid var(--border); opacity: .6; }

/* 底部 温度 / 当前应用 */
.bottom-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.bottom-card { margin-bottom: 0; min-height: 84px; }
.metric-label { font-size: 12px; color: var(--text-muted); }
.metric-value { font-size: 28px; font-weight: 700; margin-top: 6px; font-variant-numeric: tabular-nums; line-height: 1.1; }
.metric-value .unit { font-size: 13px; font-weight: 500; color: var(--text-muted); margin-left: 2px; }
.app-value { font-size: 18px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
