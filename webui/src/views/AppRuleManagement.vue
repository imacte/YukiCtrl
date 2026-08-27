<!--
  src/views/AppRuleManagement.vue

  任务 #5: App 规则管理页面 (替换旧的 AppRulesView).

  功能:
    - 顶部: FAS-only 提示横幅 (粘性, 不可关闭)
    - 列表: 已配置的 AppRule (包名 / 类型 / 强度 / 禁 burst 标识)
    - 顶部 "添加规则" 按钮: 弹出编辑表单 (van-popup + van-form)
    - 列表项点击: 编辑 / 删除
    - 所有配置项旁加 HelpTooltip
-->
<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Bridge } from '@/utils/bridge'
import { getPackagesInfo } from '@/kernelsu'
import { useSchedulerStore } from '@/stores/scheduler'
import { fetchAppRules, saveAppRule, deleteAppRule, defaultsFor, type AppRule, type RuleType, type RuleStrength } from '@/api/appRules'
import HelpTooltip from '@/components/HelpTooltip.vue'

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
    infos.forEach(info => {
      appLabelMap.value[info.packageName] = info.appLabel
    })
  } catch { /* 忽略 */ }

  rules.value = await fetchAppRules()
  await store.initData()
})

const openAdd = () => {
  editingRule.value = blankRule()
  editing.value = true
}

const openEdit = (r: AppRule) => {
  editingRule.value = JSON.parse(JSON.stringify(r))
  editing.value = true
}

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
  <div class="app-rule-mgmt">
    <van-nav-bar :title="t('app_rule_management')" left-arrow @click-left="$router.back()" fixed placeholder />

    <van-sticky>
      <div class="fas-only-banner">
        <van-icon name="warning-o" size="20" color="#ff976a" />
        <span class="banner-text">{{ t('app_rule_fas_only_notice') }}</span>
        <HelpTooltip
          :title="t('app_rule_fas_only_title')"
          :text="t('app_rule_fas_only_desc')"
        />
      </div>
    </van-sticky>

    <van-search v-model="searchText" :placeholder="t('search_app_or_pkg')" />

    <div class="add-bar">
      <van-button type="primary" block icon="plus" @click="openAdd">{{ t('add_app_rule') }}</van-button>
    </div>

    <van-cell-group v-if="rules.length === 0" inset>
      <div class="empty">
        <van-icon name="apps-o" size="48" color="#ccc" />
        <p>{{ t('no_app_rules_yet') }}</p>
      </div>
    </van-cell-group>

    <van-list v-else>
      <van-cell
        v-for="r in filteredRules"
        :key="r.package"
        :title="getLabel(r.package)"
        :label="r.package"
        clickable
        @click="openEdit(r)"
      >
        <template #value>
          <van-tag :type="r.rule_type === 'restrict' ? 'warning' : 'success'" size="medium">
            {{ typeLabel(r.rule_type) }} · {{ strengthLabel(r.strength) }}
          </van-tag>
          <van-tag v-if="r.disable_burst" type="danger" size="mini" style="margin-left: 4px;">no-burst</van-tag>
        </template>
        <template #right-icon>
          <van-icon name="cross" color="#dc3545" @click.stop="onDelete(r.package)" />
        </template>
      </van-cell>
    </van-list>

    <van-popup v-model:show="editing" position="bottom" round :style="{ height: '78%' }">
      <div class="edit-popup">
        <van-nav-bar :title="editingRule.package ? t('edit_app_rule') : t('add_app_rule')" left-text="取消" @click-left="editing = false" />

        <van-cell-group inset :title="t('choose_package')">
          <van-cell
            :title="editingRule.package ? getLabel(editingRule.package) : t('tap_to_choose')"
            :label="editingRule.package || ''"
            is-link
            @click="appPickerShow = true"
          />
        </van-cell-group>

        <van-cell-group inset :title="t('rule_type')">
          <van-cell>
            <template #value>
              <van-radio-group v-model="editingRule.rule_type" direction="horizontal">
                <van-radio name="restrict">
                  {{ t('rule_type_restrict') }}
                  <HelpTooltip :text="t('rule_type_restrict_desc')" />
                </van-radio>
                <van-radio name="boost" style="margin-left: 16px;">
                  {{ t('rule_type_boost') }}
                  <HelpTooltip :text="t('rule_type_boost_desc')" />
                </van-radio>
              </van-radio-group>
            </template>
          </van-cell>
        </van-cell-group>

        <van-cell-group inset :title="t('rule_strength')">
          <van-cell>
            <template #value>
              <van-radio-group v-model="editingRule.strength" direction="horizontal">
                <van-radio name="light">{{ t('rule_strength_light') }}</van-radio>
                <van-radio name="medium" style="margin-left: 12px;">{{ t('rule_strength_medium') }}</van-radio>
                <van-radio name="heavy" style="margin-left: 12px;">{{ t('rule_strength_heavy') }}</van-radio>
              </van-radio-group>
            </template>
          </van-cell>
        </van-cell-group>

        <van-cell-group inset :title="t('auto_derived_preview')">
          <van-cell :title="t('preview_max_freq_scale')" :value="previewValues.max_freq_scale.toFixed(2)">
            <template #right-icon>
              <HelpTooltip :text="t('preview_max_freq_scale_desc')" />
            </template>
          </van-cell>
          <van-cell :title="t('preview_target_util_offset')" :value="(editingRule.target_util_offset ?? previewValues.target_util_offset) + ''">
            <template #right-icon>
              <HelpTooltip :text="t('preview_target_util_offset_desc')" />
            </template>
          </van-cell>
        </van-cell-group>

<style scoped>
.fas-only-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  background: #fff7e8;
  border-bottom: 1px solid #ffd591;
  color: #874d00;
  font-size: 13px;
  line-height: 1.4;
}
.banner-text { flex: 1; }
.add-bar { margin: 12px 16px; }
.empty {
  padding: 32px 0;
  text-align: center;
  color: #999;
}
.empty p { margin: 8px 0 0; font-size: 13px; }
.edit-popup { background: #f7f8fa; min-height: 100%; }
</style>
        <van-cell-group inset :title="t('advanced_options')">
          <van-cell :title="t('disable_burst')">
            <template #value>
              <van-switch v-model="editingRule.disable_burst" />
            </template>
            <template #right-icon>
              <HelpTooltip :text="t('disable_burst_desc')" />
            </template>
          </van-cell>
          <van-cell :title="t('boost_threshold_offset')">
            <template #value>
              <van-stepper
                v-model="editingRule.boost_threshold_offset"
                :min="-10"
                :max="10"
                :step="1"
              />
            </template>
            <template #right-icon>
              <HelpTooltip :text="t('boost_threshold_offset_desc')" />
            </template>
          </van-cell>
        </van-cell-group>

        <div style="padding: 16px;">
          <van-button type="primary" block @click="onSave">{{ t('save') }}</van-button>
        </div>
      </div>
    </van-popup>

    <van-popup v-model:show="appPickerShow" position="bottom" round :style="{ height: '60%' }">
      <van-nav-bar :title="t('choose_package')" />
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