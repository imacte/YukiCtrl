# Ticket 05 — App 规则引擎 (Phase 3)

## 状态

📝 **草稿占位** — 详见 `.scratch/yumi-personalize/spec.md` Section 3 (Phase 3) + Section 4 (App 规则) + Section 5 (决策流程图).

## 目标

让用户能对特定 App 施加调度偏置 (限制或加速).

## 核心思路

- 配置: `src/config.rs` 加 `AppRules` 模型 (规则列表: 包名 + 类型 + 强度)
- 引擎: `src/scheduler/app_rule.rs` 新增 — 前台 App 匹配 → 参数偏置
- UI: `webui/src/components/AppRules.vue` 新增 — 添加/删除规则, 强度滑块

## 规则结构 (来自 spec.md)

```yaml
- package: "com.example.app"
  type: "restrict"        # restrict / boost
  strength: "medium"      # light / medium / heavy
  max_freq_scale: 0.8     # 自定义缩放系数
  target_util_offset: -20 # 目标利用率偏移
  disable_burst: true     # 是否禁用 burst
  boost_threshold_offset: 15  # boost 触发阈值偏移
```

## 验收标准

- 添加限制规则后, 对应 App 前台时 max_freq 降低
- 添加加速规则后, 对应 App 更容易触发 boost

## 与其他 ticket 的依赖

- **前置依赖**: Phase 1 (per-CPU 感知) 完成 — 决策需要 CPU/GPU/触摸等数据
- **后置依赖**: Phase 4 (WebUI 全面扩展) 提供 UI 框架

## 待 grill-me 确认 (本 ticket 开工前)

1. **规则匹配粒度** — 包名精确匹配? 还是支持通配符 (e.g. `com.tencent.*`)?
2. **多规则冲突** — 一个 App 同时匹配 restrict + boost 规则时, 取最强 / 取最后 / 报错?
3. **生效时机** — 进入前台立刻生效, 还是 PID 调频周期 (5Hz) 内平滑过渡?
4. **持久化** — 规则存哪里? `/data/adb/modules/yumi/config/app_rules.yaml` 还是 SQLite?
5. **首次配置 UX** — WebUI 提供内置 App 列表 (按类别: 游戏/社交/视频) 还是要用户手动输入包名?

## 当前状态

- ❌ 未开始
- ⏭️ 依赖 Phase 1 (Ticket-03) 完成