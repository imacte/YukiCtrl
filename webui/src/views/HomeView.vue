<!--
<!--
  src/views/HomeView.vue

  核心领航员 — 首页 (亮色, 卡片化, 大白话)
  实时状态总览: 当前模式 / CPU 8 核 / 温度 / 触摸 / 触摸指示器 / 快捷入口
-->
  src/views/HomeView.vue

  \u6838\u5fc3\u9886\u822a\u5458 \u2014 \u5b9e\u65f6\u603b\u89c8\u4eea\u8868\u76d8 (\u6697\u8272)
-->
<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed } from 'vue'
import { useSchedulerStore } from '@/stores/scheduler'
import { useI18n } from 'vue-i18n'
import { fetchSenseSnapshot, type SenseSnapshot } from '@/api/sense'
import { fetchHotplugState, maskToCpuArray, type HotplugState } from '@/api/hotplug'

const store = useSchedulerStore()
const { t, locale } = useI18n()

const snap = ref<SenseSnapshot | null>(null)
const hotplug = ref<HotplugState | null>(null)
const startedAt = ref(Date.now())
const uptime = ref('0\u79d2')

let snapTimer: number | null = null
let hotTimer: number | null = null
let uptimeTimer: number | null = null
let cpuHistTimer: number | null = null

const toggleLanguage = () => {
  const newLang = locale.value === 'zh' ? 'en' : 'zh'
  locale.value = newLang
  localStorage.setItem('app_lang', newLang)
}

const modes = computed(() => [
  { key: 'powersave', name: t('mode_powersave'), desc: t('desc_powersave'), icon: 'shield-o', color: '#10b981' },
  { key: 'balance', name: t('mode_balance'), desc: t('desc_balance'), icon: 'balance-o', color: '#4c9aff' },
  { key: 'performance', name: t('mode_performance'), desc: t('desc_performance'), icon: 'fire', color: '#f59e0b' },
  { key: 'fast', name: t('mode_fast'), desc: t('desc_fast'), icon: 'upgrade', color: '#ef4444' },
])

const currentMode = computed(() =>
  modes.value.find(m => m.key === store.currentMode) || modes.value[1]
)

const cpuBars = computed(() => snap.value?.cpu_utils_pct ?? Array(8).fill(0))
const cpuCores = computed(() => hotplug.value ? maskToCpuArray(hotplug.value.online_mask) : Array(8).fill(true))

const cpuHistory = ref<number[]>([])
const CPU_HIST_MAX = 60

const pushCpuHistory = () => {
  if (!snap.value) return
  const avg = cpuBars.value.reduce((a, b) => a + b, 0) / 8
  cpuHistory.value.push(avg)
  if (cpuHistory.value.length > CPU_HIST_MAX) cpuHistory.value.shift()
}

const trendPath = computed(() => {
  if (cpuHistory.value.length < 2) return ''
  const w = 320, h = 60
  const step = w / (CPU_HIST_MAX - 1)
  return cpuHistory.value.map((v, i) => {
    const x = i * step
    const y = h - (v / 100) * h
    return `${i === 0 ? 'M' : 'L'} ${x.toFixed(1)} ${y.toFixed(1)}`
  }).join(' ')
})

const trendFill = computed(() => {
  const p = trendPath.value
  if (!p) return ''
  return `${p} L 320 60 L 0 60 Z`
})

const tempColor = computed(() => {
  const tv = snap.value?.temp_c ?? 0
  if (tv === 0) return 'var(--text-muted)'
  if (tv > 65) return 'var(--danger)'
  if (tv > 55) return 'var(--warning)'
  return 'var(--success)'
})

const gpuColor = computed(() => {
  const g = snap.value?.gpu_load_pct ?? 0
  if (g > 80) return 'var(--danger)'
  if (g > 50) return 'var(--warning)'
  return 'var(--accent)'
})

const ioColor = computed(() => {
  const v = snap.value?.io_full_pct ?? 0
  if (v > 30) return 'var(--danger)'
  if (v > 15) return 'var(--warning)'
  return 'var(--accent)'
})

const swapColor = computed(() => {
  const v = snap.value?.mem_full_pct ?? 0
  if (v > 30) return 'var(--danger)'
  if (v > 15) return 'var(--warning)'
  return 'var(--accent)'
})

const avgCpu = computed(() => {
  if (!snap.value) return 0
  return snap.value.cpu_utils_pct.reduce((a, b) => a + b, 0) / 8
})

const fpsDisplay = computed(() => {
  const f = snap.value?.fps ?? 0
  return f > 0 ? `${f}` : '\u2014'
})

const handleModeSelect = async (modeKey: string) => {
  await store.switchMode(modeKey)
}

onMounted(async () => {
  await store.initData()
  snapTimer = window.setInterval(async () => {
    try { snap.value = await fetchSenseSnapshot() } catch {}
  }, 1000)
  hotTimer = window.setInterval(async () => {
    try { hotplug.value = await fetchHotplugState() } catch {}
  }, 500)
  cpuHistTimer = window.setInterval(pushCpuHistory, 1000)
  uptimeTimer = window.setInterval(() => {
    const ms = Date.now() - startedAt.value
    const s = Math.floor(ms / 1000)
    if (s < 60) uptime.value = `${s}\u79d2`
    else if (s < 3600) uptime.value = `${Math.floor(s / 60)}\u5206${s % 60}\u79d2`
    else uptime.value = `${Math.floor(s / 3600)}\u65f6${Math.floor((s % 3600) / 60)}\u5206`
  }, 1000)
})

onUnmounted(() => {
  if (snapTimer) clearInterval(snapTimer)
  if (hotTimer) clearInterval(hotTimer)
  if (cpuHistTimer) clearInterval(cpuHistTimer)
  if (uptimeTimer) clearInterval(uptimeTimer)
})
</script>
<template>
  <div class="dashboard">
    <div class="hero">
      <div class="hero-text">
        <div class="brand">\u6838\u5fc3\u9886\u822a\u5458</div>
        <div class="subtitle">\u516b\u8def\u611f\u77e5 \u00b7 \u667a\u80fd\u8c03\u5ea6 \u00b7 \u52a8\u6001\u5173\u6838</div>
      </div>
      <button class="lang-btn" @click="toggleLanguage">
        {{ locale === 'zh' ? 'EN' : '\u4e2d' }}
      </button>
    </div>

    <div class="status-strip">
      <div class="status-cell" :class="{ ok: store.isDaemonRunning }">
        <span class="dot" :class="{ ok: store.isDaemonRunning }"></span>
        <span>{{ store.isDaemonRunning ? '\u8fd0\u884c\u4e2d' : '\u672a\u8fd0\u884c' }}</span>
      </div>
      <div class="status-cell">
        <span class="muted">\u8fd0\u884c</span>
        <span>{{ uptime }}</span>
      </div>
      <div class="status-cell" :style="{ color: tempColor }">
        <span class="muted">\u6e29\u5ea6</span>
        <span>{{ (snap?.temp_c ?? 0).toFixed(1) }}&deg;C</span>
      </div>
    </div>

    <div class="card mode-card" :style="{ borderColor: currentMode.color + '40' }">
      <div class="mode-icon" :style="{ background: currentMode.color + '20', color: currentMode.color }">
        <van-icon :name="currentMode.icon" size="28" />
      </div>
      <div class="mode-info">
        <div class="mode-label">\u5f53\u524d\u6a21\u5f0f</div>
        <div class="mode-name" :style="{ color: currentMode.color }">{{ currentMode.name }}</div>
      </div>
      <router-link to="/config" class="mode-link">
        \u5207\u6362
        <van-icon name="arrow" size="14" />
      </router-link>
    </div>

    <div class="card">
      <div class="card-header">
        <span class="card-title">8 \u6838\u72b6\u6001</span>
        <span class="card-meta">{{ cpuCores.filter(Boolean).length }}/8 \u5728\u7ebf</span>
      </div>
      <div class="cpu-grid">
        <div v-for="(online, i) in cpuCores" :key="i"
             class="cpu-cell"
             :class="{ offline: !online, hot: cpuBars[i] > 80, warm: cpuBars[i] > 50 && cpuBars[i] <= 80 }">
          <div class="cpu-id">cpu{{ i }}</div>
          <div class="cpu-bar-wrap">
            <div class="cpu-bar" :style="{ height: cpuBars[i] + '%' }"></div>
          </div>
          <div class="cpu-load">{{ Math.round(cpuBars[i]) }}</div>
        </div>
      </div>
    </div>

    <div class="metrics">
      <div class="metric">
        <div class="metric-label">CPU \u5e73\u5747</div>
        <div class="metric-value" :style="{ color: avgCpu > 80 ? 'var(--danger)' : avgCpu > 50 ? 'var(--warning)' : 'var(--accent)' }">
          {{ avgCpu.toFixed(0) }}<span class="metric-unit">%</span>
        </div>
        <div class="metric-bar">
          <div class="metric-bar-fill" :style="{ width: avgCpu + '%', background: avgCpu > 80 ? 'var(--danger)' : avgCpu > 50 ? 'var(--warning)' : 'var(--accent)' }"></div>
        </div>
      </div>

      <div class="metric">
        <div class="metric-label">GPU \u8d1f\u8f7d</div>
        <div class="metric-value" :style="{ color: gpuColor }">
          {{ (snap?.gpu_load_pct ?? 0).toFixed(0) }}<span class="metric-unit">%</span>
        </div>
        <div class="metric-bar">
          <div class="metric-bar-fill" :style="{ width: (snap?.gpu_load_pct ?? 0) + '%', background: gpuColor }"></div>
        </div>
      </div>

      <div class="metric">
        <div class="metric-label">IO \u538b\u529b</div>
        <div class="metric-value" :style="{ color: ioColor }">
          {{ (snap?.io_full_pct ?? 0).toFixed(1) }}<span class="metric-unit">%</span>
        </div>
        <div class="metric-bar">
          <div class="metric-bar-fill" :style="{ width: Math.min(100, (snap?.io_full_pct ?? 0) * 3) + '%', background: ioColor }"></div>
        </div>
      </div>
      <div class="metric">
        <div class="metric-label">\u5185\u5b58</div>
        <div class="metric-value" :style="{ color: swapColor }">
          {{ (snap?.mem_full_pct ?? 0).toFixed(1) }}<span class="metric-unit">%</span>
        </div>
        <div class="metric-bar">
          <div class="metric-bar-fill" :style="{ width: Math.min(100, (snap?.mem_full_pct ?? 0) * 3) + '%', background: swapColor }"></div>
        </div>
      </div>

      <div class="metric">
        <div class="metric-label">FPS</div>
        <div class="metric-value" style="color: var(--accent)">{{ fpsDisplay }}</div>
        <div class="metric-sub">{{ snap?.screen_on ? '\u4eae\u5c4f' : '\u606f\u5c4f' }}</div>
      </div>

      <div class="metric">
        <div class="metric-label">\u6e29\u5ea6</div>
        <div class="metric-value" :style="{ color: tempColor }">
          {{ (snap?.temp_c ?? 0).toFixed(1) }}<span class="metric-unit">&deg;C</span>
        </div>
        <div class="metric-sub">{{ snap?.touch_down ? '\u89e6\u6478\u4e2d' : '\u9759\u6b62' }}</div>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <span class="card-title">CPU \u8d1f\u8f7d\u8d8b\u52bf</span>
        <span class="card-meta">\u8fd1 60 \u79d2</span>
      </div>
      <svg class="trend-chart" viewBox="0 0 320 60" preserveAspectRatio="none">
        <defs>
          <linearGradient id="trend-grad" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color="#4c9aff" stop-opacity="0.5"/>
            <stop offset="100%" stop-color="#4c9aff" stop-opacity="0"/>
          </linearGradient>
        </defs>
        <path v-if="trendFill" :d="trendFill" fill="url(#trend-grad)" />
        <path v-if="trendPath" :d="trendPath" fill="none" stroke="#4c9aff" stroke-width="1.5" />
      </svg>
    </div>

    <div class="quick-grid">
      <router-link to="/sense" class="quick">
        <van-icon name="eye-o" size="22" color="#4c9aff" />
        <span>\u516b\u8def\u611f\u77e5</span>
      </router-link>
      <router-link to="/hotplug" class="quick">
        <van-icon name="fire-o" size="22" color="#f59e0b" />
        <span>\u70ed\u63d2\u62d4</span>
      </router-link>
      <router-link to="/app-rules" class="quick">
        <van-icon name="apps-o" size="22" color="#10b981" />
        <span>App \u89c4\u5219</span>
      </router-link>
      <router-link to="/log" class="quick">
        <van-icon name="notes-o" size="22" color="#9ca3af" />
        <span>\u65e5\u5fd7</span>
      </router-link>
    </div>

    <div class="card footer-card">
      <van-icon :name="snap?.screen_on ? 'eye-o' : 'closed-eye-o'" size="20" :color="snap?.screen_on ? 'var(--accent)' : 'var(--text-muted)'" />
      <div class="footer-info">
        <div class="footer-label">{{ snap?.screen_on ? '\u4eae\u5c4f' : '\u606f\u5c4f' }}</div>
        <div class="footer-pkg">{{ snap?.current_pkg || '\u672a\u77e5' }}</div>
      </div>
    </div>
  </div>
</template>
<style scoped>
.dashboard {
  padding: 16px;
  max-width: 600px;
  margin: 0 auto;
}

/* Hero */
.hero {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  padding: 8px 4px 20px;
}
.brand {
  font-size: 28px;
  font-weight: 700;
  background: linear-gradient(135deg, #3b82f6 0%, #8b5cf6 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  letter-spacing: 1px;
}
.subtitle {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: 4px;
  letter-spacing: 0.5px;
}
.lang-btn {
  background: var(--bg-card);
  color: var(--text-secondary);
  border: 1px solid var(--border);
  padding: 6px 14px;
  border-radius: 16px;
  font-size: 13px;
  cursor: pointer;
}
.lang-btn:active { background: var(--bg-card-hover); }

/* Status strip */
.status-strip {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
.status-cell {
  flex: 1;
  background: var(--bg-card);
  border-radius: 10px;
  padding: 8px 12px;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-secondary);
  border: 1px solid var(--border);
}
.status-cell .muted { color: var(--text-muted); font-size: 11px; }
.dot {
  width: 8px; height: 8px; border-radius: 50%;
  background: var(--text-muted);
}
.dot.ok { background: var(--success); box-shadow: 0 0 8px var(--success); }

/* Card */
.card {
  background: var(--bg-card);
  border-radius: 12px;
  padding: 16px;
  margin-bottom: 12px;
  border: 1px solid var(--border);
}
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}
.card-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}
.card-meta {
  font-size: 12px;
  color: var(--text-muted);
}

/* Mode card */
.mode-card {
  display: flex;
  align-items: center;
  gap: 12px;
  border-left: 4px solid var(--accent);
}
.mode-icon {
  width: 48px; height: 48px;
  border-radius: 12px;
  display: flex; align-items: center; justify-content: center;
}
.mode-info { flex: 1; }
.mode-label { font-size: 11px; color: var(--text-muted); }
.mode-name { font-size: 18px; font-weight: 600; }
.mode-link {
  display: flex; align-items: center; gap: 4px;
  font-size: 12px; color: var(--accent);
  padding: 6px 10px; border-radius: 16px;
  background: rgba(59, 130, 246, 0.10);
}

/* CPU grid */
.cpu-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
}
.cpu-cell {
  background: var(--bg-base);
  border-radius: 10px;
  padding: 8px 4px;
  text-align: center;
  border: 1px solid var(--border);
  position: relative;
}
.cpu-cell.offline { opacity: 0.4; }
.cpu-cell.hot .cpu-bar { background: var(--danger); }
.cpu-cell.warm .cpu-bar { background: var(--warning); }
.cpu-id {
  font-size: 11px;
  color: var(--text-muted);
  font-family: monospace;
}
.cpu-bar-wrap {
  height: 48px;
  background: rgba(0,0,0,0.04);
  border-radius: 4px;
  margin: 4px 0;
  display: flex;
  align-items: flex-end;
  overflow: hidden;
}
.cpu-bar {
  width: 100%;
  background: var(--accent);
  border-radius: 4px;
  transition: height 0.3s ease, background 0.3s ease;
  min-height: 2px;
}
.cpu-load {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

/* Metrics */
.metrics {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
  margin-bottom: 12px;
}
.metric {
  background: var(--bg-card);
  border-radius: 10px;
  padding: 12px;
  border: 1px solid var(--border);
}
.metric-label {
  font-size: 11px;
  color: var(--text-muted);
  margin-bottom: 4px;
}
.metric-value {
  font-size: 24px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  line-height: 1.1;
}
.metric-unit {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-muted);
  margin-left: 2px;
}
.metric-bar {
  height: 3px;
  background: rgba(0,0,0,0.05);
  border-radius: 2px;
  margin-top: 8px;
  overflow: hidden;
}
.metric-bar-fill {
  height: 100%;
  border-radius: 2px;
  transition: width 0.3s ease, background 0.3s ease;
}
.metric-sub {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 4px;
}

/* Trend */
.trend-chart {
  width: 100%;
  height: 60px;
}

/* Quick grid */
.quick-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
  margin-bottom: 12px;
}
.quick {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px 4px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--text-secondary);
  text-decoration: none;
}
.quick:active { transform: scale(0.95); background: var(--bg-card-hover); }

/* Footer */
.footer-card {
  display: flex;
  align-items: center;
  gap: 12px;
}
.footer-info { flex: 1; }
.footer-label { font-size: 11px; color: var(--text-muted); }
.footer-pkg {
  font-size: 13px;
  color: var(--text-primary);
  font-family: monospace;
  word-break: break-all;
  margin-top: 2px;
}
</style>
