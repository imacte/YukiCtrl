<!-- src/views/config/ConfigSwap.vue — 内存子页 (绿色, 只读监控 + 自动策略说明) -->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { fetchSenseSnapshot, type SenseResult } from '@/api/sense'
import { SWAP_DESC } from '@/config/moduleSpecs'
import DescLines from '@/components/DescLines.vue'
import ResetDefaultsBtn from '@/components/ResetDefaultsBtn.vue'

const tipMsg = ref('')
function resetModuleDefaults() {
  tipMsg.value = '本模块为全自动管理, 没有可调参数, 已是默认状态'
  setTimeout(() => { tipMsg.value = '' }, 2500)
}

const router = useRouter()
const senseRes = ref<SenseResult | null>(null)
let pollTimer: number | null = null

const snap = computed(() => senseRes.value?.data ?? null)
const memText = computed(() => {
  const p = snap.value?.mem_full_pct ?? 0
  return `${p.toFixed(1)}%`
})
const swapText = computed(() => `${snap.value?.swap_used_mb ?? '--'} MB`)
const swapLoadPct = computed(() => Math.min(100, Math.round((snap.value?.swap_used_mb ?? 0) / 80))) // 假设 8GB 压缩交换满量程

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
          <span class="auto-tag">自动管理</span>
        </div>

        <div class="readout">
          <div><span>内存压力 (压力指数)</span><b>{{ memText }}</b></div>
          <div><span>压缩交换已用</span><b>{{ swapText }}</b></div>
        </div>
        <van-progress :percentage="swapLoadPct" :show-pivot="false" stroke-width="6"
                      color="#10b981" style="margin-top: 10px;" />

        <DescLines :desc="SWAP_DESC" />

        <div v-if="tipMsg" class="cfg-banner ok">{{ tipMsg }}</div>
        <ResetDefaultsBtn @reset="resetModuleDefaults" />

        <p class="cfg-intro">压力指数 = 内核报告的内存阻塞时间占比 (10 秒窗口)。
        持续高于 20% 说明内存吃紧, 此时调度器会主动降频让路、系统会加大压缩交换。</p>
      </section>
    </div>
  </div>
</template>