<!--
  src/views/SensePanel.vue

  任务 #5 / ticket-09: 八路感知实时面板.

  数据源: src/api/sense.ts::fetchSenseSnapshot (200ms 轮询 daemon 写的 sense/snapshot.yaml).

  八路:
    1. CPU 8 核柱状图
    2. GPU 负载条
    3. IO PSI (some + full 双条)
    4. Swap PSI (mem PSI + zram 已用 MB)
    5. 温度 (°C, 高温告警)
    6. 触摸状态 (down/up + 距今 ms)
    7. FPS (屏幕刷新率)
    8. 当前 App 包名 + 屏幕状态
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
  if (!snap.value || snap.value.temp_c === 0) return '#999'
  if (snap.value.temp_c > 65) return '#dc3545'
  if (snap.value.temp_c > 55) return '#ff976a'
  return '#10b981'
})

const cpuBars = computed(() => snap.value?.cpu_utils_pct ?? [])

const ageColor = computed(() => {
  if (!snap.value) return '#999'
  if (snap.value.touch_age_ms < 200) return '#10b981'
  if (snap.value.touch_age_ms < 1000) return '#ff976a'
  return '#999'
})

const pkgDisplay = computed(() => snap.value?.current_pkg || t('pkg_unknown'))
</script>

<template>
  <div class="sense-panel">
    <van-nav-bar :title="t('sense_panel')" left-arrow @click-left="$router.back()" fixed placeholder />

    <div v-if="error" class="banner-error">⚠️ {{ error }}</div>

    <!-- 1. CPU 8 核 -->
    <div class="panel-card">
      <div class="card-title">{{ t('sense_cpu') }}</div>
      <div class="cpu-bars">
        <div v-for="(u, i) in cpuBars" :key="i" class="bar-col">
          <div class="bar-wrap" :title="`cpu${i} ${u.toFixed(1)}%`">
            <div class="bar-fill" :style="{ height: u + '%', background: u > 80 ? '#dc3545' : u > 50 ? '#ff976a' : '#1989fa' }" />
          </div>
          <div class="bar-label">{{ Math.round(u) }}</div>
          <div class="bar-id">cpu{{ i }}</div>
        </div>
      </div>
    </div>

    <!-- 2. GPU -->
    <div class="panel-card">
      <div class="card-title">{{ t('sense_gpu') }}</div>
      <div class="progress-row">
        <span class="progress-label">{{ t('sense_load') }}</span>
        <van-progress :percentage="snap?.gpu_load_pct ?? 0" stroke-width="10" :show-text="false" />
        <span class="progress-value">{{ (snap?.gpu_load_pct ?? 0).toFixed(1) }}%</span>
      </div>
    </div>

    <!-- 3. IO PSI -->
    <div class="panel-card">
      <div class="card-title">{{ t('sense_io') }}</div>
      <div class="progress-row">
        <span class="progress-label">some</span>
        <van-progress :percentage="snap?.io_some_pct ?? 0" stroke-width="8" :show-text="false" color="#1989fa" />
        <span class="progress-value">{{ (snap?.io_some_pct ?? 0).toFixed(1) }}%</span>
      </div>
      <div class="progress-row">
        <span class="progress-label">full</span>
        <van-progress :percentage="snap?.io_full_pct ?? 0" stroke-width="8" :show-text="false" color="#ff976a" />
        <span class="progress-value">{{ (snap?.io_full_pct ?? 0).toFixed(1) }}%</span>
      </div>
    </div>

    <!-- 4. Swap / 内存 -->
    <div class="panel-card">
      <div class="card-title">{{ t('sense_swap') }}</div>
      <div class="progress-row">
        <span class="progress-label">mem PSI</span>
        <van-progress :percentage="snap?.mem_full_pct ?? 0" stroke-width="8" :show-text="false" color="#dc3545" />
        <span class="progress-value">{{ (snap?.mem_full_pct ?? 0).toFixed(1) }}%</span>
      </div>
      <div class="progress-row">
        <span class="progress-label">{{ t('sense_zram') }}</span>
        <span class="progress-value" style="margin-left: auto;">{{ snap?.swap_used_mb ?? 0 }} MB</span>
      </div>
    </div>

    <!-- 5. 温度 -->
    <div class="panel-card">
      <div class="card-title">{{ t('sense_temp') }}</div>
      <div class="big-value" :style="{ color: tempColor }">
        {{ snap && snap.temp_c > 0 ? snap.temp_c.toFixed(1) + '°C' : t('sense_unavail') }}
      </div>
    </div>

    <!-- 6. 触摸 -->
    <div class="panel-card">
      <div class="card-title">{{ t('sense_touch') }}</div>
      <div class="touch-row">
        <van-icon :name="snap?.touch_down ? 'down' : 'up'" :size="32" :color="snap?.touch_down ? '#1989fa' : '#999'" />
        <div class="touch-info">
          <div>{{ snap?.touch_down ? t('touch_down') : t('touch_up') }}</div>
          <div class="touch-age" :style="{ color: ageColor }">
            {{ snap?.touch_age_ms ?? 9999 }} ms
          </div>
        </div>
      </div>
    </div>

    <!-- 7. FPS -->
<style scoped>
.sense-panel { padding: 12px 12px 60px; background: #f7f8fa; min-height: 100vh; }
.banner-error {
  background: #fff3cd; border: 1px solid #ffe69c; color: #664d03;
  padding: 8px; border-radius: 4px; margin-bottom: 12px; font-size: 12px;
}
.panel-card {
  background: #fff;
  border-radius: 12px;
  padding: 14px;
  margin-bottom: 12px;
  box-shadow: 0 2px 8px rgba(0,0,0,0.04);
}
.card-title {
  font-size: 14px;
  font-weight: 600;
  color: #323233;
  margin-bottom: 10px;
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
  background: #f2f3f5;
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
.bar-label {
  font-size: 10px;
  color: #666;
  margin-top: 4px;
}
.bar-id {
  font-size: 9px;
  color: #999;
  margin-top: 1px;
}
.progress-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
  font-size: 12px;
}
.progress-label {
  width: 64px;
  color: #666;
}
.progress-value {
  width: 56px;
  text-align: right;
  color: #323233;
  font-variant-numeric: tabular-nums;
}
.big-value {
  font-size: 28px;
  font-weight: 600;
  text-align: center;
  margin: 12px 0;
  font-variant-numeric: tabular-nums;
}
.touch-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.touch-info {
  font-size: 13px;
}
.touch-age {
  font-size: 11px;
  margin-top: 2px;
}
.pkg-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.pkg-info { font-size: 13px; }
.pkg-name {
  font-family: monospace;
  font-size: 13px;
  word-break: break-all;
}
.pkg-status {
  font-size: 11px;
  color: #999;
  margin-top: 2px;
}
.footer-hint {
  text-align: center;
  font-size: 11px;
  color: #999;
  margin-top: 16px;
}
</style>
    <div class="panel-card">
      <div class="card-title">{{ t('sense_fps') }}</div>
      <div class="big-value" :style="{ color: (snap?.fps ?? 0) > 0 ? '#1989fa' : '#999' }">
        {{ (snap?.fps ?? 0) > 0 ? snap?.fps + ' fps' : (snap?.screen_on ? '?' : t('screen_off')) }}
      </div>
    </div>

    <!-- 8. 当前 App -->
    <div class="panel-card">
      <div class="card-title">{{ t('sense_current_app') }}</div>
      <div class="pkg-row">
        <van-icon :name="snap?.screen_on ? 'eye-o' : 'closed-eye-o'" :size="24" :color="snap?.screen_on ? '#1989fa' : '#999'" />
        <div class="pkg-info">
          <div class="pkg-name">{{ pkgDisplay }}</div>
          <div class="pkg-status">
            {{ snap?.screen_on ? t('screen_on') : t('screen_off') }}
          </div>
        </div>
      </div>
    </div>

    <div class="footer-hint">
      {{ t('sense_footer_hint') }}
    </div>
  </div>
</template>