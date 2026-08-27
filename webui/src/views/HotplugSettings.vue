<!--
  src/views/HotplugSettings.vue

  任务 #5: 热插拔设置页面 (基于 CoreMap 完善).

  功能:
    - 顶部 8 核网格实时状态 (200ms 轮询)
    - 2 个 toggle: 锁屏时启用 / 灭屏时启用
    - 2 个 slider: off_threshold_idle_pct / on_threshold_util_pct
    - 1 个 stepper: min_online_cores (后端目前不支持运行时改, 暂 disabled)
    - 所有配置项旁加 HelpTooltip (傻瓜化中文说明)

  通信: 与 api/hotplug.ts 一致 — WebUI 写 config.yaml, daemon 200ms tick 拾取.
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

const minOnlineCores = ref(2)

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

<template>
  <div class="hotplug-settings">
    <van-nav-bar :title="t('hotplug_settings')" left-arrow @click-left="$router.back()" fixed placeholder />

    <div v-if="error" class="banner-error">⚠️ {{ t('daemon_comm_failed') }}: {{ error }}</div>
    <div v-if="saveMsg" class="banner-ok">{{ saveMsg }}</div>

    <div class="cpu-grid-section">
      <div class="section-title">
        {{ t('cpu_grid') }}
        <span class="online-count">{{ onlineCount }}/8 {{ t('online') }}</span>
        <HelpTooltip :title="t('cpu_grid')" :text="t('cpu_grid_desc')" />
      </div>
      <div class="cpu-grid">
        <div v-for="(online, i) in cpus" :key="i" class="cpu-cell" :class="{ offline: !online, protected: i === 0 || i === 1 }">
          <div class="cpu-id">cpu{{ i }}</div>
          <div class="cpu-state">{{ online ? 'ON' : 'OFF' }}</div>
          <div v-if="i === 0 || i === 1" class="protected-tag">{{ t('protected') }}</div>
        </div>
      </div>
      <div class="meta">
        <div>{{ t('temperature') }}: {{ state?.thermal_c.toFixed(1) }}°C</div>
        <div>{{ t('updated_at') }}: {{ state?.updated_at_unix_ms ? new Date(state.updated_at_unix_ms).toLocaleTimeString() : '-' }}</div>
      </div>
    </div>

    <van-cell-group inset :title="t('toggles')">
      <van-cell :title="t('lockscreen_onoff')">
        <template #value>
          <van-switch :model-value="state?.lockscreen_onoff ?? true" @update:model-value="(v: boolean) => { if (state) { state.lockscreen_onoff = v; persist() } }" />
        </template>
        <template #right-icon>
          <HelpTooltip :text="t('lockscreen_onoff_desc')" />
        </template>
      </van-cell>
      <van-cell :title="t('screens_onoff')">
        <template #value>
          <van-switch :model-value="state?.screens_onoff ?? true" @update:model-value="(v: boolean) => { if (state) { state.screens_onoff = v; persist() } }" />
        </template>
        <template #right-icon>
          <HelpTooltip :text="t('screens_onoff_desc')" />
        </template>
      </van-cell>
    </van-cell-group>

    <van-cell-group inset :title="t('thresholds')">
      <van-cell :title="t('off_threshold_idle_pct')">
        <template #value>
          <span style="margin-right: 12px;">{{ state?.off_threshold_idle_pct ?? 95 }}%</span>
        </template>
        <template #label>
          <van-slider :model-value="state?.off_threshold_idle_pct ?? 95" :min="50" :max="100" :step="1" bar-height="4px" @update:model-value="(v: number) => { if (state) { state.off_threshold_idle_pct = v; persist() } }" />
        </template>
        <template #right-icon>
<style scoped>
.hotplug-settings { padding: 12px 0 60px; }
.banner-error {
  background: #fff3cd; border: 1px solid #ffe69c; color: #664d03;
  padding: 8px; border-radius: 4px; margin: 12px; font-size: 12px;
}
.banner-ok {
  background: #d4edda; border: 1px solid #c3e6cb; color: #155724;
  padding: 8px; border-radius: 4px; margin: 12px; font-size: 12px;
}
.cpu-grid-section {
  margin: 12px;
  padding: 12px;
  background: #fff;
  border-radius: 12px;
  box-shadow: 0 2px 8px rgba(0,0,0,0.04);
}
.section-title {
  display: flex;
  align-items: center;
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 8px;
  gap: 6px;
}
.online-count {
  margin-left: auto;
  font-size: 12px;
  color: #10b981;
  font-weight: 500;
}
.cpu-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
  margin-bottom: 12px;
}
.cpu-cell {
  border: 1px solid #1989fa;
  border-radius: 8px;
  padding: 8px;
  text-align: center;
  background: #e8f3ff;
  position: relative;
}
.cpu-cell.offline { border-color: #dc3545; background: #ffe8e8; }
.cpu-cell.protected { border-color: #28a745; background: #e8ffe8; }
.cpu-id { font-weight: bold; font-size: 14px; }
.cpu-state { font-size: 12px; margin-top: 2px; }
.protected-tag {
  position: absolute; top: 2px; right: 4px; font-size: 9px;
  background: #28a745; color: white; padding: 1px 4px; border-radius: 2px;
}
.meta { font-size: 12px; color: #666; }
.hint {
  font-size: 11px; color: #999; margin: 16px 12px 0;
}
</style>
          <HelpTooltip :text="t('off_threshold_idle_pct_desc')" />
        </template>
      </van-cell>
      <van-cell :title="t('on_threshold_util_pct')">
        <template #value>
          <span style="margin-right: 12px;">{{ state?.on_threshold_util_pct ?? 30 }}%</span>
        </template>
        <template #label>
          <van-slider :model-value="state?.on_threshold_util_pct ?? 30" :min="5" :max="80" :step="1" bar-height="4px" @update:model-value="(v: number) => { if (state) { state.on_threshold_util_pct = v; persist() } }" />
        </template>
        <template #right-icon>
          <HelpTooltip :text="t('on_threshold_util_pct_desc')" />
        </template>
      </van-cell>
    </van-cell-group>

    <van-cell-group inset :title="t('core_constraints')">
      <van-cell :title="t('min_online_cores')">
        <template #value>
          <van-stepper v-model="minOnlineCores" :min="1" :max="8" :step="1" disabled />
        </template>
        <template #right-icon>
          <HelpTooltip :text="t('min_online_cores_desc')" />
        </template>
      </van-cell>
    </van-cell-group>

    <div class="hint">ℹ️ {{ t('hotplug_hint') }}</div>
  </div>
</template>
onMounted(() => {
  refresh()
  timer = window.setInterval(refresh, 500)
})
onUnmounted(() => {
  if (timer !== null) window.clearInterval(timer)
})
</script>