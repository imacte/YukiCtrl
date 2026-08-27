<!-- src/views/config/ConfigTouch.vue — 触摸加速子页 (青色, 自动策略 + 实时触摸状态) -->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { fetchSenseSnapshot, type SenseResult } from '@/api/sense'
import { TOUCH_DESC } from '@/config/moduleSpecs'
import DescLines from '@/components/DescLines.vue'
import ResetDefaultsBtn from '@/components/ResetDefaultsBtn.vue'

const tipMsg = ref('')
function resetModuleDefaults() {
  tipMsg.value = '本模块为全自动触发, 没有可调参数, 已是默认状态'
  setTimeout(() => { tipMsg.value = '' }, 2500)
}

const router = useRouter()
const senseRes = ref<SenseResult | null>(null)
let pollTimer: number | null = null

const snap = computed(() => senseRes.value?.data ?? null)
const touchState = computed(() => (snap.value?.touch_down ? '按下中' : '未按下'))
const ageText = computed(() => {
  const ms = snap.value?.touch_age_ms
  if (ms === undefined || ms === null) return '--'
  return ms < 1000 ? `${Math.round(ms)} 毫秒前` : `${(ms / 1000).toFixed(1)} 秒前`
})

onMounted(() => {
  const refresh = async () => { try { senseRes.value = await fetchSenseSnapshot() } catch { /* 保持 */ } }
  refresh()
  pollTimer = window.setInterval(refresh, 500)
})
onUnmounted(() => { if (pollTimer !== null) window.clearInterval(pollTimer) })
</script>

<template>
  <div class="sub-page">
    <van-nav-bar title="触摸加速" left-arrow left-text="返回" @click-left="router.push('/config')" />

    <div class="sub-body">
      <section class="cfg-card" :style="{ borderLeft: '4px solid #06b6d4' }">
        <div class="cfg-card-head">
          <span class="cfg-card-name">触摸加速</span>
          <span class="auto-tag">自动触发</span>
        </div>

        <div class="readout">
          <div><span>当前触摸</span><b>{{ touchState }}</b></div>
          <div><span>最近一次触摸</span><b>{{ ageText }}</b></div>
        </div>

        <DescLines :desc="TOUCH_DESC" />

        <div v-if="tipMsg" class="cfg-banner ok">{{ tipMsg }}</div>
        <ResetDefaultsBtn @reset="resetModuleDefaults" />

        <p class="cfg-intro">验证方式: 手指按住屏幕滑动, 上方"当前触摸"应立即变为"按下中";
        松开后回到"未按下"。</p>
      </section>
    </div>
  </div>
</template>