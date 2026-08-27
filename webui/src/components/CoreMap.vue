<script setup lang="ts">
// src/components/CoreMap.vue
//
// Phase 2 / ticket-04 — 核心映射视图 (8 cpu 网格 + 热插拔控制)
//
// 数据源: src/api/hotplug.ts (文件 IPC, daemon 200ms tick 写 state.yaml)
// 风格: 沿用 vant 4 + WebUI 4模式基线; 不引入新依赖.

import { ref, onMounted, onUnmounted, computed } from 'vue'
import { fetchHotplugState, saveHotplugConfig, maskToCpuArray, type HotplugState } from '@/api/hotplug'

const state = ref<HotplugState | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

const cpus = computed(() => state.value ? maskToCpuArray(state.value.online_mask) : [])

// 200ms 刷新 (与 daemon tick 对齐)
let timer: number | null = null
const refresh = async () => {
  try {
    state.value = await fetchHotplugState()
    error.value = null
  } catch (e) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}

const saveConfig = async () => {
  if (!state.value) return
  loading.value = true
  try {
    await saveHotplugConfig({
      lockscreen_onoff: state.value.lockscreen_onoff,
      screens_onoff: state.value.screens_onoff,
      off_threshold_idle_pct: state.value.off_threshold_idle_pct,
      on_threshold_util_pct: state.value.on_threshold_util_pct
    })
    await refresh()
  } catch (e) {
    error.value = String(e)
  } finally {
    loading.value = false
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
  <div class="core-map">
    <h3>核心映射</h3>

    <div v-if="error" class="banner-error">
      ⚠️ daemon 通信失败: {{ error }}
      <br />提示: 请去 KSU Manager → Superuser → core-pilot → 模块设置 → 允许 sysfs 读写 (D7)
    </div>

    <div class="cpu-grid">
      <div
        v-for="(online, i) in cpus"
        :key="i"
        class="cpu-cell"
        :class="{ offline: !online, protected: i === 0 || i === 1 }"
      >
        <div class="cpu-id">cpu{{ i }}</div>
        <div class="cpu-state">{{ online ? 'ON' : 'OFF' }}</div>
        <div v-if="i === 0 || i === 1" class="protected-tag">受保护</div>
      </div>
    </div>

    <div class="meta">
      <div>温度: {{ state?.thermal_c.toFixed(1) }}°C</div>
      <div>debounce: {{ state?.disable_debounce_ticks }} tick (1s @ 200ms)</div>
    </div>

    <van-cell-group inset title="热插拔控制 (D4 + D6)">
      <van-cell title="锁屏时启用">
        <template #value>
          <van-switch
            :model-value="state?.lockscreen_onoff ?? true"
            @update:model-value="(v: boolean) => { if (state) state.lockscreen_onoff = v }"
            @change="saveConfig"
          />
        </template>
      </van-cell>
      <van-cell title="灭屏时启用">
        <template #value>
          <van-switch
            :model-value="state?.screens_onoff ?? true"
            @update:model-value="(v: boolean) => { if (state) state.screens_onoff = v }"
            @change="saveConfig"
          />
        </template>
      </van-cell>
      <van-cell title="disable 阈值 (idle %)">
        <template #value>
          <van-stepper
            :model-value="state?.off_threshold_idle_pct ?? 95"
            :min="50"
            :max="100"
            :step="1"
            @change="(v: number | string) => { if (state && typeof v === 'number') { state.off_threshold_idle_pct = v; saveConfig() } }"
          />
        </template>
      </van-cell>
      <van-cell title="enable 阈值 (util %)">
        <template #value>
          <van-stepper
            :model-value="state?.on_threshold_util_pct ?? 30"
            :min="5"
            :max="80"
            :step="1"
            @change="(v: number | string) => { if (state && typeof v === 'number') { state.on_threshold_util_pct = v; saveConfig() } }"
          />
        </template>
      </van-cell>
    </van-cell-group>

    <div class="hint">
      ℹ️ 配置变更 200ms 内由 daemon tick 拾取; cpu 切换延迟约 1s (debounce)
    </div>
  </div>
</template>

<style scoped>
.core-map {
  padding: 12px;
}
.banner-error {
  background: #fff3cd;
  border: 1px solid #ffe69c;
  color: #664d03;
  padding: 8px;
  border-radius: 4px;
  margin-bottom: 12px;
  font-size: 12px;
}
.cpu-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
  margin-bottom: 16px;
}
.cpu-cell {
  border: 1px solid #1989fa;
  border-radius: 8px;
  padding: 8px;
  text-align: center;
  background: #e8f3ff;
  position: relative;
}
.cpu-cell.offline {
  border-color: #dc3545;
  background: #ffe8e8;
}
.cpu-cell.protected {
  border-color: #28a745;
  background: #e8ffe8;
}
.cpu-id {
  font-weight: bold;
  font-size: 14px;
}
.cpu-state {
  font-size: 12px;
  margin-top: 2px;
}
.protected-tag {
  position: absolute;
  top: 2px;
  right: 4px;
  font-size: 9px;
  background: #28a745;
  color: white;
  padding: 1px 4px;
  border-radius: 2px;
}
.meta {
  font-size: 12px;
  color: #666;
  margin-bottom: 12px;
}
.hint {
  font-size: 11px;
  color: #999;
  margin-top: 12px;
}
</style>