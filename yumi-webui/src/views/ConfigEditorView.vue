<script setup lang="ts">
import { ref, onMounted, h, defineComponent } from 'vue';
import { useI18n } from 'vue-i18n';
import { Bridge } from '@/utils/bridge';
import { showToast, Collapse, CollapseItem, Cell, Switch } from 'vant';

// --- 状态定义 ---
const { t } = useI18n();
const activeTab = ref('rules');
const currentData = ref<any>({});
const loading = ref(false);
const activeNames = ref<string[]>([]);

// 编辑弹窗状态
const showEditDialog = ref(false);
const editingKeyPath = ref('');
const editingValue = ref('');
const editingType = ref<'string' | 'number' | 'array'>('string');

// 通用选择器配置
const modeActions = [{ name: 'powersave', color: '#4CAF50' }, { name: 'balance', color: '#2196F3' }, { name: 'performance', color: '#FF9800' }, { name: 'fast', color: '#F44336' }];
const appModeActions = [{ name: 'powersave', color: '#4CAF50' }, { name: 'balance', color: '#2196F3' }, { name: 'performance', color: '#FF9800' }, { name: 'fast', color: '#F44336' }, { name: 'fas', color: '#E91E63' }, { name: '删除该规则', color: '#FF0000', isDelete: true }];
const languageActions = [{ name: 'zh' }, { name: 'en' }];
const loglevelActions = [{ name: 'OFF' }, { name: 'ERROR' }, { name: 'WARN' }, { name: 'INFO' }, { name: 'DEBUG' }, { name: 'TRACE' }];

const showModeSheet = ref(false);
const showAppModeSheet = ref(false);
const showLanguageSheet = ref(false);
const showLoglevelSheet = ref(false);

// ===== CPU 频率 / 调速器 动态选项状态 =====
const freqsMap = ref<Record<number, string[]>>({});
const govsMap = ref<Record<number, string[]>>({});
const corePathMap = ref<Record<string, number>>({
  SmallCore: -1,
  MediumCore: -1,
  BigCore: -1,
  SuperBigCore: -1,
});

const showFreqSheet = ref(false);
const freqSheetActions = ref<{ name: string; subname?: string }[]>([]);
const showGovSheet = ref(false);
const govSheetActions = ref<{ name: string; subname?: string }[]>([]);

// pGovPath 添加调速器用
const showAddGovSheet = ref(false);
const showAddGovDialog = ref(false);
const newGovName = ref('');

// pGovPath/{gov} 添加 path 用
const showAddPathDialog = ref(false);
const newPathName = ref('');
const addingPathUnderGov = ref('');

const getCoreTypeFromKey = (key: string): string | null => {
  if (key.startsWith('SuperBigCore')) return 'SuperBigCore';
  if (key.startsWith('BigCore')) return 'BigCore';
  if (key.startsWith('MediumCore')) return 'MediumCore';
  if (key.startsWith('SmallCore')) return 'SmallCore';
  return null;
};

const isFreqField = (path: string): boolean => {
  const parts = path.split('/');
  return parts.includes('Freq') && getCoreTypeFromKey(parts[parts.length - 1] || '') !== null;
};

const isGovField = (path: string): boolean => {
  const parts = path.split('/');
  const key = parts[parts.length - 1] || '';
  // global 也走调速器选择器；其余核心字段同样走
  return parts.includes('Governor') && (key === 'global' || getCoreTypeFromKey(key) !== null);
};

const getPolicyForKey = (key: string): number => {
  const coreType = getCoreTypeFromKey(key);
  if (!coreType) return -1;
  return corePathMap.value[coreType] ?? -1;
};

const formatFreq = (kHz: string): string => {
  const num = parseInt(kHz, 10);
  if (isNaN(num)) return kHz;
  const ghz = (num / 1000000).toFixed(2);
  return `${kHz} kHz  (${ghz} GHz)`;
};

const getAllGovernors = (): { name: string }[] => {
  const set = new Set<string>();
  Object.values(govsMap.value).forEach(list => list.forEach(g => set.add(g)));
  if (set.size === 0) return [];
  // 排除 pGovPath 中已存在的调速器
  const existing = new Set<string>(Object.keys(currentData.value.pGovPath || {}));
  return Array.from(set).filter(g => !existing.has(g)).map(g => ({ name: g }));
};

const loadCpuInfo = async () => {
  try {
    const cfg = activeTab.value === 'config' && Object.keys(currentData.value).length > 0
      ? currentData.value
      : await Bridge.getMainConfig();

    const cf = cfg.CoreFramework || {};
    corePathMap.value = {
      SmallCore:    typeof cf.SmallCorePath    === 'number' ? cf.SmallCorePath    : -1,
      MediumCore:   typeof cf.MediumCorePath   === 'number' ? cf.MediumCorePath   : -1,
      BigCore:      typeof cf.BigCorePath      === 'number' ? cf.BigCorePath      : -1,
      SuperBigCore: typeof cf.SuperBigCorePath === 'number' ? cf.SuperBigCorePath : -1,
    };

    const policies = await Bridge.getCpuPolicies();
    await Promise.all(policies.map(async (p) => {
      const [freqs, govs] = await Promise.all([
        Bridge.getAvailableFreqs(p),
        Bridge.getAvailableGovernors(p),
      ]);
      freqsMap.value[p] = freqs;
      govsMap.value[p] = govs;
    }));
  } catch (e) {
    console.warn('loadCpuInfo failed:', e);
  }
};

// --- pGovPath：添加调速器 ---
const openAddGov = () => {
  newGovName.value = '';
  const govList = getAllGovernors();
  if (govList.length > 0) {
    showAddGovSheet.value = true;
  } else if (Object.keys(govsMap.value).length === 0) {
    // sysfs 未读到数据，退化为文本输入
    showAddGovDialog.value = true;
  } else {
    // sysfs 有数据但所有调速器都已添加
    showToast('所有调速器均已添加');
  }
};

const doAddGov = (name: string) => {
  if (!name.trim()) { showToast(t('name_empty')); return; }
  if (!currentData.value.pGovPath) currentData.value.pGovPath = {};
  if (currentData.value.pGovPath[name] !== undefined) { showToast(t('governor_exists')); return; }
  currentData.value.pGovPath[name] = {};
  saveConfig();
  const newPath = `pGovPath/${name}`;
  if (!activeNames.value.includes(newPath)) activeNames.value.push(newPath);
};

const onSelectAddGov = (a: any) => { doAddGov(a.name); showAddGovSheet.value = false; };
const confirmAddGov = () => { doAddGov(newGovName.value.trim()); newGovName.value = ''; showAddGovDialog.value = false; };

// --- pGovPath/{gov}：添加 path ---
const confirmAddPath = () => {
  const name = newPathName.value.trim();
  if (!name) { showToast(t('name_empty')); return; }
  const gov = addingPathUnderGov.value;
  if (!currentData.value.pGovPath?.[gov]) currentData.value.pGovPath[gov] = {};
  if (currentData.value.pGovPath[gov][name] !== undefined) { showToast(t('path_exists')); return; }
  currentData.value.pGovPath[gov][name] = '';
  saveConfig();
  newPathName.value = '';
  showAddPathDialog.value = false;
};

// --- 递归收集需要展开的路径（深度 < 2） ---
const isObj = (v: any) => v && typeof v === 'object' && !Array.isArray(v);

const collectExpandPaths = (obj: any, basePath: string): string[] => {
  const result: string[] = [];
  if (!isObj(obj)) return result;
  Object.entries(obj).forEach(([key, val]) => {
    if (isObj(val)) {
      const p = basePath ? `${basePath}/${key}` : key;
      const depth = p.split('/').length - 1;
      if (depth < 2) {
        result.push(p);
        result.push(...collectExpandPaths(val, p));
      }
    }
  });
  return result;
};

// --- 数据处理 ---
const loadData = async () => {
  loading.value = true;
  try {
    currentData.value = activeTab.value === 'rules'
      ? await Bridge.getRulesConfig()
      : await Bridge.getMainConfig();
    activeNames.value = collectExpandPaths(currentData.value, '');
    await loadCpuInfo();
  } catch (e) {
    showToast(t('load_failed'));
  } finally {
    loading.value = false;
  }
};

onMounted(loadData);

const saveConfig = async () => {
  try {
    if (activeTab.value === 'rules') await Bridge.saveRulesConfig(currentData.value);
    else await Bridge.saveMainConfig(currentData.value);
    showToast(t('saved'));
  } catch (e) {
    showToast({ type: 'fail', message: t('save_failed') });
  }
};

const setDeepValue = (obj: any, path: string, value: any) => {
  const keys = path.split('/');
  let current = obj;
  for (let i = 0; i < keys.length - 1; i++) {
    const k = keys[i] as string;
    if (!current[k]) current[k] = {};
    current = current[k];
  }
  current[keys[keys.length - 1] as string] = value;
};

// --- 点击处理 ---
const handleItemClick = (fullPath: string, value: any) => {
  if (typeof value === 'boolean') return;

  editingKeyPath.value = fullPath;

  if (fullPath === 'global_mode') { showModeSheet.value = true; return; }
  if (fullPath.startsWith('app_modes/')) { showAppModeSheet.value = true; return; }
  if (fullPath === 'meta/language') { showLanguageSheet.value = true; return; }
  if (fullPath === 'meta/loglevel') { showLoglevelSheet.value = true; return; }

  // 频率字段 → 动态选择器
  if (isFreqField(fullPath)) {
    const fieldKey = fullPath.split('/').pop() || '';
    const policy = getPolicyForKey(fieldKey);
    const freqs = policy >= 0 ? (freqsMap.value[policy] || []) : [];

    if (freqs.length === 0) {
      editingType.value = typeof value === 'number' ? 'number' : 'string';
      editingValue.value = String(value);
      showEditDialog.value = true;
      return;
    }

    freqSheetActions.value = [
      { name: 'min', subname: '最低频率（动态）' },
      { name: 'max', subname: '最高频率（动态）' },
      ...[...freqs].reverse().map(f => ({ name: String(f), subname: formatFreq(f) })),
    ];
    showFreqSheet.value = true;
    return;
  }

  // 调速器字段 → 动态选择器
  if (isGovField(fullPath)) {
    const fieldKey = fullPath.split('/').pop() || '';
    const policy = getPolicyForKey(fieldKey);
    const govs = policy >= 0 ? (govsMap.value[policy] || []) : [];

    if (govs.length === 0) {
      editingType.value = 'string';
      editingValue.value = String(value);
      showEditDialog.value = true;
      return;
    }

    govSheetActions.value = [
      // global 本身就是全局，不需要"跟随全局"空选项
      ...(fieldKey === 'global' ? [] : [{ name: '', subname: '跟随全局 Governor' }]),
      ...govs.map(g => ({ name: g, subname: '' })),
    ];
    showGovSheet.value = true;
    return;
  }

  // 默认：普通输入框
  if (Array.isArray(value)) { editingType.value = 'array'; editingValue.value = value.join(', '); }
  else if (typeof value === 'number') { editingType.value = 'number'; editingValue.value = String(value); }
  else { editingType.value = 'string'; editingValue.value = String(value); }
  showEditDialog.value = true;
};

const confirmEdit = () => {
  let val: any = editingValue.value;
  if (editingType.value === 'number') val = Number(val);
  if (editingType.value === 'array') {
    val = val.split(',').map((s: string) => {
      const trimmed = s.trim();
      return isNaN(Number(trimmed)) ? trimmed : Number(trimmed);
    }).filter((s: any) => s !== '');
  }
  setDeepValue(currentData.value, editingKeyPath.value, val);
  saveConfig();
  showEditDialog.value = false;
};

// --- 选择器回调 ---
const onSelectMode = (a: any) => { currentData.value.global_mode = a.name; saveConfig(); showModeSheet.value = false; };
const onSelectAppMode = (a: any) => {
  const pkg = editingKeyPath.value.split('/').pop() || '';
  if (a.isDelete) delete currentData.value.app_modes[pkg];
  else currentData.value.app_modes[pkg] = a.name;
  saveConfig(); showAppModeSheet.value = false;
};
const onSelectLanguage = (a: any) => { setDeepValue(currentData.value, 'meta/language', a.name); saveConfig(); showLanguageSheet.value = false; };
const onSelectLoglevel = (a: any) => { setDeepValue(currentData.value, 'meta/loglevel', a.name); saveConfig(); showLoglevelSheet.value = false; };

const onSelectFreq = (a: any) => {
  const val = (a.name === 'min' || a.name === 'max') ? a.name : Number(a.name);
  setDeepValue(currentData.value, editingKeyPath.value, val);
  saveConfig();
  showFreqSheet.value = false;
};

const onSelectGov = (a: any) => {
  setDeepValue(currentData.value, editingKeyPath.value, a.name);
  saveConfig();
  showGovSheet.value = false;
};
// ===== Govsets 添加调速器 / path =====

// 从 pGovPath 取可用调速器（排除当前 Govsets 已有的）
const getGovsetsAddGovActions = (path: string): { name: string }[] => {
  const existing = new Set<string>(Object.keys(
    path.split('/').reduce((o: any, k) => o?.[k], currentData.value) || {}
  ));
  const pGovKeys = Object.keys(currentData.value.pGovPath || {});
  return pGovKeys.filter(k => !existing.has(k)).map(k => ({ name: k }));
};

// 从 pGovPath/{gov} 取可用 path（排除当前 Govsets/{gov} 已有的）
const getGovsetsAddPathActions = (path: string): { name: string }[] => {
  const parts = path.split('/');
  const govKey = parts[parts.length - 1] || '';
  const existing = new Set<string>(Object.keys(
    path.split('/').reduce((o: any, k) => o?.[k], currentData.value) || {}
  ));
  const pGovPaths = Object.keys(currentData.value.pGovPath?.[govKey] || {});
  return pGovPaths.filter(p => !existing.has(p)).map(p => ({ name: p }));
};

const showGovsetsAddGovSheet = ref(false);
const govsetsAddGovActions = ref<{ name: string }[]>([]);
const currentGovsetsPath = ref('');

const openGovsetsAddGov = (path: string) => {
  currentGovsetsPath.value = path;
  const actions = getGovsetsAddGovActions(path);
  if (actions.length === 0) { showToast('pGovPath 中无可添加的调速器'); return; }
  govsetsAddGovActions.value = actions;
  showGovsetsAddGovSheet.value = true;
};

const onSelectGovsetsAddGov = (a: any) => {
  const pGovPaths = currentData.value.pGovPath?.[a.name] || {};
  const newNode: Record<string, any> = {};
  Object.keys(pGovPaths).forEach(p => {
    newNode[p] = { SmallCore: '', MediumCore: '', BigCore: '', SuperBigCore: '' };
  });
  setDeepValue(currentData.value, `${currentGovsetsPath.value}/${a.name}`, newNode);
  saveConfig();
  const newColPath = `${currentGovsetsPath.value}/${a.name}`;
  if (!activeNames.value.includes(newColPath)) activeNames.value.push(newColPath);
  showGovsetsAddGovSheet.value = false;
};

const showGovsetsAddPathSheet = ref(false);
const govsetsAddPathActions = ref<{ name: string }[]>([]);
const currentGovsetsChildPath = ref('');

const openGovsetsAddPath = (path: string) => {
  currentGovsetsChildPath.value = path;
  const actions = getGovsetsAddPathActions(path);
  if (actions.length === 0) { showToast('pGovPath 中无可添加的 path'); return; }
  govsetsAddPathActions.value = actions;
  showGovsetsAddPathSheet.value = true;
};

const onSelectGovsetsAddPath = (a: any) => {
  const newNode = { SmallCore: '', MediumCore: '', BigCore: '', SuperBigCore: '' };
  setDeepValue(currentData.value, `${currentGovsetsChildPath.value}/${a.name}`, newNode);
  saveConfig();
  showGovsetsAddPathSheet.value = false;
};

// --- 递归组件渲染逻辑 ---
const RecursiveItem = defineComponent({
  name: 'RecursiveItem',
  props: ['name', 'value', 'path'],
  setup(props) {
    return () => {
      if (isObj(props.value)) {
        const pathParts = props.path.split('/');
        const isPGovPath = props.path === 'pGovPath';
        const isPGovChild = pathParts.length === 2 && pathParts[0] === 'pGovPath';
        // Govsets: 路径末尾为 Govsets，且不是顶层（有父级 mode）
        const isGovsetsNode = pathParts[pathParts.length - 1] === 'Govsets' && pathParts.length >= 2;
        // Govsets/{gov}: 路径倒数第二为 Govsets
        const isGovsetsChild = pathParts[pathParts.length - 2] === 'Govsets' && pathParts.length >= 3;

        const children: any[] = Object.entries(props.value).map(([subKey, subVal]) =>
          h(RecursiveItem, { key: subKey, name: subKey, value: subVal, path: `${props.path}/${subKey}` })
        );

        if (isPGovPath) {
          children.push(h(Cell, {
            key: '__add_gov',
            title: t('add_governor'),
            center: true,
            isLink: true,
            class: 'add-btn-cell',
            onClick: () => openAddGov()
          }));
        }

        if (isPGovChild) {
          const govKey = pathParts[1];
          children.push(h(Cell, {
            key: '__add_path',
            title: t('add_path'),
            center: true,
            isLink: true,
            class: 'add-btn-cell',
            onClick: () => { newPathName.value = ''; addingPathUnderGov.value = govKey; showAddPathDialog.value = true; }
          }));
        }

        if (isGovsetsNode) {
          children.push(h(Cell, {
            key: '__govsets_add_gov',
            title: t('add_governor'),
            center: true,
            isLink: true,
            class: 'add-btn-cell',
            onClick: () => openGovsetsAddGov(props.path)
          }));
        }

        if (isGovsetsChild) {
          children.push(h(Cell, {
            key: '__govsets_add_path',
            title: t('add_path'),
            center: true,
            isLink: true,
            class: 'add-btn-cell',
            onClick: () => openGovsetsAddPath(props.path)
          }));
        }

        return h(CollapseItem, { title: props.name, name: props.path, class: 'nested-group' }, {
          default: () => children
        });
      }

      // 叶子节点
      const fieldKey = props.path.split('/').pop() || '';
      let valueSuffix = '';
      if (isFreqField(props.path) && typeof props.value === 'number') {
        valueSuffix = ` (${(props.value / 1000000).toFixed(2)} GHz)`;
      }

      const policy = getPolicyForKey(fieldKey);
      const hasSysfsData = policy >= 0 && (
        (isFreqField(props.path) && (freqsMap.value[policy]?.length ?? 0) > 0) ||
        (isGovField(props.path) && (govsMap.value[policy]?.length ?? 0) > 0)
      );

      return h(Cell, {
        title: props.name,
        center: true,
        isLink: typeof props.value !== 'boolean',
        onClick: () => handleItemClick(props.path, props.value)
      }, {
        value: () => {
          if (typeof props.value === 'boolean') return null;
          const displayVal = Array.isArray(props.value)
            ? `[${props.value.join(', ')}]`
            : String(props.value) + valueSuffix;
          return h('span', { style: hasSysfsData ? { color: '#1989fa', fontWeight: '500' } : {} }, displayVal);
        },
        'right-icon': () => typeof props.value === 'boolean' ? h(Switch, {
          modelValue: props.value,
          size: '20px',
          'onUpdate:modelValue': (newVal: boolean) => {
            setDeepValue(currentData.value, props.path, newVal);
            saveConfig();
          }
        }) : null
      });
    };
  }
});
</script>

<template>
  <div class="config-editor">
    <van-nav-bar title="详细配置" left-arrow @click-left="$router.back()" fixed placeholder>
      <template #right><van-icon name="replay" size="18" @click="loadData" /></template>
    </van-nav-bar>

    <div class="tab-container">
      <van-tabs v-model:active="activeTab" type="card" animated @change="loadData" color="#1989fa">
        <van-tab title="调度规则 (Rules)" name="rules" />
        <van-tab title="核心配置 (Config)" name="config" />
      </van-tabs>
    </div>

    <van-loading v-if="loading" class="loading-center" vertical>加载中...</van-loading>

    <div v-else class="config-content">
      <van-collapse v-model="activeNames" :border="false">
        <RecursiveItem
          v-for="(val, key) in currentData"
          :key="key"
          :name="String(key)"
          :value="val"
          :path="String(key)"
        />
      </van-collapse>
      <div style="height: 60px;"></div>
    </div>

    <!-- 普通文本编辑弹窗 -->
    <van-dialog v-model:show="showEditDialog" title="编辑" show-cancel-button @confirm="confirmEdit">
      <div class="dialog-content">
        <div class="path-hint">{{ editingKeyPath.replace(/\//g, ' > ') }}</div>
        <van-field v-model="editingValue" :type="editingType === 'number' ? 'number' : 'text'" input-align="center" border autofocus />
      </div>
    </van-dialog>

    <!-- 全局模式 / 应用规则 / 语言 / 日志等级 -->
    <van-action-sheet v-model:show="showModeSheet" :actions="modeActions" cancel-text="取消" @select="onSelectMode" />
    <van-action-sheet v-model:show="showAppModeSheet" :actions="appModeActions" cancel-text="取消" @select="onSelectAppMode" />
    <van-action-sheet v-model:show="showLanguageSheet" :actions="languageActions" cancel-text="取消" @select="onSelectLanguage" />
    <van-action-sheet v-model:show="showLoglevelSheet" :actions="loglevelActions" cancel-text="取消" @select="onSelectLoglevel" />

    <!-- 动态频率选择器 -->
    <van-action-sheet
      v-model:show="showFreqSheet"
      :actions="freqSheetActions"
      :title="editingKeyPath.split('/').pop() + ' 频率选择'"
      cancel-text="取消"
      @select="onSelectFreq"
    />

    <!-- 动态调速器选择器（Freq/Governor 字段用） -->
    <van-action-sheet
      v-model:show="showGovSheet"
      :actions="govSheetActions"
      :title="editingKeyPath.split('/').pop() + ' 调速器选择'"
      cancel-text="取消"
      @select="onSelectGov"
    />

    <!-- pGovPath 添加调速器：优先 sysfs 选择，失败退化为文本输入 -->
    <van-action-sheet
      v-model:show="showAddGovSheet"
      :actions="getAllGovernors()"
      title="选择调速器"
      cancel-text="取消"
      @select="onSelectAddGov"
    />
    <van-dialog v-model:show="showAddGovDialog" :title="t('add_governor')" show-cancel-button @confirm="confirmAddGov">
      <div class="dialog-content">
        <div class="path-hint">{{ t('add_governor_hint') }}</div>
        <van-field v-model="newGovName" :placeholder="t('governor_name_placeholder')" input-align="center" border autofocus />
      </div>
    </van-dialog>

    <!-- pGovPath/{gov} 添加 path -->
    <van-dialog v-model:show="showAddPathDialog" :title="t('add_path_title', { gov: addingPathUnderGov })" show-cancel-button @confirm="confirmAddPath">
      <div class="dialog-content">
        <div class="path-hint">{{ t('add_path_hint', { gov: addingPathUnderGov }) }}</div>
        <van-field v-model="newPathName" :placeholder="t('path_name_placeholder')" input-align="center" border autofocus />
      </div>
    </van-dialog>

    <!-- Govsets 添加调速器（从 pGovPath 的 key 中选） -->
    <van-action-sheet
      v-model:show="showGovsetsAddGovSheet"
      :actions="govsetsAddGovActions"
      title="选择调速器（来自 pGovPath）"
      cancel-text="取消"
      @select="onSelectGovsetsAddGov"
    />

    <!-- Govsets/{gov} 添加 path（从 pGovPath/{gov} 的 key 中选） -->
    <van-action-sheet
      v-model:show="showGovsetsAddPathSheet"
      :actions="govsetsAddPathActions"
      title="选择 path（来自 pGovPath）"
      cancel-text="取消"
      @select="onSelectGovsetsAddPath"
    />
  </div>
</template>

<style scoped>
.config-editor { min-height: 100vh; background: #f7f8fa; }
.tab-container { padding: 12px 16px; background: #fff; margin-bottom: 8px; }
.loading-center { padding-top: 100px; }
.config-content { padding: 0 12px; }
.nested-group { margin-bottom: 4px; border-radius: 8px; overflow: hidden; }
:deep(.van-collapse-item__content) { padding: 0 0 0 16px; background: #fafafa; }
:deep(.van-cell) { margin-bottom: 1px; border-radius: 4px; }
.dialog-content { padding: 20px 16px; }
.path-hint { font-size: 11px; color: #999; text-align: center; margin-bottom: 12px; }
:deep(.add-btn-cell) { color: #1989fa; font-size: 13px; background: #f0f7ff; border-radius: 4px; margin-top: 4px; }
:deep(.add-btn-cell .van-cell__title) { color: #1989fa; }
:deep(.add-btn-cell .van-icon) { color: #1989fa; }
:deep(.van-action-sheet__subname) { font-size: 11px; color: #999; }
</style>