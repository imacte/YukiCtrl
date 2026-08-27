<!--
  src/views/config/ConfigGpu.vue — 显卡子页 (紫色)
  需求升级: 自动管理 → 可配置, 亮屏/息屏两套 (频率护栏 + 加速阈值).
  实时负载读数保留 (帧平滑引擎仍自动调频, 护栏约束其上下限).
-->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { fetchSenseSnapshot, type SenseResult } from '@/api/sense'
import { GPU_PARAMS } from '@/config/moduleSpecs'
import ScreenScopedModule from '@/components/ScreenScopedModule.vue'

const router = useRouter()
const senseRes = ref<SenseResult | null>(null)
let pollTimer: number | null = null

const snap = computed(() => senseRes.value?.data ?? null)
const gpuText = computed(() => {
  const g = Number(snap.value?.gpu_load_pct ?? 0)
  return g > 0 ? `${Math.round(g)}%` : '--'
})
const fpsText = computed(() => {
  const f = Number(snap.value?.fps ?? 0)
  return f > 0 ? f.toFixed(0) : '--'
})
const hzText = computed(() => {
  const h = Number(snap.value?.display_hz ?? 0)
  return h > 0 ? `${h.toFixed(0)}Hz` : '--'
})

onMounted(() => {
  const refresh = async () => { try { senseRes.value = await fetchSenseSnapshot() } catch { /* 保持 */ } }
  refresh()
  pollTimer = window.setInterval(refresh, 1500)
})
onUnmounted(() => { if (pollTimer !== null) window.clearInterval(pollTimer) })
</script>

<template>
  <div class="sub-page">
    <van-nav-bar title="显卡" left-arrow left-text="返回" @click-left="router.push('/config')" />
    <div class="sub-body">
      <section class="cfg-card" :style="{ borderLeft: '4px solid #8b5cf6' }">
        <div class="cfg-card-head">
          <span class="cfg-card-name">显卡设置</span>
          <span class="live-tag">改动自动生效</span>
        </div>
        <ScreenScopedModule module-key="gpu" :params="GPU_PARAMS">
          <div class="readout">
            <div><span>显卡负载</span><b>{{ gpuText }}</b></div>
            <div><span>实测帧率</span><b>{{ fpsText }} 帧/秒</b></div>
            <div><span>屏幕刷新率</span><b>{{ hzText }}</b></div>
            <div><span>链路</span><b>{{ senseRes?.ok ? '正常' : '断开' }}</b></div>
          </div>
        </ScreenScopedModule>
      </section>
    </div>
  </div>
</template>
