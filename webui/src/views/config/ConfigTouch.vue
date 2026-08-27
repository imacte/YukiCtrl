<!--
  src/views/config/ConfigTouch.vue — 触摸加速子页 (青色)
  需求升级: 自动触发 → 可配置, 亮屏/息屏两套 (开关/范围/时长).
-->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { fetchSenseSnapshot, type SenseResult } from '@/api/sense'
import { TOUCH_PARAMS } from '@/config/moduleSpecs'
import ScreenScopedModule from '@/components/ScreenScopedModule.vue'

const router = useRouter()
const senseRes = ref<SenseResult | null>(null)
let pollTimer: number | null = null

const snap = computed(() => senseRes.value?.data ?? null)
const touchState = computed(() => (snap.value?.touch_down ? '按下中' : '未按下'))
const ageText = computed(() => {
  const a = Number(snap.value?.touch_age_ms ?? 0)
  if (a <= 0) return '--'
  return a < 1000 ? `${a.toFixed(0)}ms 前` : `${(a / 1000).toFixed(1)}s 前`
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
    <van-nav-bar title="触摸加速" left-arrow left-text="返回" @click-left="router.push('/config')" />
    <div class="sub-body">
      <section class="cfg-card" :style="{ borderLeft: '4px solid #06b6d4' }">
        <div class="cfg-card-head">
          <span class="cfg-card-name">触摸加速</span>
          <span class="live-tag">改动自动生效</span>
        </div>
        <ScreenScopedModule module-key="touch" :params="TOUCH_PARAMS">
          <div class="readout">
            <div><span>当前触摸</span><b>{{ touchState }}</b></div>
            <div><span>最近一次触摸</span><b>{{ ageText }}</b></div>
          </div>
        </ScreenScopedModule>
        <p class="cfg-intro">验证方式: 手指按住屏幕滑动, 上方"当前触摸"应立即变为"按下中"; 松开后回到"未按下"。</p>
      </section>
    </div>
  </div>
</template>
