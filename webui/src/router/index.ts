// src/router/index.ts
import { createRouter, createWebHashHistory } from 'vue-router'
import HomeView from '../views/HomeView.vue'
import AppRuleManagement from '../views/AppRuleManagement.vue'
import ScheduleSettings from '../views/ScheduleSettings.vue'
import SensePanel from '../views/SensePanel.vue'
import LogViewerView from '../views/LogViewerView.vue'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'home', component: HomeView },
    // 任务 #5: 旧的 AppRulesView 已被 AppRuleManagement 替代
    // 保留 /apps 路由指向新页面 (避免旧书签失效)
    { path: '/apps', name: 'apps', component: AppRuleManagement },
    { path: '/app-rules', name: 'app-rules', component: AppRuleManagement },
    // 任务 B 彻底重做: 调度页 = ScheduleSettings (8 张主题色卡片)
    { path: '/config', name: 'config', component: ScheduleSettings },
    // 旧的 HotplugSettings / ConfigEditorView 已并入调度页
    { path: '/hotplug', redirect: '/config' },
    { path: '/sense', name: 'sense', component: SensePanel },
    { path: '/log', name: 'log', component: LogViewerView }
  ]
})

export default router
