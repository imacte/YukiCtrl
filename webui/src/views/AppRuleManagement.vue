<!--
  src/views/AppRuleManagement.vue

  App \u89c4\u5219\u7ba1\u7406 (\u6697\u8272)
-->
<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Bridge } from '@/utils/bridge'
import { getPackagesInfo } from '@/kernelsu'
import { useSchedulerStore } from '@/stores/scheduler'
import { fetchAppRules, saveAppRule, deleteAppRule, defaultsFor, type AppRule, type RuleType, type RuleStrength } from '@/api/appRules'
import HelpTooltip from '@/components/HelpTooltip.vue'
import DescLines from '@/components/DescLines.vue'

const { t } = useI18n()
const store = useSchedulerStore()

const allApps = ref<string[]>([])
const appLabelMap = ref<Record<string, string>>({})
const rules = ref<AppRule[]>([])
const searchText = ref('')

const editing = ref(false)
const editingRule = ref<AppRule>(blankRule())

function blankRule(): AppRule {
  return { package: '', rule_type: 'restrict', strength: 'medium' }
}

const filteredRules = computed(() => {
  const q = searchText.value.toLowerCase()
  if (!q) return rules.value
  return rules.value.filter(r =>
    r.package.toLowerCase().includes(q) ||
    (appLabelMap.value[r.package] || '').toLowerCase().includes(q)
  )
})

const getLabel = (pkg: string) => appLabelMap.value[pkg] || pkg

onMounted(async () => {
  const pkgs = await Bridge.getInstalledApps()
  allApps.value = pkgs
  try {
    const infos = getPackagesInfo(pkgs)
    infos.forEach(info => { appLabelMap.value[info.packageName] = info.appLabel })
  } catch { /* ignore */ }
  rules.value = await fetchAppRules()
  await store.initData()
})

const openAdd = () => { editingRule.value = blankRule(); editing.value = true }
const openEdit = (r: AppRule) => { editingRule.value = JSON.parse(JSON.stringify(r)); editing.value = true }
const onSave = async () => {
  if (!editingRule.value.package) return
  await saveAppRule(editingRule.value)
  rules.value = await fetchAppRules()
  editing.value = false
}
const onDelete = async (pkg: string) => {
  await deleteAppRule(pkg)
  rules.value = await fetchAppRules()
}

const strengthLabel = (s?: RuleStrength) => {
  switch (s) {
    case 'light': return t('rule_strength_light')
    case 'heavy': return t('rule_strength_heavy')
    default: return t('rule_strength_medium')
  }
}
const typeLabel = (t: RuleType) => t === 'restrict' ? t('rule_type_restrict') : t('rule_type_boost')

const previewValues = computed(() => {
  const r = editingRule.value
  const s = r.strength ?? 'medium'
  return defaultsFor(r.rule_type, s)
})

/* ==================== 大白话五维说明 (问题 1) ==================== */
const TYPE_RESTRICT_DESC: [string, string, string, string, string] = [
  '限制规则: 给指定应用"踩刹车" — 压低它允许的最高频率、提前降频。',
  '(限制更狠) 该应用更省电、发热更小, 但动画和加载明显变慢。',
  '(限制更松) 应用更流畅, 省电效果打折扣。',
  '给后台工具、不常看画质的应用、纯文字类应用加限制。',
  '后台下载器/天气/输入法建议加"轻度"限制。']

const TYPE_BOOST_DESC: [string, string, string, string, string] = [
  '加速规则: 给指定应用"踩油门" — 抬高频率上限、更早升频。',
  '(加速更狠) 游戏更稳更顺, 发热耗电明显增加。',
  '(加速更松) 省电一些, 重负载场景可能偶有掉帧。',
  '给大型游戏、视频剪辑、直播推流类应用加加速。',
  '重度游戏加"重度"加速; 网游加"中度"即可。']

const STRENGTH_DESC: [string, string, string, string, string] = [
  '规则的作用幅度。轻度约 ±5% 频率 / 10 个百分点利用率; 中度 ±10% / 20; 重度 ±20% / 35。',
  '(选更重) 效果翻倍 — 限制更省电、加速更流畅, 副作用也更大。',
  '(选更轻) 影响温和, 几乎无副作用但改善也小。',
  '先从中度试起, 观察一两天再决定加重或减轻。',
  '默认中度; 电竞重度, 工具类轻度。']

const DISABLE_BURST_DESC: [string, string, string, string, string] = [
  '开关"突发高频": 系统在检测到瞬间负载尖峰时会把频率猛拉到最高, 这个开关把它禁掉。',
  '(开启禁用) 该应用不再瞬间拉满频率, 省电降温明显, 瞬时操作可能慢几十毫秒。',
  '(关闭禁用) 允许突发高频, 点击响应最快, 但更容易发热。',
  '耗电敏感的常驻应用建议开启; 游戏类一定不要开。',
  '限制规则建议开启, 加速规则保持关闭。']

const BOOST_OFFSET_DESC: [string, string, string, string, string] = [
  '开核阈值微调 (仅加速规则生效)。负数 = 更早唤醒核心, 正数 = 更晚。',
  '(调负) 一点小负载就唤醒核心, 跟手但费电。',
  '(调正) 只有大负载才唤醒核心, 省电但响应略慢。',
  '抢购、音游等需要极限响应的场景调 -3 ~ -5。',
  '保持 0 不动即可, 绝大多数场景感知不到差别。']

const FREQ_SCALE_DESC: [string, string, string, string, string] = [
  '频率上限倍率: 1.00 = 不变; 限制规则 < 1 (压低上限), 加速规则 > 1 (抬高上限)。',
  '(数值更大) 应用可用的最高频率更高, 更流畅也更热。',
  '(数值更小) 频率被压住, 更凉快省电。',
  '由类型 + 强度自动推导, 无需手算; 仅想微调时改高级自定义。',
  '跟随上方预览值即可。']

const UTIL_OFFSET_DESC: [string, string, string, string, string] = [
  '利用率偏移: 帧平滑引擎判断"该不该提频"的灵敏度修正, 单位是百分点。',
  '(正值) 更容易判定为"忙" → 提频更积极。',
  '(负值) 更容易判定为"闲" → 提频更保守。',
  '由类型 + 强度自动推导; 游戏团战掉帧时可手动加 5 试试。',
  '保持自动预览值。']

const appPickerShow = ref(false)
const appSearchText = ref('')
const filteredAppsForPicker = computed(() => {
  const q = appSearchText.value.toLowerCase()
  if (!q) return allApps.value
  return allApps.value.filter(p => p.toLowerCase().includes(q) || (appLabelMap.value[p] || '').toLowerCase().includes(q))
})
const pickApp = (pkg: string) => {
  editingRule.value.package = pkg
  appPickerShow.value = false
}
</script>
<template>
  <div class="rule-page">
    <div class="page-header">
      <span class="page-title">{{ t('app_rule_management') }}</span>
      <van-button type="primary" size="small" @click="openAdd">{{ t('add_app_rule') }}</van-button>
    </div>

    <div class="fas-only-banner">
      <van-icon name="info-o" />
      <span class="banner-text">{{ t('app_rule_fas_only_notice') }}</span>
    </div>

    <van-search v-model="searchText" :placeholder="t('search_apps')" />

    <div class="rule-list" v-if="filteredRules.length">
      <div v-for="r in filteredRules" :key="r.package" class="rule-card" @click="openEdit(r)">
        <div class="rule-row">
          <div>
            <div class="rule-name">{{ getLabel(r.package) }}</div>
            <div class="rule-pkg">{{ r.package }}</div>
          </div>
          <van-icon name="delete-o" @click.stop="onDelete(r.package)" color="var(--danger)" />
        </div>
        <div class="rule-meta">
          <span class="rule-tag" :class="r.rule_type">{{ typeLabel(r.rule_type) }}</span>
          <span class="rule-tag">{{ strengthLabel(r.strength) }}</span>
          <span v-if="r.disable_burst" class="rule-tag">禁突发</span>
        </div>
      </div>
    </div>

    <div class="empty" v-else>
      <van-icon name="apps-o" size="48" color="var(--text-muted)" />
      <p>{{ t('no_app_rules_yet') }}</p>
    </div>

    <van-popup v-model:show="editing" position="bottom" round :style="{ height: '85%' }">
      <div class="edit-popup">
        <div class="page-header">
          <span class="page-title">{{ editingRule.package ? t('edit_app_rule') : t('add_app_rule') }}</span>
          <van-button type="primary" size="small" @click="onSave">{{ t('save') }}</van-button>
        </div>

        <div class="card">
          <div class="card-title">{{ t('choose_package') }}</div>
          <div class="config-row" @click="appPickerShow = true" style="cursor: pointer;">
            <div class="row-text">
              <span>{{ getLabel(editingRule.package) || t('tap_to_choose') }}</span>
            </div>
            <van-icon name="arrow" />
          </div>
        </div>

        <div class="card">
          <div class="card-title">{{ t('rule_type') }}</div>
          <van-radio-group v-model="editingRule.rule_type" direction="horizontal">
            <van-radio name="restrict">{{ t('rule_type_restrict') }}</van-radio>
            <van-radio name="boost" style="margin-left: 12px;">{{ t('rule_type_boost') }}</van-radio>
          </van-radio-group>
          <div style="margin-top: 10px;">
            <DescLines :desc="editingRule.rule_type === 'restrict' ? TYPE_RESTRICT_DESC : TYPE_BOOST_DESC" />
          </div>
        </div>

        <div class="card">
          <div class="card-title">{{ t('rule_strength') }}</div>
          <van-radio-group v-model="editingRule.strength" direction="horizontal">
            <van-radio name="light">{{ t('rule_strength_light') }}</van-radio>
            <van-radio name="medium" style="margin-left: 12px;">{{ t('rule_strength_medium') }}</van-radio>
            <van-radio name="heavy" style="margin-left: 12px;">{{ t('rule_strength_heavy') }}</van-radio>
          </van-radio-group>
          <div style="margin-top: 10px;">
            <DescLines :desc="STRENGTH_DESC" />
          </div>
        </div>

        <div class="card">
          <div class="card-title">{{ t('auto_derived_preview') }}</div>
          <div class="config-row">
            <div class="row-text">
              <span>频率上限倍率</span>
              <HelpTooltip title="频率上限倍率" :list="FREQ_SCALE_DESC" />
            </div>
            <span class="val">{{ previewValues.max_freq_scale.toFixed(2) }}</span>
          </div>
          <div class="config-row">
            <div class="row-text">
              <span>利用率偏移</span>
              <HelpTooltip title="利用率偏移" :list="UTIL_OFFSET_DESC" />
            </div>
            <span class="val">{{ editingRule.target_util_offset ?? previewValues.target_util_offset }}</span>
          </div>
        </div>

        <div class="card">
          <div class="card-title">{{ t('advanced_options') }}</div>
          <div class="config-row">
            <div class="row-text">
              <span>禁用突发高频</span>
              <HelpTooltip title="禁用突发高频" :list="DISABLE_BURST_DESC" />
            </div>
            <van-switch v-model="editingRule.disable_burst" />
          </div>
          <div class="config-row">
            <div class="row-text">
              <span>开核阈值微调</span>
              <HelpTooltip title="开核阈值微调" :list="BOOST_OFFSET_DESC" />
            </div>
            <van-stepper v-model="editingRule.boost_threshold_offset" :min="-10" :max="10" :step="1" />
          </div>
        </div>
      </div>
    </van-popup>

    <van-popup v-model:show="appPickerShow" position="bottom" round :style="{ height: '60%' }">
      <div class="page-header"><span class="page-title">{{ t('choose_package') }}</span></div>
      <van-search v-model="appSearchText" :placeholder="t('search_app_or_pkg')" />
      <van-cell
        v-for="pkg in filteredAppsForPicker"
        :key="pkg"
        :title="getLabel(pkg)"
        :label="pkg"
        clickable
        @click="pickApp(pkg)"
      >
        <template #icon>
          <img :src="`ksu://icon/${pkg}`" style="width: 32px; height: 32px; margin-right: 8px; border-radius: 6px;" loading="lazy" />
        </template>
      </van-cell>
    </van-popup>
  </div>
</template>
<style scoped>
.rule-page {
  padding: 0;
  max-width: 600px;
  margin: 0 auto;
}
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px;
}
.page-title { font-size: 20px; font-weight: 600; }
.fas-only-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  background: rgba(245, 158, 11, 0.12);
  border: 1px solid rgba(245, 158, 11, 0.3);
  color: var(--warning);
  font-size: 13px;
  line-height: 1.4;
  margin: 0 12px 12px;
  border-radius: 8px;
}
.banner-text { flex: 1; }
.rule-list { padding: 0 12px; }
.rule-card {
  background: var(--bg-card);
  border-radius: 12px;
  padding: 12px;
  margin-bottom: 8px;
  border: 1px solid var(--border);
  cursor: pointer;
}
.rule-card:active { background: var(--bg-card-hover); }
.rule-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.rule-name { font-size: 14px; color: var(--text-primary); font-weight: 600; }
.rule-pkg {
  font-size: 11px;
  color: var(--text-muted);
  font-family: monospace;
  margin-top: 2px;
}
.rule-meta {
  display: flex;
  gap: 6px;
  margin-top: 8px;
}
.rule-tag {
  background: rgba(76, 154, 255, 0.15);
  color: var(--accent);
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 11px;
}
.rule-tag.boost {
  background: rgba(245, 158, 11, 0.15);
  color: var(--warning);
}
.rule-tag.restrict {
  background: rgba(16, 185, 129, 0.15);
  color: var(--success);
}
.empty {
  padding: 60px 0;
  text-align: center;
  color: var(--text-muted);
}
.empty p { margin: 12px 0 0; font-size: 13px; }

.edit-popup { background: var(--bg-base); min-height: 100%; }

.card {
  background: var(--bg-card);
  border-radius: 12px;
  padding: 16px;
  margin: 12px;
  border: 1px solid var(--border);
}
.card-title {
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 12px;
  color: var(--accent);
}
.config-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 0;
  border-top: 1px solid var(--border);
}
.config-row:first-of-type { border-top: 0; }
.row-text {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  font-size: 13px;
}
.val {
  font-size: 13px;
  color: var(--accent);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.hint {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 8px;
  line-height: 1.5;
}
</style>
