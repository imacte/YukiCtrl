<!--
  src/components/DescLines.vue — 大白话五维说明 (统一渲染)

  五维: 是什么 / 调高会怎样 / 调低会怎样 / 什么情况建议调 / 建议值
  全 WebUI 所有参数说明都通过本组件展示, 保证格式一致.
-->
<script setup lang="ts">
import { DESC_KEYS } from '@/config/moduleSpecs'

defineProps<{ desc: string[] }>()
</script>

<template>
  <div class="d-wrap">
    <div v-for="(d, i) in desc" :key="i" class="d-line">
      <span class="d-k">{{ DESC_KEYS[i] ?? '' }} ·</span>
      <span class="d-v">{{ d }}</span>
    </div>
  </div>
</template>

<style scoped>
.d-wrap {
  margin-top: 7px; padding: 9px 10px;
  background: var(--bg-base); border-radius: 8px;
}
.d-line { display: flex; gap: 6px; font-size: 12px; line-height: 1.65; color: var(--text-secondary); }
.d-k { flex-shrink: 0; color: var(--accent); font-weight: 600; white-space: nowrap; }
.d-v { flex: 1; }
</style>