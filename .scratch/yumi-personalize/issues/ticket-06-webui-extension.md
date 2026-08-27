# Ticket 06 — WebUI 全面扩展 (Phase 4)

## 状态

📝 **草稿占位** — 详见 `.scratch/yumi-personalize/spec.md` Section 3 (Phase 4) + Section 4 (完整配置项清单).

## 目标

实现所有配置项的傻瓜化 UI. 让用户能在手机上点几下就完成全套配置, 不需要 ssh 进 adb shell 改 YAML.

## 核心任务

| 任务 | 说明 |
|---|---|
| 8 卡片配置页 | CPU/GPU/IO/Swap/触摸/帧平滑/温度/热插拔, 每个卡片亮屏/息屏两个分页 |
| 实时感知面板 | 显示八路感知数据 + 核心在线状态 |
| IO 调度器动态下拉 | 读 /sys/block/*/queue/scheduler 生成选项 |
| 帮助系统 | 每个参数旁的"?"按钮, 显示中文傻瓜说明 |
| App 规则页面 | 规则管理 (与 ticket-05 联动) |
| 配置导入/导出 | YAML 文件导入导出 |

## UI 框架 (基于现有 baseline WebUI)

baseline 已确认: KSU WebUIActivity 自动接管 `webroot/`, Vite + Vue 3 栈, IPC Channel 与 daemon 通信.

需要新增的 Vue 组件:

```
webui/src/components/
├── PerCpuMonitor.vue      # 8 路感知数据卡 (实时)
├── CoreMap.vue            # 8 核网格 + 在线状态 (Ticket-04 用)
├── ConfigCard.vue         # 通用卡片 (亮屏/息屏切换 + 表单)
│   ├── CpuConfig.vue      # CPU 配置卡
│   ├── GpuConfig.vue      # GPU 配置卡
│   ├── IoConfig.vue       # IO 配置卡
│   ├── SwapConfig.vue     # Swap 配置卡
│   ├── TouchConfig.vue    # 触摸配置卡
│   ├── FrameConfig.vue    # 帧平滑配置卡
│   ├── TempConfig.vue     # 温度配置卡
│   ├── BurstConfig.vue    # 突发突破配置卡
│   └── HotplugConfig.vue  # 热插拔配置卡
├── AppRules.vue           # App 规则管理 (Ticket-05)
└── HelpTooltip.vue        # 通用"? "帮助提示

webui/src/api/
├── hotplug.ts             # 热插拔 IPC (Ticket-04)
├── app_rules.ts           # App 规则 IPC (Ticket-05)
├── config.ts              # 通用配置 IPC
└── sensor.ts              # 实时感知数据 IPC
```

## 验收标准

- 所有配置项可通过 WebUI 修改并保存
- 实时数据面板更新正常 (5 Hz 推送)
- 傻瓜说明完整 (每个参数都有"?"提示)
- 亮屏/息屏切换无缝

## 与其他 ticket 的依赖

- **前置依赖**: Ticket-02 (baseline WebUI 已跑通) ✅
- **后置依赖**: Ticket-03/04/05/07 各自完成后, 对应配置卡才能填实

## 待 grill-me 确认

1. **UI 风格** — 沿用 baseline 已有的 Vue 风格 (蓝色 header + 卡片式), 还是重新设计?
2. **配置持久化** — WebUI 改完后, 实时生效还是重启 daemon 才生效?
3. **导入/导出格式** — 单 YAML 文件 vs 拆成多个 (per 域)?
4. **实时感知粒度** — 5 Hz 推送 (200ms) 足够, 还是需要更高频 (如触摸事件实时触发)?

## 当前状态

- ❌ 未开始
- ⏭️ 依赖 Ticket-03/04/05 提供配置 schema 和 IPC 接口