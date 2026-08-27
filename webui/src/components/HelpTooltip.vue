<!--
  src/components/HelpTooltip.vue

  任务 #5: 通用帮助气泡 — 每个配置项右侧加 "?" 按钮, 点击显示傻瓜化中文说明.

  用法:
    <HelpTooltip text="禁掉突发高频, 避免耗电" />
    <HelpTooltip :title="什么是 FAS" text="..." />

  设计:
    - vant 的 van-popup + icon-question-o 实现
    - 默认浅黄底 (类似 MIUI 设置页提示)
    - 完全无外部依赖
-->
<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue'
import { DESC_KEYS } from '@/config/moduleSpecs'

// 问题 2 修复版: 三重关闭保障 (遮罩 / 按钮 / 10 秒超时), popup teleport 到 body
// 脱离父级 @click.stop 事件流, 不可能再被外层拦截导致卡死.
const props = defineProps<{
  /** 标题 (可选) */
  title?: string
  /** 正文 (一句话说明; list 未传时使用) */
  text?: string
  /** 五维说明数组 [是什么, 调高, 调低, 何时调, 建议值] (传入时优先渲染) */
  list?: string[]
  /** 按钮大小 px */
  size?: number
}>()

const show = ref(false)
let autoTimer: number | null = null

function open() { show.value = true }
function close() { show.value = false }

watch(show, v => {
  if (autoTimer !== null) { window.clearTimeout(autoTimer); autoTimer = null }
  if (v) autoTimer = window.setTimeout(() => { show.value = false }, 10_000)
})
onUnmounted(() => { if (autoTimer !== null) window.clearTimeout(autoTimer) })

void props
</script>

<template>
  <span class="help-tip" :style="{ width: (size ?? 18) + 'px', height: (size ?? 18) + 'px' }" @click.stop.prevent="open">
    <van-icon name="question-o" :size="size ?? 16" color="#1989fa" />
  </span>

  <van-popup
    teleport="body"
    :show="show"
    round
    close-on-click-overlay
    :style="{ width: 'min(86vw, 340px)' }"
    @click-overlay="close"
    @update:show="(v: boolean) => { if (!v) close() }"
  >
    <div class="help-popup">
      <div class="help-head">
        <span class="help-title">{{ title ?? '说明' }}</span>
        <van-icon name="cross" size="18" color="#9ca3af" class="help-x" @click="close" />
      </div>

      <div v-if="list && list.length" class="help-list">
        <div v-for="(d, i) in list" :key="i" class="hd-line">
          <span class="hd-k">{{ DESC_KEYS[i] ?? '' }} ·</span>
          <span class="hd-v">{{ d }}</span>
        </div>
      </div>
      <p v-else class="help-text">{{ text }}</p>

      <van-button block size="small" type="primary" @click="close">知道了</van-button>
    </div>
  </van-popup>
</template>

<style scoped>
.help-tip {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  margin-left: 4px;
  border-radius: 50%;
  transition: background 0.2s;
}
.help-tip:active { background: rgba(25, 137, 250, 0.15); }
.help-popup { padding: 16px; }
.help-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
.help-title { font-size: 15px; font-weight: 700; color: var(--text-primary); }
.help-x { padding: 4px; cursor: pointer; }
.help-text { margin: 0 0 12px; font-size: 13px; line-height: 1.6; color: var(--text-secondary); }

.help-list { margin-bottom: 12px; }
.hd-line { display: flex; gap: 6px; font-size: 12.5px; line-height: 1.7; color: var(--text-secondary); margin-bottom: 6px; }
.hd-k { flex-shrink: 0; color: var(--accent); font-weight: 600; white-space: nowrap; }
.hd-v { flex: 1; }
</style>