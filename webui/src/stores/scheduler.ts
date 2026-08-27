// src/stores/scheduler.ts
//
// 全局调度状态 (问题 3 修复核心):
//   currentMode 是唯一真源 — 首页切换 / 调度页档位 chip / 帧平滑页说明全部读这里.
//   任何页面 switchMode() 后, 其他页面立即可见 (Pinia 响应式).
// 保存状态 lastSaveOk / lastSavedAt 也放这里, 列表页可显示"刚刚保存过".
import { defineStore } from 'pinia';
import { Bridge } from '@/utils/bridge';

export const useSchedulerStore = defineStore('scheduler', {
  state: () => ({
    currentMode: 'balance',
    /** initData 是否已完成 (防止子页面在模式读取完成前渲染出错误的默认档位) */
    modeLoaded: false,
    appRules: {} as Record<string, string>,
    isDaemonRunning: false,
    loading: false,
    /** 最近一次配置保存结果: null=从未保存, true=成功, false=失败 */
    lastSaveOk: null as boolean | null,
    lastSavedAt: 0
  }),
  actions: {
    async initData() {
      this.loading = true;
      try {
        const [mode, rules, running] = await Promise.all([
          Bridge.getCurrentMode(),
          Bridge.getAppRules(),
          Bridge.isDaemonRunning()
        ]);
        this.currentMode = mode;
        this.appRules = rules;
        this.isDaemonRunning = running;
        this.modeLoaded = true;
      } finally {
        this.loading = false;
      }
    },
    /** 轻量刷新: 只同步当前模式 (子页面 onMounted / 切回前台时调用) */
    async refreshMode() {
      try {
        this.currentMode = await Bridge.getCurrentMode();
        this.modeLoaded = true;
      } catch { /* 保持旧值 */ }
    },
    async switchMode(mode: string) {
      await Bridge.setMode(mode);
      this.currentMode = mode; // 全局立即生效, 所有页面同步
      this.modeLoaded = true;
    },
    /** 各配置子页面保存成功/失败后上报, 驱动全局"已保存"提示 */
    reportSave(ok: boolean) {
      this.lastSaveOk = ok;
      this.lastSavedAt = Date.now();
    }
  }
});