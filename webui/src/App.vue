<script setup lang="ts">
import { provide, ref } from 'vue'

// 亮色主题 — 核心领航员 (core-pilot) WebUI
// 设计原则: 移动端优先 / 中文 / 大白话 / 卡片化 / 傻瓜化
const isDark = ref(false)
provide('isDark', isDark)
</script>

<template>
  <van-config-provider theme="light">
    <router-view v-slot="{ Component }">
      <transition name="fade" mode="out-in">
        <component :is="Component" />
      </transition>
    </router-view>
    <nav class="app-nav">
      <router-link to="/" class="nav-item">
        <van-icon name="chart-trending-o" size="22" />
        <span>首页</span>
      </router-link>
      <router-link to="/config" class="nav-item">
        <van-icon name="setting-o" size="22" />
        <span>调度</span>
      </router-link>
      <router-link to="/app-rules" class="nav-item">
        <van-icon name="apps-o" size="22" />
        <span>规则</span>
      </router-link>
      <router-link to="/log" class="nav-item">
        <van-icon name="notes-o" size="22" />
        <span>日志</span>
      </router-link>
    </nav>
  </van-config-provider>
</template>

<style>
/* ============================================================
   亮色主题令牌 — 浅底白卡, 彩色点缀
   ============================================================ */
:root {
  --bg-base: #f5f6f8;
  --bg-card: #ffffff;
  --bg-card-hover: #f0f4ff;
  --bg-elevated: #fafbfc;
  --border: #e6e8ec;
  --border-strong: #d6dae0;
  --text-primary: #1f2937;
  --text-secondary: #4b5563;
  --text-muted: #9ca3af;
  --accent: #3b82f6;       /* 主蓝 */
  --accent-hover: #2563eb;
  --accent-soft: #eaf2ff;
  --success: #10b981;      /* 绿 */
  --success-soft: #e6f7ef;
  --warning: #f59e0b;      /* 橙 */
  --warning-soft: #fef3cd;
  --danger: #ef4444;       /* 红 */
  --danger-soft: #fde8e8;
  --info: #6b7280;
  --purple: #8b5cf6;
  --purple-soft: #f3edff;
  --pink: #ec4899;
}

* { box-sizing: border-box; }

html, body {
  margin: 0;
  padding: 0;
  background-color: var(--bg-base);
  color: var(--text-primary);
  font-family: -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Helvetica, Segoe UI, Arial, Roboto, 'PingFang SC', 'miui', 'Hiragino Sans GB', 'Microsoft Yahei', sans-serif;
  -webkit-font-smoothing: antialiased;
  min-height: 100vh;
}

#app {
  min-height: 100vh;
  padding-bottom: 70px;
}

a { color: var(--accent); text-decoration: none; }
a.router-link-active { color: var(--accent); }

/* 底部导航 — 移动端优先, 大按钮 */
.app-nav {
  position: fixed;
  bottom: 0; left: 0; right: 0;
  height: 64px;
  background: var(--bg-card);
  border-top: 1px solid var(--border);
  display: flex;
  justify-content: space-around;
  align-items: center;
  z-index: 1000;
  box-shadow: 0 -2px 8px rgba(0,0,0,0.04);
}

.app-nav .nav-item {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 500;
  padding: 8px 0;
  transition: color 0.2s;
  text-decoration: none;
}

.app-nav .nav-item.router-link-active {
  color: var(--accent);
}

.app-nav .nav-item:active {
  transform: scale(0.95);
}

/* fade transition */
.fade-enter-active, .fade-leave-active { transition: opacity 0.15s; }
.fade-enter-from, .fade-leave-to { opacity: 0; }

/* ============================================================
   Vant 组件亮色覆盖 (项目内统一) — 全局 style, 直接写选择器
   ============================================================ */
.van-nav-bar {
  background: var(--bg-card) !important;
}
.van-nav-bar__title {
  color: var(--text-primary) !important;
}
.van-nav-bar .van-icon {
  color: var(--text-primary) !important;
}
.van-cell-group--inset {
  margin: 12px !important;
  background: var(--bg-card) !important;
  border-radius: 12px !important;
  overflow: hidden;
}
.van-cell {
  background: var(--bg-card) !important;
  color: var(--text-primary) !important;
}
.van-cell__title {
  color: var(--text-primary) !important;
}
.van-cell__value {
  color: var(--text-secondary) !important;
}
.van-cell-group__title {
  color: var(--text-secondary) !important;
  padding-left: 4px !important;
}
.van-dialog {
  background: var(--bg-card) !important;
}
.van-popup {
  background: var(--bg-card) !important;
}
.van-popup--top {
  background: var(--warning-soft) !important;
  color: #664d03 !important;
}
.van-action-sheet__header {
  color: var(--text-primary) !important;
}
.van-action-sheet__item {
  background: var(--bg-card) !important;
  color: var(--text-primary) !important;
}
.van-button--primary {
  background: var(--accent) !important;
  border-color: var(--accent) !important;
}
.van-button--danger {
  background: var(--danger) !important;
  border-color: var(--danger) !important;
}
.van-field {
  background: var(--bg-card) !important;
}
.van-search {
  background: var(--bg-card) !important;
}
.van-progress {
  background: rgba(0,0,0,0.05) !important;
}
.van-slider {
  background: var(--bg-card) !important;
}
.van-switch__node {
  background: var(--bg-card) !important;
}
.van-radio__label {
  color: var(--text-primary) !important;
}
.van-loading__text {
  color: var(--text-secondary) !important;
}
.van-tabbar-item--active {
  color: var(--accent) !important;
}

/* ============================================================
   配置子页公共样式 (问题 5 拆分后由 8 个子页面共享, 避免重复声明)
   ============================================================ */
.sub-page { padding-bottom: 20px; max-width: 600px; margin: 0 auto; }
.sub-body { padding: 12px 16px 0; }
.cfg-card {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 14px;
  margin-bottom: 12px;
}
.cfg-card-head { display: flex; justify-content: space-between; align-items: center; }
.cfg-card-name { font-size: 16px; font-weight: 700; }
.cfg-intro { font-size: 12.5px; color: var(--text-secondary); line-height: 1.65; margin: 10px 0 4px; }
.cfg-banner { padding: 8px 12px; border-radius: 8px; margin-bottom: 10px; font-size: 13px; }
.cfg-banner.err { background: var(--danger-soft); color: var(--danger); }
.cfg-banner.ok { background: var(--success-soft); color: var(--success); }
.save-btn {
  border: none; background: var(--accent); color: #fff;
  padding: 7px 18px; border-radius: 18px; font-size: 13px;
}
.save-btn:active { opacity: .85; }
.auto-tag, .live-tag { font-size: 11px; padding: 3px 10px; border-radius: 10px; }
.auto-tag { color: var(--text-secondary); background: rgba(0,0,0,.06); }
.live-tag { color: var(--success); background: var(--success-soft); }
.readout { display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px; margin-top: 10px; }
.readout div {
  background: var(--bg-base); border-radius: 8px; padding: 9px 10px;
  display: flex; justify-content: space-between; align-items: center;
}
.readout span { font-size: 12px; color: var(--text-muted); }
.readout b { font-size: 15px; font-variant-numeric: tabular-nums; }
.mode-chip-row { display: flex; gap: 8px; margin: 10px 0 4px; }
.mode-chip {
  flex: 1; border: 1.5px solid var(--border); background: var(--bg-base);
  border-radius: 16px; padding: 7px 0; font-size: 13px; color: var(--text-secondary);
}
.mode-chip.on { border-color: var(--accent); color: var(--accent); background: rgba(59,130,246,.10); font-weight: 600; }
.switch-row {
  display: flex; justify-content: space-between; align-items: center;
  margin-top: 14px; padding-top: 12px; border-top: 1px dashed var(--border);
}
.switch-row b { font-size: 14px; display: block; }
.switch-row small { font-size: 11.5px; color: var(--text-muted); }
.state-line {
  margin-top: 12px; padding: 9px 10px; border-radius: 8px;
  background: rgba(59,130,246,.08);
  font-size: 12px; color: var(--text-secondary); line-height: 1.6;
}
.keep-block { margin-top: 14px; padding-top: 12px; border-top: 1px dashed var(--border); }
.keep-title { font-size: 14.5px; font-weight: 700; }
.keep-hint { font-size: 11.5px; color: var(--text-muted); margin: 3px 0 8px; line-height: 1.5; }
.keep-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 7px; }
.keep-btn {
  position: relative;
  border: 1.5px solid var(--border); border-radius: 9px;
  background: var(--bg-base); color: var(--text-secondary);
  font-size: 12.5px; padding: 9px 0;
}
.keep-btn.on { border-color: var(--danger); background: var(--danger-soft); color: var(--danger); font-weight: 600; }
.keep-btn.locked { opacity: .75; cursor: not-allowed; }
.keep-btn small { display: block; font-size: 9px; color: var(--text-muted); line-height: 1; margin-top: 1px; }
.keep-btn.on small { color: var(--danger); }
.gears-input {
  width: 100%; box-sizing: border-box; margin-top: 6px; padding: 9px 10px;
  border: 1px solid var(--border-strong); border-radius: 8px;
  font-size: 14px; color: var(--text-primary);
}
</style>