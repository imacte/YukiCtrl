<script setup lang="ts">
import { ref, onMounted, nextTick, computed } from 'vue'
import { Bridge } from '@/utils/bridge'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const logContent = ref('')
const loading = ref(false)
const terminalBody = ref<HTMLElement | null>(null)

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
