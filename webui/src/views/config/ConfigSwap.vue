<!--
  src/views/config/ConfigSwap.vue — 内存子页 (绿色)
  需求升级: 自动管理 → 可配置, 亮屏/息屏两套 (交换倾向 + 压力线).
-->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { fetchSenseSnapshot, type SenseResult } from '@/api/sense'
import { SWAP_PARAMS } from '@/config/moduleSpecs'
import ScreenScopedModule from '@/components/ScreenScopedModule.vue'

const router = useRouter()
const senseRes = ref<SenseResult | null>(null)
let pollTimer: number | null = null

const snap = computed(() => senseRes.value?.data ?? null)
const memText = computed(() => `${Math.round(Number(snap.value?.mem_full_pct ?? 0))}%`)
const swapText = computed(() => {
  const mb = snap.value?.swap_used_mb
  return mb !== undefined ? `${mb} MB` : '--'
})
const swapLoadPct = computed(() => {
  const p = Number(snap.value?.mem_full_pct ?? 0)
  return Math.min(100, Math.max(0, p * 2))
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
    <van-nav-bar title="内存" left-arrow left-text="返回" @click-left="router.push('/config')" />
    <div class="sub-body">
      <section class="cfg-card" :style="{ borderLeft: '4px solid #10b981' }">
        <div class="cfg-card-head">
          <span class="cfg-card-name">内存设置</span>
          <span class="live-tag">改动自动生效</span>
        </div>
        <ScreenScopedModule module-key="swap" :params="SWAP_PARAMS">
          <div class="readout">
            <div><span>内存压力 (压力指数)</span><b>{{ memText }}</b></div>
            <div><span>压缩交换已用</span><b>{{ swapText }}</b></div>
          </div>
          <van-progress :percentage="swapLoadPct" :show-pivot="false" stroke-width="6"
                        color="#10b981" style="margin-top: 10px;" />
        </ScreenScopedModule>
        <p class="cfg-intro">压力指数 = 内核报告的内存阻塞时间占比 (10 秒窗口)。
        持续高于压力线说明内存吃紧, 可下调"交换倾向"让系统更积极换出。</p>
      </section>
    </div>
  </div>
</template>
