// src/router/index.ts
import { createRouter, createWebHashHistory } from 'vue-router'
import HomeView from '../views/HomeView.vue'
import AppRuleManagement from '../views/AppRuleManagement.vue'
import HotplugSettings from '../views/HotplugSettings.vue'
import SensePanel from '../views/SensePanel.vue'
import ConfigEditorView from '../views/ConfigEditorView.vue'
import LogViewerView from '../views/LogViewerView.vue'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'home', component: HomeView },
    // 任务 #5: 老的 AppRulesView 已被 AppRuleManagement 替代
    // 保留 /apps 路由指向新页面 (避免旧书签失效)
    { path: '/apps', name: 'apps', component: AppRuleManagement },
    { path: '/app-rules', name: 'app-rules', component: AppRuleManagement },
    { path: '/hotplug', name: 'hotplug', component: HotplugSettings },
    { path: '/sense', name: 'sense', component: SensePanel },
    { path: '/config', name: 'config', component: ConfigEditorView },
    { path: '/log', name: 'log', component: LogViewerView }
  ]
})

export default router