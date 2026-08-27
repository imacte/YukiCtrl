<!--
  src/components/ResetDefaultsBtn.vue — 恢复默认按钮 (模块页底部通用组件)

  用法:
    <ResetDefaultsBtn @reset="onReset" />                      // 模块级: 恢复本模块默认值
    <ResetDefaultsBtn label="恢复全部默认值" danger @reset="onResetAll" />  // 全局级
  reset 事件在用户确认弹窗后触发; 具体恢复逻辑由父页面实现 (写盘即热生效)。
-->
<script setup lang="ts">
import { ref } from 'vue'

withDefaults(defineProps<{ label?: string; danger?: boolean }>(), {
  label: '恢复本模块默认值',
  danger: false,
})
const emit = defineEmits<{ (e: 'reset'): void }>()
const show = ref(false)

function onConfirm() { emit('reset') }
</script>

<template>
  <div class="reset-wrap">
    <button class="reset-btn" :class="{ danger }" @click="show = true">{{ label }}</button>
    <van-dialog
      v-model:show="show"
      title="恢复默认值"
      show-cancel-button
      confirm-button-text="恢复"
      cancel-button-text="取消"
      @confirm="onConfirm"
    >
      <div class="reset-confirm-text">
        {{ danger
          ? '将把所有模块、所有档位、亮屏/息屏的全部参数恢复为默认值, 立即生效并保存。应用专属规则与当前档位保留。'
          : '将把本模块的参数恢复为默认值, 立即生效并保存。' }}
      </div>
    </van-dialog>
  </div>
</template>

<style scoped>
.reset-wrap { margin-top: 20px; padding-top: 16px; border-top: 1px dashed var(--border); }
.reset-btn {
  width: 100%; padding: 11px 0; border-radius: 10px;
  border: 1px solid var(--border-strong);
  background: var(--bg-card); color: var(--text-secondary);
  font-size: 14.5px; font-weight: 600;
}
.reset-btn:active { opacity: 0.7; }
.reset-btn.danger { border-color: #b91c1c; color: #b91c1c; }
.reset-confirm-text {
  padding: 18px 22px; font-size: 14px; line-height: 1.6;
  color: var(--text-primary);
}
</style>
