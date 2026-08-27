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
import { ref } from 'vue'

defineProps<{
  /** 标题 (可选) */
  title?: string
  /** 正文 (傻瓜化中文, 必填) */
  text: string
  /** 按钮大小 px */
  size?: number
}>()

const show = ref(false)
</script>

<template>
  <span class="help-tip" :style="{ width: (size ?? 18) + 'px', height: (size ?? 18) + 'px' }" @click.stop="show = true">
    <van-icon name="question-o" :size="size ?? 16" color="#1989fa" />

    <van-popup v-model:show="show" position="top" round :style="{ background: '#fff3cd', color: '#664d03' }">
      <div class="help-popup">
        <h4 v-if="title" class="help-title">{{ title }}</h4>
        <p class="help-text">{{ text }}</p>
        <van-button size="small" type="warning" @click="show = false">知道了</van-button>
      </div>
    </van-popup>
  </span>
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
.help-tip:hover {
  background: rgba(25, 137, 250, 0.1);
}
.help-popup {
  padding: 16px;
  max-width: 280px;
}
.help-title {
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
}
.help-text {
  margin: 0 0 12px;
  font-size: 13px;
  line-height: 1.5;
}
</style>