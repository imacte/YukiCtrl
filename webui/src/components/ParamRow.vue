<!--
  src/components/ParamRow.vue — 单个配置参数行 (滑条 / 下拉 / 数字输入 + 五维说明)

  事件: update — 已按 spec 做好类型转换:
    - select  → 永远字符串
    - range/num → 数字; spec.asString 为真时转字符串 (后端 serde 是 String, 传数字会炸)
-->
<script setup lang="ts">
import { type ParamSpec, fmtVal } from '@/config/moduleSpecs'
import DescLines from './DescLines.vue'

const props = defineProps<{ spec: ParamSpec; value: unknown }>()
const emit = defineEmits<{ (e: 'update', v: unknown): void }>()

function onRange(e: Event) {
  const n = Number((e.target as HTMLInputElement).value)
  emit('update', props.spec.asString ? String(n) : n)
}
function onSelect(e: Event) {
  emit('update', (e.target as HTMLSelectElement).value)
}
function onNum(e: Event) {
  const raw = (e.target as HTMLInputElement).value
  const n = Number(raw)
  emit('update', Number.isFinite(n) ? (props.spec.asString ? String(n) : n) : raw)
}
</script>

<template>
  <div class="param">
    <div class="param-head">
      <span class="p-label">{{ spec.label }}</span>
      <span v-if="spec.type !== 'select'" class="p-val">{{ fmtVal(spec, value ?? spec.fb ?? 0) }}</span>
    </div>

    <select
      v-if="spec.type === 'select'"
      class="p-select"
      :value="String(value ?? spec.fb ?? '')"
      @change="onSelect"
    >
      <option v-for="o in spec.options" :key="String(o.v)" :value="String(o.v)">{{ o.n }}</option>
    </select>

    <input
      v-else-if="spec.type === 'range'"
      type="range" class="p-range"
      :min="spec.min" :max="spec.max" :step="spec.step"
      :value="Number(value ?? spec.fb ?? 0)"
      @input="onRange"
    />

    <input
      v-else
      type="number" class="p-input num"
      :min="spec.min" :max="spec.max" :step="spec.step"
      :value="Number(value ?? spec.fb ?? 0)"
      @change="onNum"
    />

    <DescLines :desc="spec.desc" />
  </div>
</template>

<style scoped>
.param { margin-top: 16px; padding-top: 12px; border-top: 1px dashed var(--border); }
.param:first-of-type { border-top: none; margin-top: 4px; }
.param-head { display: flex; justify-content: space-between; align-items: baseline; }
.p-label { font-size: 14.5px; font-weight: 600; color: var(--text-primary); }
.p-val { font-size: 13.5px; font-weight: 600; color: var(--accent); font-variant-numeric: tabular-nums; }

.p-range { width: 100%; accent-color: var(--accent); height: 30px; }
.p-select {
  width: 100%; margin-top: 6px; padding: 9px 10px;
  border: 1px solid var(--border-strong); border-radius: 8px;
  background: var(--bg-card); color: var(--text-primary); font-size: 13.5px;
}
.p-input {
  width: 100%; box-sizing: border-box; margin-top: 6px; padding: 9px 10px;
  border: 1px solid var(--border-strong); border-radius: 8px;
  font-size: 14px; color: var(--text-primary);
}
.p-input.num { width: 120px; }
</style>