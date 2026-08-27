<!--
  src/views/SensePanel.vue

  \u516b\u8def\u611f\u77e5\u8be6\u7ec6\u6570\u636e (\u6697\u8272)
-->
<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { fetchSenseSnapshot, type SenseSnapshot } from '@/api/sense'

const { t } = useI18n()

const snap = ref<SenseSnapshot | null>(null)
const error = ref<string | null>(null)

const fetchOnce = async () => {
  try {
    snap.value = await fetchSenseSnapshot()
    error.value = null
  } catch (e) {
    error.value = String(e)
  }
}

let timer: number | null = null
onMounted(() => {
  fetchOnce()
  timer = window.setInterval(fetchOnce, 1000)
})
onUnmounted(() => {
  if (timer !== null) window.clearInterval(timer)
})

const tempColor = computed(() => {
  if (!snap.value || snap.value.temp_c === 0) return 'var(--text-muted)'
  if (snap.value.temp_c > 65) return 'var(--danger)'
  if (snap.value.temp_c > 55) return 'var(--warning)'
  return 'var(--success)'
})

const cpuBars = computed(() => snap.value?.cpu_utils_pct ?? [])

const ageColor = computed(() => {
  if (!snap.value) return 'var(--text-muted)'
  if (snap.value.touch_age_ms < 200) return 'var(--success)'
  if (snap.value.touch_age_ms < 1000) return 'var(--warning)'
  return 'var(--text-muted)'
})

const pkgDisplay = computed(() => snap.value?.current_pkg || t('pkg_unknown'))

function barColor(v: number) {
  if (v > 80) return 'var(--danger)'
  if (v > 50) return 'var(--warning)'
  return 'var(--accent)'
}
</script>
<template>
  <div class="sense-page">
    <div class="page-header">
      <span class="page-title">{{ t('sense_panel') }}</span>
    </div>

    <div v-if="error" class="banner-error">\u26a0\ufe0f {{ error }}</div>

    <!-- CPU 8 \u6838\u67f1 -->
    <div class="card">
      <div class="card-header">
        <span class="card-title">{{ t('sense_cpu') }}</span>
        <span class="card-meta">8 \u6838</span>
      </div>
      <div class="cpu-bars">
        <div v-for="(u, i) in cpuBars" :key="i" class="bar-col">
          <div class="bar-wrap" :title="`cpu${i} ${u.toFixed(1)}%`">
            <div class="bar-fill" :style="{ height: u + '%', background: barColor(u) }" />
          </div>
          <div class="bar-label">{{ Math.round(u) }}</div>
          <div class="bar-id">cpu{{ i }}</div>
        </div>
      </div>
    </div>

    <!-- GPU -->
    <div class="card">
      <div class="card-title">{{ t('sense_gpu') }}</div>
      <div class="big-value" :style="{ color: snap?.gpu_load_pct ? barColor(snap.gpu_load_pct) : 'var(--text-muted)' }">
        {{ (snap?.gpu_load_pct ?? 0).toFixed(1) }}%
      </div>
      <div class="bar-track">
        <div class="bar-fill-h" :style="{ width: (snap?.gpu_load_pct ?? 0) + '%', background: barColor(snap?.gpu_load_pct ?? 0) }"></div>
      </div>
    </div>

    <!-- IO / Swap -->
    <div class="card">
      <div class="card-title">{{ t('sense_io') }}</div>
      <div class="progress-row">
        <span class="progress-label">some</span>
        <div class="bar-track">
          <div class="bar-fill-h" :style="{ width: (snap?.io_some_pct ?? 0) + '%', background: 'var(--accent)' }"></div>
        </div>
        <span class="progress-value">{{ (snap?.io_some_pct ?? 0).toFixed(1) }}%</span>
      </div>
      <div class="progress-row">
        <span class="progress-label">full</span>
        <div class="bar-track">
          <div class="bar-fill-h" :style="{ width: (snap?.io_full_pct ?? 0) + '%', background: 'var(--warning)' }"></div>
        </div>
        <span class="progress-value">{{ (snap?.io_full_pct ?? 0).toFixed(1) }}%</span>
      </div>
    </div>

    <div class="card">
      <div class="card-title">{{ t('sense_swap') }}</div>
      <div class="progress-row">
        <span class="progress-label">mem PSI</span>
        <div class="bar-track">
          <div class="bar-fill-h" :style="{ width: Math.min(100, (snap?.mem_full_pct ?? 0) * 3) + '%', background: 'var(--accent)' }"></div>
        </div>
        <span class="progress-value">{{ (snap?.mem_full_pct ?? 0).toFixed(1) }}%</span>
      </div>
      <div class="progress-row">
        <span class="progress-label">zram</span>
        <span class="progress-value">{{ snap?.swap_used_mb ?? 0 }} MB</span>
      </div>
    </div>

    <!-- \u6e29\u5ea6 + \u89e6\u6478 -->
    <div class="dual-row">
      <div class="card">
        <div class="card-title">{{ t('sense_temp') }}</div>
        <div class="big-value" :style="{ color: tempColor }">
          {{ (snap?.temp_c ?? 0).toFixed(1) }}<span class="metric-unit">\u00b0C</span>
        </div>
      </div>
      <div class="card">
        <div class="card-title">{{ t('sense_touch') }}</div>
        <div class="touch-row">
          <van-icon :name="snap?.touch_down ? 'down' : 'up'" size="22" :color="snap?.touch_down ? 'var(--accent)' : 'var(--text-muted)'" />
          <div>
            <div class="touch-info">{{ snap?.touch_down ? t('touch_down') : t('touch_up') }}</div>
            <div class="touch-age" :style="{ color: ageColor }">{{ snap?.touch_age_ms ?? 9999 }} ms</div>
          </div>
        </div>
      </div>
    </div>

    <!-- FPS -->
    <div class="card">
      <div class="card-title">{{ t('sense_fps') }}</div>
      <div class="big-value" :style="{ color: (snap?.fps ?? 0) > 0 ? 'var(--accent)' : 'var(--text-muted)' }">
        {{ (snap?.fps ?? 0) > 0 ? snap?.fps + ' fps' : '\u2014' }}
      </div>
    </div>

    <!-- \u5f53\u524d\u5e94\u7528 -->
    <div class="card">
      <div class="card-title">{{ t('sense_current_app') }}</div>
      <div class="pkg-row">
        <van-icon :name="snap?.screen_on ? 'eye-o' : 'closed-eye-o'" size="22" :color="snap?.screen_on ? 'var(--accent)' : 'var(--text-muted)'" />
        <div class="pkg-info">
          <div class="pkg-name">{{ pkgDisplay }}</div>
          <div class="pkg-status">{{ snap?.screen_on ? t('screen_on') : t('screen_off') }}</div>
        </div>
      </div>
    </div>

    <div class="footer-hint">{{ t('sense_footer_hint') }}</div>
  </div>
</template>
<style scoped>
.sense-page {
  padding: 16px;
  max-width: 600px;
  margin: 0 auto;
}
.page-header {
  padding: 8px 4px 16px;
}
.page-title { font-size: 20px; font-weight: 600; }
.banner-error {
  background: rgba(239, 68, 68, 0.15);
  border: 1px solid var(--danger);
  color: var(--danger);
  padding: 8px 12px; border-radius: 8px;
  margin-bottom: 12px; font-size: 12px;
}
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
  margin-bottom: 8px;
}
.card-meta {
  font-size: 12px;
  color: var(--text-muted);
}
.cpu-bars {
  display: grid;
  grid-template-columns: repeat(8, 1fr);
  gap: 6px;
  align-items: end;
  height: 120px;
}
.bar-col {
  display: flex;
  flex-direction: column;
  align-items: center;
  height: 100%;
}
.bar-wrap {
  width: 100%;
  flex: 1;
  background: rgba(0,0,0,0.04);
  border-radius: 4px;
  display: flex;
  align-items: flex-end;
  overflow: hidden;
}
.bar-fill {
  width: 100%;
  border-radius: 4px;
  transition: height 0.3s ease;
  min-height: 2px;
}
.bar-track {
  flex: 1;
  height: 6px;
  background: rgba(0,0,0,0.05);
  border-radius: 3px;
  overflow: hidden;
  margin: 0 8px;
}
.bar-fill-h {
  height: 100%;
  border-radius: 3px;
  transition: width 0.3s ease;
}
.bar-label {
  font-size: 11px;
  color: var(--text-primary);
  margin-top: 4px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.bar-id {
  font-size: 9px;
  color: var(--text-muted);
  font-family: monospace;
}
.big-value {
  font-size: 32px;
  font-weight: 700;
  text-align: center;
  margin: 8px 0;
  font-variant-numeric: tabular-nums;
}
.metric-unit {
  font-size: 14px;
  color: var(--text-muted);
  font-weight: 500;
  margin-left: 4px;
}
.progress-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  font-size: 12px;
}
.progress-label {
  width: 64px;
  color: var(--text-muted);
}
.progress-value {
  width: 60px;
  text-align: right;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}
.dual-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-bottom: 12px;
}
.dual-row .card { margin-bottom: 0; }
.touch-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
}
.touch-info {
  font-size: 13px;
  color: var(--text-primary);
}
.touch-age {
  font-size: 11px;
  margin-top: 2px;
  font-variant-numeric: tabular-nums;
}
.pkg-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 4px;
}
.pkg-info { flex: 1; }
.pkg-name {
  font-family: monospace;
  font-size: 13px;
  color: var(--text-primary);
  word-break: break-all;
}
.pkg-status {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 2px;
}
.footer-hint {
  text-align: center;
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 16px;
}
</style>
