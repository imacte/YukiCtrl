<!--
  src/views/HomeView.vue — 首页 (感知面板合并版)

  布局 (从上到下):
    1. 页头:    标题 + 实时链路指示 + 守护进程状态
    2. 模式:    省电 / 均衡 / 性能 / 极速 四按钮
    3. 核心:    8 宫格 (在线/离线/高负载 + 每核利用率细条)
    4. 感知:    八路数据卡 (温度/当前应用/GPU/内存/IO/触摸/帧率/刷新率)

  历史: 原 SensePanel.vue 已并入本页并删除, /sense 路由重定向回 /.
-->
<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useSchedulerStore } from '@/stores/scheduler'
import { fetchSenseSnapshot, type SenseResult } from '@/api/sense'
import { Bridge } from '@/utils/bridge'
import { fetchHotplugState, maskToCpuArray, type HotplugState } from '@/api/hotplug'
import HelpTooltip from '@/components/HelpTooltip.vue'
import { MODE_HELP } from '@/config/moduleSpecs'

const router = useRouter()
const store = useSchedulerStore()

const senseRes = ref<SenseResult | null>(null)
const hotplug = ref<HotplugState | null>(null)
const daemonUp = ref(true)
const tick = ref(0) // 每秒自增, 驱动链路新鲜度 recompute

let snapTimer: number | null = null
let hotTimer: number | null = null
let daemonTimer: number | null = null
let tickTimer: number | null = null

const refreshSnap = async () => {
  try { senseRes.value = await fetchSenseSnapshot() } catch { /* 保持上次 */ }
}
const refreshHot = async () => {
  try { hotplug.value = await fetchHotplugState() } catch { /* 保持上次 */ }
}
const refreshDaemon = async () => {
  try { daemonUp.value = await Bridge.isDaemonRunning() } catch { /* 保持上次 */ }
}

onMounted(() => {
  store.initData()
  refreshSnap()
  refreshHot()
  refreshDaemon()
  snapTimer = window.setInterval(refreshSnap, 500)
  hotTimer = window.setInterval(refreshHot, 1000)
  daemonTimer = window.setInterval(refreshDaemon, 3000)
  tickTimer = window.setInterval(() => { tick.value++ }, 1000)
})
onUnmounted(() => {
  for (const t of [snapTimer, hotTimer, daemonTimer, tickTimer]) {
    if (t !== null) window.clearInterval(t)
  }
})

/* ---------- 实时链路 ---------- */
const linkFreshMs = computed(() => {
  void tick.value
  if (!senseRes.value?.ok) return -1
  return Date.now() - (senseRes.value.data.updated_at_unix_ms || 0)
})
const linkOk = computed(() => linkFreshMs.value >= 0 && linkFreshMs.value < 5000)
const linkTitle = computed(() =>
  linkOk.value ? `数据新鲜 (${Math.max(0, linkFreshMs.value)}ms 前)` : (senseRes.value?.err || '尚未读到快照'))

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

/* ---------- 2. 核心 8 宫格 (含每核利用率条) ---------- */
interface CoreCell { id: number; label: string; online: boolean; util: number; high: boolean }
const HIGH_LOAD_PCT = 70
const cores = computed<CoreCell[]>(() => {
  const onlineArr = hotplug.value ? maskToCpuArray(hotplug.value.online_mask) : Array(8).fill(true)
  const utils = senseRes.value?.data.cpu_utils_pct ?? Array(8).fill(0)
  return Array.from({ length: 8 }, (_, i) => ({
    id: i,
    label: `核心${i + 1}`,
    online: onlineArr[i],
    util: Math.round(utils[i] ?? 0),
    high: (utils[i] ?? 0) >= HIGH_LOAD_PCT,
  }))
})
function utilBar(v: number) {
  if (!v) return 'var(--border)'
  if (v > 80) return 'var(--danger)'
  if (v > 50) return 'var(--warning)'
  return 'var(--accent)'
}
const keepList = computed<Set<number>>(() => {
  const raw = hotplug.value?.active_keep_cores ?? ''
  if (!raw) return new Set<number>()
  return new Set(raw.split(',').map(s => parseInt(s.trim(), 10)).filter(n => !Number.isNaN(n)))
})
const screenOn = computed(() => senseRes.value?.data.screen_on ?? hotplug.value?.screen_on ?? true)

/* ---------- 3. 八路感知指标 ---------- */
const snap = computed(() => senseRes.value?.data ?? null)
function barColor(v: number) {
  if (v > 80) return 'var(--danger)'
  if (v > 50) return 'var(--warning)'
  return 'var(--accent)'
}
const tempText = computed(() => {
  const t = snap.value?.temp_c ?? 0
  return t > 0 ? t.toFixed(1) : '--'
})
const tempColor = computed(() => {
  const t = snap.value?.temp_c ?? 0
  if (t <= 0) return 'var(--text-muted)'
  if (t > 65) return 'var(--danger)'
  if (t > 55) return 'var(--warning)'
  return 'var(--success)'
})
/** 包名去前缀展示 */
function shortPkg(p: string): string {
  if (!p) return ''
  return p.startsWith('com.') ? p.slice(4) : p
}
const appText = computed(() => shortPkg(snap.value?.current_pkg || '') || '未识别')
const gpuVal = computed(() => snap.value?.gpu_load_pct ?? 0)
const gpuText = computed(() => gpuVal.value > 0 ? gpuVal.value.toFixed(0) + '%' : '--')
const memPct = computed(() => Math.round(snap.value?.mem_full_pct ?? 0))
const swapMb = computed(() => snap.value?.swap_used_mb ?? 0)
const ioSome = computed(() => snap.value?.io_some_pct ?? 0)
const ioFull = computed(() => snap.value?.io_full_pct ?? 0)
const fpsText = computed(() => {
  const f = snap.value?.fps ?? 0
  return f > 0 ? String(f) : '--'
})
const hzText = computed(() => {
  const h = snap.value?.display_hz ?? 0
  return h > 0 ? Math.round(h) + '' : '--'
})
const touchDown = computed(() => !!snap.value?.touch_down)
const touchAge = computed(() => snap.value?.touch_age_ms ?? 9999)
const ageColor = computed(() => {
  if (touchDown.value) return 'var(--success)'
  if (touchAge.value < 1000) return 'var(--warning)'
  return 'var(--text-muted)'
})
</script>
<template>
  <div class="home-page">
    <!-- 页头 -->
    <div class="page-head">
      <div class="head-row">
        <span class="app-name">核心领航员</span>
        <span class="live-pill" :class="{ bad: !linkOk }" :title="linkTitle">
          <i class="ldot" :class="{ ok: linkOk, bad: !linkOk }"></i>{{ linkOk ? '实时' : '断开' }}
        </span>
      </div>
      <div class="daemon-line" @click="router.push('/log')">
        <span class="dot" :class="daemonUp ? 'ok' : 'down'"></span>
        {{ daemonUp ? '调度守护进程运行中' : '守护进程未响应 · 点看日志' }}
      </div>
    </div>

    <!-- 1. 模式切换 (带五维说明, 问题 1) -->
    <div class="section-row">
      <span class="section-title">调度模式</span>
      <HelpTooltip title="调度模式" :list="MODE_HELP[currentMode] ?? MODE_HELP['balance']" />
    </div>
    <div class="mode-grid">
      <button v-for="m in modes" :key="m.key" class="mode-btn"
        :class="{ active: m.key === currentMode }"
        @click="pickMode(m.key)">
        <van-icon :name="m.icon" size="20" :style="{ color: m.key === currentMode ? m.color : undefined }" />
        <span class="mode-name">{{ m.name }}</span>
        <span class="mode-desc">{{ m.desc }}</span>
      </button>
    </div>

    <!-- 2. 核心八宫格 -->
    <div class="card">
      <div class="card-title-row">
        <span class="card-title">核心八宫格</span>
        <span class="screen-tag">{{ screenOn ? '亮屏' : '熄屏' }}</span>
      </div>
      <div class="core-grid">
        <div v-for="c in cores" :key="c.id"
          class="core-cell" :class="{ hot: c.high && c.online, offline: !c.online }">
          <span v-if="keepList.has(c.id)" class="keep-flag">保</span>
          <div class="core-label">{{ c.label }}</div>
          <div class="core-state">{{ c.online ? c.util + '%' : 'off' }}</div>
          <div class="util-bar"><i :style="{ width: c.online ? c.util + '%' : '0%', background: utilBar(c.util) }"></i></div>
        </div>
      </div>
      <div class="legend">
        <span><i class="lg lg-on"></i>在线</span>
        <span><i class="lg lg-hot"></i>≥70%</span>
        <span><i class="lg lg-off"></i>离线</span>
        <span><i class="keep-demo">保</i>=常驻核心</span>
      </div>
    </div>

    <!-- 3. 八路感知 -->
    <div class="grid2">
      <div class="card metric-card">
        <div class="metric-label">芯片温度</div>
        <div class="metric-value" :style="{ color: tempColor }">{{ tempText }}<span class="unit">℃</span></div>
      </div>
      <div class="card metric-card">
        <div class="metric-label">当前前台应用</div>
        <div class="app-value" :title="snap?.current_pkg">{{ appText }}</div>
      </div>
      <div class="card metric-card">
        <div class="metric-label">显卡负载</div>
        <div class="metric-value sm">{{ gpuText }}</div>
        <div class="bar-h"><i :style="{ width: gpuVal + '%', background: barColor(gpuVal) }"></i></div>
      </div>
      <div class="card metric-card">
        <div class="metric-label">内存压力</div>
        <div class="metric-value sm">{{ memPct }}<span class="unit">%</span></div>
        <div class="bar-h"><i :style="{ width: memPct + '%', background: barColor(memPct) }"></i></div>
        <div class="metric-sub">交换 {{ swapMb }} MB</div>
      </div>

      <div class="card metric-card">
        <div class="metric-label">读写压力 (IO)</div>
        <div class="bar-row"><span>some</span><div class="bar-h slim"><i :style="{ width: ioSome + '%', background: 'var(--accent)' }"></i></div><b>{{ ioSome.toFixed(1) }}</b></div>
        <div class="bar-row"><span>full</span><div class="bar-h slim"><i :style="{ width: ioFull + '%', background: barColor(ioFull) }"></i></div><b>{{ ioFull.toFixed(1) }}</b></div>
      </div>
      <div class="card metric-card">
        <div class="metric-label">触摸状态</div>
        <div class="touch-line">
          <span class="pill" :class="touchDown ? 'on' : ''">{{ touchDown ? '按下' : '抬起' }}</span>
          <span class="touch-age" :style="{ color: ageColor }">{{ touchAge }}ms</span>
        </div>
      </div>

      <div class="card metric-card">
        <div class="metric-label">帧率 (实测出帧)</div>
        <div class="metric-value sm">{{ fpsText }}<span class="unit" v-if="fpsText !== '--'">FPS</span></div>
      </div>
      <div class="card metric-card">
        <div class="metric-label">屏幕刷新率</div>
        <div class="metric-value sm">{{ hzText }}<span class="unit" v-if="hzText !== '--'">Hz</span></div>
      </div>
    </div>

    <div class="footer-hint">数据 0.5s 刷新 · sense/snapshot.yaml</div>
  </div>
</template>
<style scoped>
.home-page { padding: 14px 14px 24px; max-width: 600px; margin: 0 auto; }
.page-head { padding: 2px 4px 12px; }
.head-row { display: flex; align-items: center; justify-content: space-between; }
.app-name { font-size: 22px; font-weight: 700; }
.live-pill { display: inline-flex; align-items: center; gap: 5px; font-size: 12px; color: var(--text-secondary); background: var(--bg-card); border: 1px solid var(--border); border-radius: 999px; padding: 3px 10px; }
.ldot { width: 7px; height: 7px; border-radius: 50%; display: inline-block; }
.ldot.ok { background: var(--success); box-shadow: 0 0 6px rgba(16,185,129,.7); animation: pulse 1.6s infinite; }
.ldot.bad { background: var(--danger); }
@keyframes pulse { 0%,100% { opacity: 1 } 50% { opacity: .45 } }
.daemon-line { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-secondary); margin-top: 6px; cursor: pointer; }
.dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
.dot.ok { background: var(--success); box-shadow: 0 0 6px rgba(16,185,129,.6); }
.dot.down { background: var(--danger); }

.card { background: var(--bg-card); border: 1px solid var(--border); border-radius: 14px; padding: 14px; margin-bottom: 12px; }
.metric-card { margin-bottom: 0; min-height: 92px; display: flex; flex-direction: column; }

.section-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 4px 8px;
}
.section-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--text-secondary);
}
.mode-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 10px; margin-bottom: 12px; }
.mode-btn { border: 2px solid var(--border); background: var(--bg-base); border-radius: 12px; padding: 13px 6px 11px; display: flex; flex-direction: column; align-items: center; gap: 4px; transition: transform .1s; color: var(--text-muted); }
.mode-btn:active { transform: scale(.96); }
.mode-btn.active { background: rgba(59,130,246,.10); }
.mode-btn .mode-name { font-size: 17px; font-weight: 600; color: var(--text-primary); }
.mode-btn .mode-desc { font-size: 11px; color: var(--text-muted); text-align: center; line-height: 1.3; }

.card-title-row { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
.card-title { font-size: 15px; font-weight: 600; }
.screen-tag { font-size: 11px; color: var(--accent); background: rgba(59,130,246,.12); padding: 2px 10px; border-radius: 10px; }

.core-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; }
.core-cell { position: relative; background: var(--success-soft); border: 1.5px solid var(--success); border-radius: 10px; padding: 9px 4px 8px; text-align: center; transition: all .25s; }
.core-cell.hot { background: var(--warning-soft); border-color: var(--warning); }
.core-cell.offline { background: var(--bg-base); border-color: var(--border); opacity: .55; }
.keep-flag { position: absolute; top: 3px; right: 4px; font-size: 9px; line-height: 1; color: #fff; background: var(--accent); padding: 2px 4px; border-radius: 5px; }
.core-label { font-size: 12px; font-weight: 600; color: var(--text-primary); }
.core-state { font-size: 12px; color: var(--text-secondary); margin: 2px 0 5px; font-variant-numeric: tabular-nums; }
.util-bar { height: 4px; border-radius: 2px; background: rgba(0,0,0,.07); overflow: hidden; }
.util-bar i { display: block; height: 100%; border-radius: 2px; transition: width .35s ease; }
.keep-demo { font-style: normal; font-size: 9px; color: #fff; background: var(--accent); padding: 1px 3px; border-radius: 4px; }
.legend { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 10px; font-size: 11px; color: var(--text-muted); align-items: center; }
.legend span { display: inline-flex; align-items: center; gap: 4px; }
.lg { width: 10px; height: 10px; border-radius: 3px; display: inline-block; }
.lg-on { background: var(--success-soft); border: 1.5px solid var(--success); }
.lg-hot { background: var(--warning-soft); border: 1.5px solid var(--warning); }
.lg-off { background: var(--bg-base); border: 1.5px solid var(--border); opacity: .6; }

.grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.metric-label { font-size: 12px; color: var(--text-muted); }
.metric-value { font-size: 28px; font-weight: 700; margin-top: auto; font-variant-numeric: tabular-nums; line-height: 1.15; }
.metric-value.sm { font-size: 22px; }
.metric-value .unit { font-size: 13px; font-weight: 500; color: var(--text-muted); margin-left: 2px; }
.metric-sub { font-size: 11px; color: var(--text-muted); margin-top: 2px; }
.app-value { font-size: 17px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-top: auto; }
.bar-h { height: 7px; border-radius: 4px; background: rgba(0,0,0,.06); overflow: hidden; margin-top: 6px; }
.bar-h i { display: block; height: 100%; border-radius: 4px; transition: width .35s ease; }
.bar-row { display: flex; align-items: center; gap: 8px; font-size: 11px; color: var(--text-secondary); margin-top: 7px; }
.bar-row span { width: 34px; }
.bar-row b { width: 36px; text-align: right; font-variant-numeric: tabular-nums; color: var(--text-primary); }
.bar-h.slim { flex: 1; margin-top: 0; }
.touch-line { display: flex; align-items: center; gap: 8px; margin-top: auto; }
.pill { font-size: 12px; padding: 3px 12px; border-radius: 999px; background: var(--bg-base); border: 1px solid var(--border); color: var(--text-muted); }
.pill.on { background: var(--success-soft); border-color: var(--success); color: var(--text-primary); font-weight: 600; }
.touch-age { font-size: 12px; font-variant-numeric: tabular-nums; }
.footer-hint { text-align: center; font-size: 11px; color: var(--text-muted); margin-top: 14px; }
</style>