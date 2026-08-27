<script setup lang="ts">
import { ref, onMounted, nextTick, computed } from 'vue'
import yaml from 'js-yaml'
import { Bridge } from '@/utils/bridge'
import { useI18n } from 'vue-i18n'
import { saveHotplugConfig } from '@/api/hotplug'
import { CLG_MODE_DEFAULTS, FREQ_LIMIT_DEFAULTS, HOTPLUG_DEFAULTS, IO_DEFAULTS, FAS_DEFAULTS, MODULE_SCOPED_DEFAULTS } from '@/config/moduleSpecs'
import ResetDefaultsBtn from '@/components/ResetDefaultsBtn.vue'

const { t } = useI18n()
const logContent = ref('')
const loading = ref(false)
const terminalBody = ref<HTMLElement | null>(null)
const resetMsg = ref('')

const fetchLog = async () => {
  loading.value = true
  try {
    const text = await Bridge.getDaemonLog()
    logContent.value = text || ''
    await nextTick()
    if (terminalBody.value) {
      terminalBody.value.scrollTop = terminalBody.value.scrollHeight
    }
  } finally {
    loading.value = false
  }
}

/**
 * 需求: 恢复全部默认值 — 所有模块、所有档位、亮屏/息屏的全部参数。
 * 覆盖范围:
 *   - config.yaml: 四档位五参数 + 频率护栏 + 读写三参数 + 读写总开关
 *   - hotplug/config.yaml: 核心开关全量默认
 *   - rules.yaml fas_rules: 帧率容差/PID 三系数/目标帧率档位
 * 保留: 应用专属规则 (app_modes/per_app_profiles), 当前全局档位, meta, 日志级别。
 * 写盘即热生效 (daemon inotify / 200ms tick)。
 */
const resetAllDefaults = async () => {
  try {
    // 1. config.yaml — 四档位 + 频率护栏 + 读写 + 全模块亮/息屏双套 (modules.*)
    const mainCfg: any = await Bridge.getMainConfig()
    for (const [mode, defs] of Object.entries(CLG_MODE_DEFAULTS)) {
      if (!mainCfg[mode]) mainCfg[mode] = {}
      if (!mainCfg[mode].cpu_load_governor) mainCfg[mode].cpu_load_governor = {}
      for (const [k, v] of Object.entries(defs)) mainCfg[mode].cpu_load_governor[k] = v
    }
    mainCfg.freq_limits = { ...FREQ_LIMIT_DEFAULTS }
    if (!mainCfg.IO_Settings) mainCfg.IO_Settings = {}
    for (const [k, v] of Object.entries(IO_DEFAULTS)) mainCfg.IO_Settings[k] = v
    if (!mainCfg.function) mainCfg.function = {}
    mainCfg.function.IOOptimization = true
    if (!mainCfg.modules) mainCfg.modules = {}
    for (const [mk, scopes] of Object.entries(MODULE_SCOPED_DEFAULTS)) {
      if (mk === 'temp') continue // temp 双套在 hotplug/config.yaml, 见下
      if (!mainCfg.modules[mk]) mainCfg.modules[mk] = {}
      for (const [sc, kv] of Object.entries(scopes)) {
        if (!mainCfg.modules[mk][sc]) mainCfg.modules[mk][sc] = {}
        for (const [k, v] of Object.entries(kv)) mainCfg.modules[mk][sc][k] = v
      }
    }
    await Bridge.saveMainConfig(yaml.load(yaml.dump(mainCfg)))

    // 2. hotplug/config.yaml — 核心开关全量默认 (含温度双套)
    const hpDefault = JSON.parse(JSON.stringify(HOTPLUG_DEFAULTS))
    await saveHotplugConfig(hpDefault)

    // 3. rules.yaml fas_rules — 帧平滑参数 (保留 per_app_profiles 与 app_modes)
    const rulesCfg: any = await Bridge.getRulesConfig()
    if (!rulesCfg.fas_rules) rulesCfg.fas_rules = {}
    for (const [k, v] of Object.entries(FAS_DEFAULTS)) {
      const segs = k.split('.')
      let cur = rulesCfg.fas_rules
      for (let i = 0; i < segs.length - 1; i++) {
        if (!cur[segs[i]]) cur[segs[i]] = {}
        cur = cur[segs[i]]
      }
      cur[segs[segs.length - 1]] = v
    }
    await Bridge.saveRulesConfig(rulesCfg)

    resetMsg.value = '已恢复全部默认值, 各模块立即生效 (当前档位与应用专属规则保留)'
  } catch (e) {
    resetMsg.value = `恢复失败: ${String(e)}`
  }
  setTimeout(() => { resetMsg.value = '' }, 4000)
}

const formattedLog = computed(() => {
  if (!logContent.value) return `<div class="log-empty">${t('log_empty')}</div>`
  return logContent.value.split('\n').map(line => {
    if (!line.trim()) return ''
    let html = line.replace(/\[\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\]/g, m => `<span class="log-time">${m}</span>`)
    html = html.replace(/\[INFO\]/g, `<span class="log-info">[INFO]</span>`)
    html = html.replace(/\[WARN\]/g, `<span class="log-warn">[WARN]</span>`)
    html = html.replace(/\[ERROR\]/g, `<span class="log-error">[ERROR]</span>`)
    html = html.replace(/\[(core-pilot[^\]]*|Scheduler|AppDetect|Screen|Boot)\]/g, m => `<span class="log-tag">${m}</span>`)
    return `<div class="log-line">${html}</div>`
  }).join('')
})

onMounted(() => fetchLog())
</script>

<template>
  <div class="log-viewer">
    <div class="page-header">
      <span class="page-title">{{ t('view_log') }}</span>
      <van-icon name="replay" size="20" color="var(--accent)" @click="fetchLog" />
    </div>

    <div class="global-reset-card">
      <div class="gr-title">危险操作</div>
      <div class="gr-desc">把所有模块、所有档位、亮屏/息屏的全部参数恢复为出厂默认值, 立即生效并保存。应用专属规则与当前档位保留。</div>
      <div v-if="resetMsg" class="gr-msg" :class="{ err: resetMsg.startsWith('恢复失败') }">{{ resetMsg }}</div>
      <ResetDefaultsBtn label="恢复全部默认值" danger @reset="resetAllDefaults" />
    </div>

    <van-loading v-if="loading && !logContent" class="loading-center" vertical>{{ t('loading') }}</van-loading>

    <div v-else class="terminal-card">
      <div class="terminal-header">
        <div class="mac-buttons">
          <span class="btn close"></span>
          <span class="btn minimize"></span>
          <span class="btn maximize"></span>
        </div>
        <div class="terminal-title">{{ t('log_terminal_title', { file: 'daemon.log', shell: 'bash' }) }}</div>
      </div>
      <div class="terminal-body" ref="terminalBody">
        <div class="log-container" v-html="formattedLog"></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.global-reset-card {
  margin: 0 16px 16px; padding: 14px 16px;
  border: 1px solid #b91c1c; border-radius: 12px;
  background: var(--bg-card);
}
.gr-title { font-size: 15px; font-weight: 700; color: #b91c1c; }
.gr-desc { margin: 6px 0 12px; font-size: 13px; line-height: 1.6; color: var(--text-secondary); }
.gr-msg { margin-bottom: 10px; font-size: 13px; color: #15803d; }
.gr-msg.err { color: #b91c1c; }
</style>

<style scoped>
.log-viewer { min-height: 100vh; background: var(--bg-base); }
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px;
}
.page-title { font-size: 20px; font-weight: 600; }
.loading-center { padding-top: 100px; }

.terminal-card {
  margin: 0 16px 16px;
  background: #0a0a0a;
  border-radius: 12px;
  border: 1px solid var(--border);
  overflow: hidden;
  height: calc(100vh - 130px);
  display: flex;
  flex-direction: column;
}
.terminal-header {
  background: #1f1f1f;
  height: 36px;
  display: flex;
  align-items: center;
  padding: 0 16px;
  position: relative;
  border-bottom: 1px solid #2a2a2a;
}
.mac-buttons { display: flex; gap: 8px; }
.mac-buttons .btn {
  width: 12px; height: 12px; border-radius: 50%; display: inline-block;
}
.mac-buttons .close { background: #ff5f56; }
.mac-buttons .minimize { background: #ffbd2e; }
.mac-buttons .maximize { background: #27c93f; }
.terminal-title {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  color: #888;
  font-size: 13px;
  font-weight: 500;
}
.terminal-body {
  flex: 1;
  padding: 12px 16px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: #444 #0a0a0a;
}
.terminal-body::-webkit-scrollbar { width: 6px; }
.terminal-body::-webkit-scrollbar-track { background: #0a0a0a; }
.terminal-body::-webkit-scrollbar-thumb { background: #444; border-radius: 10px; }
.log-container {
  font-family: 'JetBrains Mono', 'Fira Code', 'Courier New', Consolas, monospace;
  font-size: 12px;
  line-height: 1.6;
  color: #d4d4d4;
  word-wrap: break-word;
  white-space: pre-wrap;
}
:deep(.log-empty) { color: #888; font-style: italic; }
:deep(.log-line) { margin-bottom: 2px; }
:deep(.log-time) { color: #6a9955; }
:deep(.log-info) { color: #569cd6; font-weight: bold; }
:deep(.log-warn) { color: #dcdcaa; font-weight: bold; }
:deep(.log-error) { color: #f44747; font-weight: bold; }
:deep(.log-tag) { color: #c586c0; }
</style>
