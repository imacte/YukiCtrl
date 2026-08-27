// src/router/index.ts
import { createRouter, createWebHashHistory } from 'vue-router'
import HomeView from '../views/HomeView.vue'
import AppRuleManagement from '../views/AppRuleManagement.vue'
import LogViewerView from '../views/LogViewerView.vue'
import ConfigListView from '../views/ConfigListView.vue'

// 问题 5: 调度页拆分为 模块列表 (/config) + 8 个模块子页面
import ConfigHotplug from '../views/config/ConfigHotplug.vue'
import ConfigCpu from '../views/config/ConfigCpu.vue'
import ConfigGpu from '../views/config/ConfigGpu.vue'
import ConfigTouch from '../views/config/ConfigTouch.vue'
import ConfigFrame from '../views/config/ConfigFrame.vue'
import ConfigIo from '../views/config/ConfigIo.vue'
import ConfigSwap from '../views/config/ConfigSwap.vue'
import ConfigTemp from '../views/config/ConfigTemp.vue'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'home', component: HomeView },
    // 旧的 AppRulesView 已被 AppRuleManagement 替代; 保留 /apps 旧地址不失效
    { path: '/apps', name: 'apps', component: AppRuleManagement },
    { path: '/app-rules', name: 'app-rules', component: AppRuleManagement },

    // ── 调度: 模块列表 + 8 个子页面 ──
    { path: '/config', name: 'config', component: ConfigListView },
    { path: '/config/hotplug', name: 'config-hotplug', component: ConfigHotplug },
    { path: '/config/cpu', name: 'config-cpu', component: ConfigCpu },
    { path: '/config/gpu', name: 'config-gpu', component: ConfigGpu },
    { path: '/config/touch', name: 'config-touch', component: ConfigTouch },
    { path: '/config/frame', name: 'config-frame', component: ConfigFrame },
    { path: '/config/io', name: 'config-io', component: ConfigIo },
    { path: '/config/swap', name: 'config-swap', component: ConfigSwap },
    { path: '/config/temp', name: 'config-temp', component: ConfigTemp },

    // 历史路由迁移: 旧调度单页 / 旧热插拔页 → 新列表
    { path: '/schedule', redirect: '/config' },
    { path: '/hotplug', redirect: '/config/hotplug' },
    // 感知面板已合并进首页; 旧地址一律回首页
    { path: '/sense', redirect: '/' },
    { path: '/log', name: 'log', component: LogViewerView }
  ]
})

export default router
