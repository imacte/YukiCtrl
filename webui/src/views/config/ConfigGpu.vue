<!-- src/views/config/ConfigGpu.vue — 显卡子页 (紫色, 只读监控) -->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { fetchSenseSnapshot, type SenseResult } from '@/api/sense'
import { GPU_DESC } from '@/config/moduleSpecs'
import DescLines from '@/components/DescLines.vue'

const router = useRouter()
const senseRes = ref<SenseResult | null>(null)
let pollTimer: number | null = null

const snap = computed(() => senseRes.value?.data ?? null)
const gpuText = computed(() => {
  const g = Number(snap.value?.gpu_load_pct ?? 0)
  return g > 0 ? Math.round(g) + '%' : '--'
})
const fpsText = computed(() => {
  const f = snap.value?.fps ?? 0
  return f > 0 ? String(f) : '静止'
})
const hzText = computed(() => {
  const h = snap.value?.display_hz ?? 0
  return h > 0 ? Math.round(h) + ' 赫兹' : '--'
})

onMounted(() => {
  const refresh = async () => { try { senseRes.value = await fetchSenseSnapshot() } catch { /* 保持 */ } }
  refresh()
  pollTimer = window.setInterval(refresh, 1000)
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
          <span class="auto-tag">自动管理</span>
        </div>
        <p class="cfg-intro">显卡频率由帧平滑引擎根据游戏负载自动调节 (它比手动锁频更聪明:
          掉帧瞬间拉频、空闲立刻回落)。此处展示实时负载:</p>

        <div class="readout">
          <div><span>显卡负载</span><b>{{ gpuText }}</b></div>
          <div><span>实测帧率</span><b>{{ fpsText }} 帧/秒</b></div>
          <div><span>屏幕刷新率</span><b>{{ hzText }}</b></div>
          <div><span>链路</span><b>{{ senseRes?.ok ? '正常' : '断开' }}</b></div>
        </div>

        <DescLines :desc="GPU_DESC" />
      </section>
    </div>
  </div>
</template>