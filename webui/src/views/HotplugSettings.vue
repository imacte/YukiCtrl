<!--
  src/views/HotplugSettings.vue

  \u70ed\u63d2\u62d4\u8bbe\u7f6e\u9875\u9762 (\u6697\u8272)
-->
<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { fetchHotplugState, saveHotplugConfig, maskToCpuArray, type HotplugState, type HotplugConfig } from '@/api/hotplug'
import HelpTooltip from '@/components/HelpTooltip.vue'

const { t } = useI18n()

const state = ref<HotplugState | null>(null)
const error = ref<string | null>(null)
const saving = ref(false)
const saveMsg = ref('')

const cpus = computed(() => state.value ? maskToCpuArray(state.value.online_mask) : [])
const onlineCount = computed(() => cpus.value.filter(Boolean).length)

let timer: number | null = null
const refresh = async () => {
  try {
    state.value = await fetchHotplugState()
    error.value = null
  } catch (e) {
    error.value = String(e)
  }
}

const persist = async () => {
  if (!state.value) return
  saving.value = true
  try {
    const cfg: HotplugConfig = {
      lockscreen_onoff: state.value.lockscreen_onoff,
      screens_onoff: state.value.screens_onoff,
      off_threshold_idle_pct: state.value.off_threshold_idle_pct,
      on_threshold_util_pct: state.value.on_threshold_util_pct,
    }
    await saveHotplugConfig(cfg)
    saveMsg.value = t('hotplug_config_saved') as string
    setTimeout(() => saveMsg.value = '', 1500)
  } catch (e) {
    error.value = String(e)
  } finally {
    saving.value = false
  }
}

onMounted(() => {
  refresh()
  timer = window.setInterval(refresh, 500)
})
onUnmounted(() => {
  if (timer !== null) window.clearInterval(timer)
})
</script>

<template>
  <div class="hotplug-page">
    <div class="page-header">
      <span class="page-title">{{ t('hotplug_settings') }}</span>
      <span class="page-meta">{{ onlineCount }}/8 {{ t('online') }}</span>
    </div>

    <div v-if="error" class="banner-error">\u26a0\ufe0f {{ t('daemon_comm_failed') }}: {{ error }}</div>
    <div v-if="saveMsg" class="banner-ok">{{ saveMsg }}</div>

    <div class="card">
      <div class="card-header">
        <span class="card-title">8 \u6838\u72b6\u6001</span>
        <span class="card-meta">{{ state?.thermal_c?.toFixed(1) }}\u00b0C</span>
      </div>
      <div class="cpu-grid">
        <div v-for="(online, i) in cpus" :key="i" class="cpu-cell" :class="{ offline: !online, protected: i < 2 }">
          <div class="cpu-id">cpu{{ i }}</div>
          <div class="cpu-state">{{ online ? 'ON' : 'OFF' }}</div>
          <div v-if="i < 2" class="protected-tag">{{ t('protected') }}</div>
        </div>
      </div>
      <div class="hint">{{ t('cpu_grid_desc') }}</div>
    </div>

    <div class="card">
      <div class="card-title">{{ t('toggles') }}</div>
      <div class="row">
        <div class="row-text">
          <span class="row-label">{{ t('lockscreen_onoff') }}</span>
          <HelpTooltip :text="t('lockscreen_onoff_desc')" />
        </div>
        <van-switch :model-value="state?.lockscreen_onoff ?? true"
                    @update:model-value="(v: boolean) => { if (state) { state.lockscreen_onoff = v; persist() } }" />
      </div>
      <div class="row">
        <div class="row-text">
          <span class="row-label">{{ t('screens_onoff') }}</span>
          <HelpTooltip :text="t('screens_onoff_desc')" />
        </div>
        <van-switch :model-value="state?.screens_onoff ?? true"
                    @update:model-value="(v: boolean) => { if (state) { state.screens_onoff = v; persist() } }" />
      </div>
    </div>

    <div class="card">
      <div class="card-title">{{ t('thresholds') }}</div>
      <div class="slider-row">
        <div class="row-text">
          <span class="row-label">{{ t('off_threshold_idle_pct') }}</span>
          <HelpTooltip :text="t('off_threshold_idle_pct_desc')" />
        </div>
        <span class="slider-value">{{ state?.off_threshold_idle_pct ?? 95 }}%</span>
      </div>
      <van-slider :model-value="state?.off_threshold_idle_pct ?? 95"
                  :min="50" :max="100" :step="1" bar-height="4px"
                  @update:model-value="(v: number) => { if (state) { state.off_threshold_idle_pct = v; persist() } }" />
      <div class="slider-row" style="margin-top: 16px;">
        <div class="row-text">
          <span class="row-label">{{ t('on_threshold_util_pct') }}</span>
          <HelpTooltip :text="t('on_threshold_util_pct_desc')" />
        </div>
        <span class="slider-value">{{ state?.on_threshold_util_pct ?? 30 }}%</span>
      </div>
      <van-slider :model-value="state?.on_threshold_util_pct ?? 30"
                  :min="5" :max="80" :step="1" bar-height="4px"
                  @update:model-value="(v: number) => { if (state) { state.on_threshold_util_pct = v; persist() } }" />
    </div>

    <div class="hint-bar">{{ t('hotplug_hint') }}</div>
  </div>
</template>
<style scoped>
.hotplug-page {
  padding: 16px;
  max-width: 600px;
  margin: 0 auto;
}
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 4px 16px;
}
.page-title { font-size: 20px; font-weight: 600; }
.page-meta { font-size: 13px; color: var(--text-muted); }
.banner-error {
  background: rgba(239, 68, 68, 0.15);
  border: 1px solid var(--danger);
  color: var(--danger);
  padding: 8px 12px; border-radius: 8px;
  margin-bottom: 12px; font-size: 12px;
}
.banner-ok {
  background: rgba(16, 185, 129, 0.15);
  border: 1px solid var(--success);
  color: var(--success);
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
.card-title {
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 12px;
}
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}
.card-meta {
  font-size: 12px;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}
.cpu-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
}
.cpu-cell {
  background: var(--bg-base);
  border: 1px solid var(--success);
  border-radius: 10px;
  padding: 8px 4px;
  text-align: center;
  position: relative;
}
.cpu-cell.offline {
  border-color: var(--text-muted);
  opacity: 0.5;
}
.cpu-cell.protected {
  border-color: var(--accent);
  background: rgba(59, 130, 246, 0.08);
}
.cpu-id {
  font-weight: 600;
  font-size: 14px;
  color: var(--text-primary);
  font-family: monospace;
}
.cpu-state {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 4px;
}
.protected-tag {
  position: absolute;
  top: 2px; right: 4px;
  font-size: 9px;
  background: var(--accent);
  color: white;
  padding: 1px 4px;
  border-radius: 4px;
}
.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 0;
  border-top: 1px solid var(--border);
}
.row:first-of-type { border-top: 0; }
.row-text {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
}
.row-label {
  font-size: 14px;
  color: var(--text-primary);
}
.slider-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}
.slider-value {
  font-size: 13px;
  color: var(--accent);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.hint {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 8px;
  line-height: 1.5;
}
.hint-bar {
  text-align: center;
  font-size: 11px;
  color: var(--text-muted);
  padding: 12px;
}
</style>
