// src/config/moduleSpecs.ts
//
// 全部调度配置的"唯一声明中心":
//   - 8 个模块入口卡 (名称 / 主题色 / 图标 / 路由)
//   - 每个参数的类型 / 范围 / 兜底值
//   - 每个参数的"大白话五维说明": 是什么 / 调高会怎样 / 调低会怎样 / 什么情况建议调 / 建议值
//
// 后端字段名对齐 yumi-fork/src/scheduler/config.rs:
//   - IO_Settings.Scheduler  (serde rename, 大写 S — 前端旧版写小写导致保存无效, 已修正)
//   - read_ahead_kb / nomerges / iostats 在 Rust 侧是 String, 保存必须保持字符串
//     (写入数字会让 serde_yaml 解析整份 config 失败, daemon 直接拒绝加载)

export interface ParamSpec {
  /** 深路径: mainCfg 从根算起 (如 IO_Settings.Scheduler); CLG/FAS 组从组根算起 (如 up_threshold) */
  path: string
  label: string
  type: 'range' | 'select' | 'num'
  min?: number
  max?: number
  step?: number
  /** 显示换算: 显示值 = 实际值 * scale (0..1 → 百分比用 100) */
  scale?: number
  unit?: string
  /** 磁盘上缺省该键时的展示兜底值 */
  fb?: string
  options?: { v: string | number; n: string }[]
  /** 后端 serde 是 String: 保存时保持字符串类型 */
  asString?: boolean
  /** 五维说明: [是什么, 调高会怎样, 调低会怎样, 什么情况建议调, 建议值] */
  desc: [string, string, string, string, string]
}

export const DESC_KEYS = ['是什么', '调高会怎样', '调低会怎样', '什么情况建议调', '建议值'] as const

/** 数值显示 (带 scale / unit / 小数位自适应) */
export function fmtVal(p: ParamSpec, v: unknown): string {
  const s = p.scale ?? 1
  const n = Number(v)
  if (!Number.isFinite(n)) return '--'
  // 小数位由"显示粒度"决定: step 换算到显示坐标系 (step * scale).
  // 修复 bug: 旧逻辑 (s !== 1 && step < 1) 对无 scale 的参数 (如平滑系数
  // 0.05~1) 取 0 位小数 → 0.05..0.49 全显示 "0", 滑块拖动数值看似不动.
  const dispStep = (p.step ?? 1) * s
  const decimals = dispStep >= 1 ? 0 : (dispStep >= 0.1 ? 1 : 2)
  return `${(n * s).toFixed(decimals)}${p.unit ?? ''}`
}

export function fmtNum(v: unknown): string {
  return Number.isFinite(Number(v)) ? String(v) : '--'
}

/* ==================== 8 个模块入口 (调度页列表) ==================== */

export type ModuleKey = 'hotplug' | 'cpu' | 'gpu' | 'touch' | 'frame' | 'io' | 'swap' | 'temp'

export interface ModuleMeta {
  key: ModuleKey
  name: string
  color: string
  icon: string
  route: string
  /** 一句话介绍 (入口卡副标题) */
  brief: string
  /** 入口卡右侧标签 (文案统一: 可配置模块=改动自动生效; 纯自动模块=自动管理) */
  tag: '改动自动生效' | '自动管理'
}

export const MODULES: ModuleMeta[] = [
  { key: 'hotplug', name: '核心开关', color: '#ef4444', icon: 'cluster-o',          route: '/config/hotplug', brief: '8 个处理器核心的在线休眠与保留策略', tag: '改动自动生效' },
  { key: 'cpu',     name: '处理器',   color: '#3b82f6', icon: 'setting-o',          route: '/config/cpu',     brief: '升降频灵敏度, 按省电/均衡/性能/极速分别记忆', tag: '改动自动生效' },
  { key: 'gpu',     name: '显卡',     color: '#8b5cf6', icon: 'chart-trending-o',   route: '/config/gpu',     brief: '显卡频率护栏与负载加速, 亮屏/息屏独立', tag: '改动自动生效' },
  { key: 'touch',   name: '触摸加速', color: '#06b6d4', icon: 'hot-o',              route: '/config/touch',   brief: '触摸唤醒核心的开关/范围/时长, 亮屏/息屏独立', tag: '改动自动生效' },
  { key: 'frame',   name: '帧平滑',   color: '#ec4899', icon: 'play-circle-o',      route: '/config/frame',   brief: '游戏帧率自动稳帧, 掉帧判定与提频双套', tag: '改动自动生效' },
  { key: 'io',      name: '读写',     color: '#f59e0b', icon: 'records',            route: '/config/io',      brief: '存储调度算法与预读缓存, 亮屏/息屏独立', tag: '改动自动生效' },
  { key: 'swap',    name: '内存',     color: '#10b981', icon: 'diamond-o',          route: '/config/swap',    brief: '交换倾向与内存压力线, 亮屏/息屏独立', tag: '改动自动生效' },
  { key: 'temp',    name: '温度保护', color: '#991b1b', icon: 'warning-o',          route: '/config/temp',    brief: '软/硬双温度线, 亮屏/息屏独立', tag: '改动自动生效' },
]

/** 亮/息屏双套切换的组名 (各模块页共用) */
export const SCREEN_SCOPES: Record<string, string> = {
  screen_on: '亮屏时',
  screen_off: '息屏时',
}

/* ==================== 调度模式 (全局唯一真源: stores/scheduler) ==================== */

export const MODE_NAMES: Record<string, string> = {
  powersave: '省电',
  balance: '均衡',
  performance: '性能',
  fast: '极速',
}

/** 首页模式切换的五维说明 (每个模式一条) */
export const MODE_HELP: Record<string, [string, string, string, string, string]> = {
  powersave: ['限制处理器频率, 把续航放在第一位。',
              '(选更省电的方向) 发热小、掉电慢, 但打开应用、滑动响应会慢半拍。',
              '(选更流畅的方向) 反应快, 但发热和耗电明显增加。',
              '外出没电、夜间挂机、只聊天看视频时选它。',
              '出门在外电量低于 30% 时使用。'],
  balance: ['日常推荐档: 流畅和续航折中, 大多数场景的最佳选择。',
            '(选更流畅的方向) 操作更爽, 续航略降。',
            '(选更省电的方向) 更凉快, 重负载偶有卡顿。',
            '不知道选什么时的默认答案。',
            '日常使用保持均衡即可。'],
  performance: ['游戏、重负载优先流畅, 允许更高发热。',
                '(选更激进的方向) 帧率更稳, 机身更烫、耗电更快。',
                '(选保守方向) 温度低一些, 重载游戏可能掉帧。',
                '玩大型游戏、直播、长时间录制视频时选它。',
                '游戏时段使用, 日常不建议常开。'],
  fast: ['火力全开: 频率基本拉满, 不计功耗。',
         '(选更激进方向) 已是上限, 再高只能靠外设散热。',
         '(往回收) 降回「性能」档即可获得明显降温。',
         '极少数极限场景: 跑分、压测、短暂爆发任务。',
         '仅在插电散热或短时间冲刺时使用。'],
}

/* ==================== 处理器 (蓝色) — 负载调速器, 按 {mode}.cpu_load_governor 存 ==================== */

export const CLG_PARAMS: ParamSpec[] = [
  {
    path: 'target_load', label: '目标负载', type: 'range', min: 5, max: 95, step: 1, unit: '%', fb: '60',
    desc: ['帧平滑引擎的综合压力目标: CPU/显卡/内存压力高于它就提频, 低于它就省电。每个档位独立记忆。', 
           '更激进: 轻度压力也开始拉频率, 帧率更稳但耗电增加。', 
           '更保守: 允许更大的压力波动, 更省电但重负载可能掉帧。', 
           '游戏帧率不稳 → 上调; 发热明显 → 下调。', 
           '省电 40 / 均衡 60 / 性能 75 / 极速 85。'],
  },
  {
    path: 'up_threshold', label: '升频阈值', type: 'range', min: 0.5, max: 0.95, step: 0.01, scale: 100, fb: '0.8',
    desc: ['处理器利用率超过这个百分比, 才允许提高频率。是整套调速器最核心的灵敏度开关。',
           '更灵敏: 负载一涨就升频, 跟手、不卡, 但耗电增加、发热变大。',
           '更迟钝: 平时省电低温, 但打开应用瞬间可能先顿一下再提速。',
           '感觉操作跟手慢、点开应用转圈时建议往下调; 发热大续航差时往上调。',
           '均衡模式 75~85; 性能/极速 70~80; 省电 85。'],
  },
  {
    path: 'down_threshold', label: '降频阈值', type: 'range', min: 0.3, max: 0.8, step: 0.01, scale: 100, fb: '0.5',
    desc: ['利用率低于这个百分比就开始降频省电。必须比升频阈值低, 中间形成"迟滞带"防止频率来回跳。',
           '降得更晚: 频率保持高位更久, 后台加载快但费电。',
           '降得更早: 更省电, 但窗口切换可能轻微变慢。',
           '与升频阈值差距太小时 (频繁跳频发热), 可把本值调低拉大迟滞带。',
           '比升频阈值低 20~25 个百分点, 例如升频 80 → 降频 55。'],
  },
  {
    path: 'smoothing_up', label: '上行平滑系数', type: 'range', min: 0.05, max: 1, step: 0.05, fb: '0.6',
    desc: ['每次提频的"油门行程"。数值越大单次提得越猛。',
           '响应迅猛, 游戏开镜、秒开应用更快; 代价是功耗毛刺、偶尔过冲。',
           '提温缓和更省电, 但重负载场景会感觉"加速肉"。',
           '玩大型游戏觉得提帧慢 → 调大; 待机发烫 → 调小。',
           '均衡 0.6; 性能/极速 0.7~0.8; 省电 0.4。'],
  },
  {
    path: 'smoothing_down', label: '下行平滑系数', type: 'range', min: 0.05, max: 1, step: 0.05, fb: '0.3',
    desc: ['每次降频的"刹车行程"。越大降得越干脆。',
           '负载一落立刻省电, 温度掉得快。',
           '保留余量应对突发点击, 避免刚降频又卡。',
           '刷视频发热明显 → 调大; 快速滑动偶发卡顿 → 调小。',
           '均衡 0.3; 重度游戏可降到 0.2。'],
  },
]

/* ==================== 频率护栏 (处理器页, 亮屏/息屏两套) ==================== */

export const FREQ_LIMIT_PARAMS: ParamSpec[] = [
  {
    path: 'screen_on_min_pct', label: '亮屏最低频率', type: 'range', min: 0, max: 100, step: 5, unit: '%', fb: '0',
    desc: ['亮屏时处理器频率的下限 (相对最高频的百分比), 再闲也不低于这条线。', 
           '低负载也不掉底频, 滑动点击零迟滞, 但待机功耗上升。', 
           '空闲时充分降频省电, 突发操作开头可能慢半拍。', 
           '亮屏打字卡顿 → 抬到 15~20; 追求省电 → 保持 0。', 
           '0 (不限制)。'],
  },
  {
    path: 'screen_on_max_pct', label: '亮屏最高频率', type: 'range', min: 20, max: 100, step: 5, unit: '%', fb: '100',
    desc: ['亮屏时处理器频率的上限 (相对最高频的百分比), 再忙也不越过这条线。', 
           '放开上限性能全释放, 发热耗电也随之增加。', 
           '封顶限频明显省电降温, 重负载性能受影响。', 
           '夏天日常 80% 就够; 跑分游戏保持 100。', 
           '100 (不限制)。'],
  },
  {
    path: 'screen_off_min_pct', label: '息屏最低频率', type: 'range', min: 0, max: 100, step: 5, unit: '%', fb: '0',
    desc: ['黑屏待机时处理器频率的下限。', 
           '后台任务 (音乐/下载) 更流畅, 但待机耗电增加。', 
           '待机最省电; 极低频可能影响后台任务及时性。', 
           '有黑屏听歌/导航需求 → 10~15。', 
           '0 (不限制)。'],
  },
  {
    path: 'screen_off_max_pct', label: '息屏最高频率', type: 'range', min: 10, max: 100, step: 5, unit: '%', fb: '100',
    desc: ['黑屏待机时处理器频率的上限 — 息屏限频是最有效的省电手段之一。', 
           '放宽到 100 = 黑屏也不限频, 后台全速但费电。', 
           '压到 40~60% 黑屏耗电显著下降, 唤醒瞬间由亮屏护栏接管不受影响。', 
           '夜间掉电快 → 压到 50; 有后台重任务 → 80。', 
           '100 (不限制); 省电取向建议 60。'],
  },
]

/* ==================== 默认值中心 (恢复按钮数据源, 与包内 config.yaml 逐字对齐) ==================== */

/** 四档位调度默认值: 升频/降频/上行平滑/下行平滑/目标负载 */
export const CLG_MODE_DEFAULTS: Record<string, Record<string, number>> = {
  powersave:    { up_threshold: 0.85, down_threshold: 0.60, smoothing_up: 0.40, smoothing_down: 0.50, target_load: 40 },
  balance:      { up_threshold: 0.80, down_threshold: 0.50, smoothing_up: 0.60, smoothing_down: 0.30, target_load: 60 },
  performance:  { up_threshold: 0.65, down_threshold: 0.40, smoothing_up: 0.80, smoothing_down: 0.20, target_load: 75 },
  fast:         { up_threshold: 0.01, down_threshold: 0.01, smoothing_up: 1.0,  smoothing_down: 0.01, target_load: 85 },
}

/** 频率护栏默认 (0/100 = 不限制) */
export const FREQ_LIMIT_DEFAULTS: Record<string, number> = {
  screen_on_min_pct: 0, screen_on_max_pct: 100, screen_off_min_pct: 0, screen_off_max_pct: 100,
}

/** 核心开关 (hotplug/config.yaml) 全量默认 */
export const HOTPLUG_DEFAULTS = {
  lockscreen_onoff: true,
  screens_onoff: true,
  off_threshold_idle_pct: 95,
  on_threshold_util_pct: 30,
  min_online_cores: 4,
  thermal_force_all_on_c: 70,
  screen_on_keep_cores: [0, 1, 2, 3, 4, 5] as number[],
  screen_off_keep_cores: [0, 1] as number[],
  temp_on_soft_c: 0,
  temp_on_hard_c: 70,
  temp_off_soft_c: 0,
  temp_off_hard_c: 70,
}

/** 读写 (IO_Settings 内层, 注意值必须是字符串) */
export const IO_DEFAULTS: Record<string, string> = {
  Scheduler: '', read_ahead_kb: '128', nomerges: '2', iostats: '0',
}

export const FRAME_MODULE_PARAMS: ParamSpec[] = [
  {
    path: 'jank_margin_ms', label: '掉帧判定阈值', type: 'range', min: 1, max: 20, step: 1, unit: ' ms', fb: '4',
    desc: ['一帧的耗时超出预算多少毫秒判定为掉帧 (越小判定越严)。', 
           '更小 → 轻微超时也算掉帧, 提频更积极更耗电。', 
           '更大 → 只盯严重卡顿, 更省电但小抖动不处理。', 
           '电竞手感 → 2~3; 日常 → 4~6。', 
           '4 毫秒。'],
  },
  {
    path: 'boost_enabled', label: '掉帧提频开关', type: 'select', fb: 'true',
    options: [{ v: 'true', n: '开启 (掉帧即提频)' }, { v: 'false', n: '关闭' }],
    desc: ['判定掉帧后立即提升处理器性能档位的总开关。', 
           '(开启) 掉帧瞬间频率顶上去, 帧率更快恢复。', 
           '(关闭) 只记录不干预, 完全交给基础调速。', 
           '息屏时帧引擎本来挂起, 息屏套一般关闭。', 
           '亮屏开启, 息屏关闭。'],
  },
  {
    path: 'boost_strength', label: '提频强度', type: 'range', min: 0, max: 2, step: 0.1, fb: '1',
    desc: ['掉帧提频的力度倍数 (1 = 标准幅度)。', 
           '更猛 → 帧率恢复更快, 功耗尖峰更大。', 
           '更缓 → 平滑过渡省电, 恢复稍慢。', 
           '团战掉帧回不来 → 1.3~1.5。', 
           '1.0。'],
  },
]

/** 温度双套 (hotplug/config.yaml 的 temp_{on,off}_{soft,hard}_c) */
export const TEMP_DUAL_PARAMS: ParamSpec[] = [
  {
    path: 'soft_c', label: '软阈值 (预警)', type: 'range', min: 0, max: 95, step: 1, unit: '°C', fb: '0',
    desc: ['温度达到这条线时记预警日志, 提醒注意但不改变行为 (0 = 关闭预警)。', 
           '更低 → 更早收到温度提醒。', 
           '更高 / 0 → 只在真正过热时才有动静。', 
           '夏天户外使用 → 55 提前观察。', 
           '0 (关闭)。'],
  },
  {
    path: 'hard_c', label: '硬阈值 (强制全核)', type: 'range', min: 45, max: 100, step: 1, unit: '°C', fb: '70',
    desc: ['温度达到这条线时所有休眠核心立即强制拉回在线 — 过热比费电更危险。', 
           '(调低) 保护更早介入, 高温天性能更稳。', 
           '(调高) 尽量晚干预, 机身更烫才触发。', 
           '夏天烫手掉帧 → 下调到 65。', 
           '70°C 行业安全线。'],
  },
]

/** IO 息屏套 (modules.io.screen_off) */
export const IO_OFF_PARAMS: ParamSpec[] = [
  {
    path: 'scheduler', label: '存储调度算法', type: 'select', fb: '', asString: true,
    options: [
      { v: '', n: '保持亮屏设置' }, { v: 'mq-deadline', n: 'mq-deadline (低延迟)' },
      { v: 'bfq', n: 'bfq (公平流畅)' }, { v: 'kyber', n: 'kyber (均衡)' },
      { v: 'none', n: 'none (多核直通)' },
    ],
    desc: ['黑屏待机时切换的存储调度算法 (留空 = 不改, 沿用亮屏值)。', 
           '待机时换低延迟算法 → 后台小任务更快醒。', 
           '保持一致 → 避免反复切换开销。', 
           '一般留空即可。', 
           '保持亮屏设置。'],
  },
  {
    path: 'read_ahead_kb', label: '息屏预读缓存', type: 'range', min: 32, max: 2048, step: 32, unit: ' KB', fb: '128', asString: true,
    desc: ['黑屏待机时的预读大小 (音乐/下载后台场景可适当加大)。', 
           '后台顺序读更快。', 
           '省内存省电。', 
           '夜间挂下载 → 512; 纯待机 → 64~128。', 
           '128 KB。'],
  },
]

/** 帧平滑 (rules.yaml fas_rules) 默认 */
export const FAS_DEFAULTS: Record<string, unknown> = {
  fps_margin: 3,
  fps_gears: [30, 60, 90, 120],
  'pid.kp': 0.5, 'pid.ki': 0.05, 'pid.kd': 0.1,
}

/** 各模块双套默认值 (恢复按钮数据源; gpu/touch/swap/frame = config.modules.*;
 *  io 仅息屏套; temp = hotplug/config.yaml temp_* 键) */
export const MODULE_SCOPED_DEFAULTS: Record<string, Record<string, Record<string, unknown>>> = {
  gpu: {
    screen_on:  { min_pct: 0, max_pct: 100, boost_util_pct: 0 },
    screen_off: { min_pct: 0, max_pct: 100, boost_util_pct: 0 },
  },
  touch: {
    screen_on:  { enabled: true, extra_cores: 8, duration_ms: 200 },
    screen_off: { enabled: false, extra_cores: 0, duration_ms: 200 },
  },
  swap: {
    screen_on:  { swappiness: 100, pressure_pct: 20 },
    screen_off: { swappiness: 100, pressure_pct: 20 },
  },
  frame: {
    screen_on:  { jank_margin_ms: 4, boost_enabled: true, boost_strength: 1.0 },
    screen_off: { jank_margin_ms: 4, boost_enabled: false, boost_strength: 1.0 },
  },
  io: {
    screen_off: { scheduler: '', read_ahead_kb: '128' },
  },
  temp: {
    screen_on:  { soft_c: 0, hard_c: 70 },
    screen_off: { soft_c: 0, hard_c: 70 },
  },
}

export const GPU_PARAMS: ParamSpec[] = [
  {
    path: 'min_pct', label: '最低频率', type: 'range', min: 0, max: 100, step: 5, unit: '%', fb: '0',
    desc: ['显卡频率下限 (相对硬件最高频的百分比), 再闲也不低于这条线。', 
           '待机频率抬升, 滑动渲染零波动, 但耗电增加。', 
           '空闲充分降频省电, 极端场景可能偶发轻微掉帧。', 
           '游戏过场动画掉帧 → 抬到 20~30; 省电 → 保持 0。', 
           '0% (不限制)。'],
  },
  {
    path: 'max_pct', label: '最高频率', type: 'range', min: 20, max: 100, step: 5, unit: '%', fb: '100',
    desc: ['显卡频率上限 (相对硬件最高频的百分比)。', 
           '放开上限游戏满血, 发热耗电随之上升。', 
           '封顶限频明显降温, 重负载游戏帧率受影响。', 
           '夏天日常 70~80%; 跑分保持 100。', 
           '100% (不限制)。'],
  },
  {
    path: 'boost_util_pct', label: '加速阈值', type: 'range', min: 0, max: 100, step: 5, unit: '%', fb: '0',
    desc: ['显卡负载超过这条线时, 临时把最高频拉满; 回落后自动恢复上限 (0 = 关闭加速)。', 
           '更低阈值 → 更容易触发加速, 游戏更稳更耗电。', 
           '更高阈值 → 只在重负载时加速, 更省电。', 
           '游戏帧率不稳 → 60~70; 日常 → 0 关闭。', 
           '0% (关闭)。'],
  },
]

export const TOUCH_PARAMS: ParamSpec[] = [
  {
    path: 'enabled', label: '触摸加速开关', type: 'select', fb: 'true',
    options: [{ v: 'true', n: '开启 (跟手优先)' }, { v: 'false', n: '关闭 (省电优先)' }],
    desc: ['手指触到屏幕的瞬间立即唤醒核心, 保证滑动跟手。', 
           '(开启) 触摸瞬间核心就绪, 但待机功耗略增。', 
           '(关闭) 触摸后由负载决定唤醒, 极省电但开头几帧可能粘滞。', 
           '息屏黑屏时触摸加速本来就不参与; 息屏套一般关闭。', 
           '亮屏开启, 息屏关闭。'],
  },
  {
    path: 'extra_cores', label: '额外唤醒核心数', type: 'range', min: 0, max: 8, step: 1, unit: ' 个', fb: '8',
    desc: ['触摸瞬间除保留核心外额外唤醒几个核心 (8 = 全部唤醒, 即最跟手)。', 
           '唤醒更多核心 → 重开应用/游戏瞬间越流畅, 越费电。', 
           '只唤醒保留核心 → 最省电, 大核需等负载判定才起。', 
           '游戏切回 → 6~8; 日常聊天 → 2~3。', 
           '8 个 (全部)。'],
  },
  {
    path: 'duration_ms', label: '保护时长', type: 'range', min: 50, max: 1000, step: 50, unit: ' ms', fb: '200',
    desc: ['触摸唤醒核心后的保护窗口, 期间不允许关核。', 
           '更长的保护 → 连续滑动全程满血, 耗电略增。', 
           '更短的保护 → 点一下就回落, 最省电。', 
           '快速滑动感觉掉帧 → 加到 300~400。', 
           '200 毫秒。'],
  },
]

export const SWAP_PARAMS: ParamSpec[] = [
  {
    path: 'swappiness', label: '交换倾向', type: 'range', min: 0, max: 200, step: 10, fb: '100',
    desc: ['内核把内存数据换出到压缩交换区的积极程度 (即 vm.swappiness)。', 
           '更积极换出 → 后台保留更多, 但前台可能卡顿。', 
           '更不倾向换出 → 前台更流畅, 但后台更容易被杀。', 
           '游戏档建议 60~80; 大内存多后台 → 120~160。', 
           '100 (内核默认)。'],
  },
  {
    path: 'pressure_pct', label: '内存压力线', type: 'range', min: 0, max: 100, step: 5, unit: '%', fb: '20',
    desc: ['内存阻塞占比超过这条线时记预警日志 (仅监控提示, 不改变调度行为)。', 
           '更低 → 更早发现内存吃紧。', 
           '更高 → 只在严重不足时提醒。', 
           '感觉后台被杀频繁 → 调低到 10 提前观察。', 
           '20%。'],
  },
]

/* ==================== 读写 (橙色) — config.IO_Settings (注意 Scheduler 大写 S) ==================== */

export const IO_PARAMS: ParamSpec[] = [
  {
    path: 'IO_Settings.Scheduler', label: '存储调度算法', type: 'select', fb: '', asString: true,
    options: [
      { v: '', n: '保持内核默认' }, { v: 'mq-deadline', n: 'mq-deadline (低延迟)' },
      { v: 'bfq', n: 'bfq (公平流畅)' }, { v: 'kyber', n: 'kyber (均衡)' },
      { v: 'none', n: 'none (多核直通)' },
    ],
    desc: ['决定系统如何排队读写请求, 影响应用打开速度和滑动流畅度。',
           '(选更激进的算法) 操作响应更快, 但后台大量下载时前台可能被抢延迟。',
           '(选偏保守的算法) 后台任务更稳定, 但点开应用的瞬间响应略慢。',
           '闪存机型的日常使用选"低延迟"即可; 感觉滑动掉帧再试"公平流畅"。',
           'mq-deadline; 不要随意改 none, 部分内核会直接拒绝写入。'],
  },
  {
    path: 'IO_Settings.read_ahead_kb', label: '预读缓存', type: 'range', min: 32, max: 2048, step: 32,
    unit: ' KB', fb: '128', asString: true,
    desc: ['读到一块数据时, 提前把后面相邻的数据也读进缓存。适合顺序大文件场景。',
           '刷视频、看图更快; 占用内存变多, 小随机读取可能浪费带宽。',
           '省内存, 但启动应用、加载大图会慢。',
           '经常看在线视频或本地相册卡顿时可以调大。',
           '128~512 KB; 超过 1024 一般收益递减。'],
  },
  {
    path: 'IO_Settings.nomerges', label: '请求合并控制', type: 'select', fb: '2', asString: true,
    options: [{ v: '0', n: '允许合并 (吞吐优先)' }, { v: '1', n: '只禁读合并' }, { v: '2', n: '全部禁用 (延迟优先)' }],
    desc: ['控制内核是否把相邻的读写请求合并成一个大请求。',
           '"禁用合并"时每个请求独立执行, 单次操作延迟更低, 连续读写吞吐下降。',
           '"允许合并"时连续小读写拼成大请求, 拷贝文件更快, 个别请求等待变长。',
           '重度下载/拷贝选允许合并; 追求点击跟手可试"全部禁用"。',
           '保持"允许合并"; 游戏机型可选"全部禁用"微调手感。'],
  },
]

/** 读写功能总开关 (function.IOOptimization) 的五维说明 */
export const IO_OPT_DESC: [string, string, string, string, string] = [
  '读写优化的总闸门。关闭后下面的调度算法、预读缓存全部不生效。',
  '(开启) 模块接管全部存储设备的读写参数, 效果按上面的配置生效。',
  '(关闭) 存储完全交回系统默认, 此页其余设置变灰无效。',
  '怀疑读写优化与某个应用冲突时, 可以临时关闭对比。',
  '保持开启。',
]

/* ==================== 帧平滑 (粉色) — rules.yaml fas_rules (即时热重载) ==================== */

export const FAS_PARAMS: ParamSpec[] = [
  {
    path: 'fps_margin', label: '帧率容差', type: 'range', min: 0.5, max: 10, step: 0.5, fb: '3', unit: ' 帧',
    desc: ['实际帧率比目标低多少帧以内, 都算达标不干预。是"松紧带"的宽度。',
           '干预少, 掉帧一两帧也不提频, 更凉快省电。',
           '帧率盯得紧, 稍有下滑立刻拉频率, 更稳但更耗电。',
           '游戏画面偶尔小幅波动但不想费电 → 调大; 追求满帧丝滑 → 调小。',
           '3 帧; 电竞玩家建议 2。'],
  },
  {
    path: 'pid.kp', label: '提频灵敏度', type: 'num', min: 0, max: 5, step: 0.01, fb: '0.5',
    desc: ['比例系数: 帧时间超出预算越多, 提频动作越大。调速器的"主油门"。',
           '掉帧瞬间猛拉频率, 回帧快, 功耗尖峰明显。',
           '提频温和省电, 但重度负载下追帧慢半拍。',
           '团战掉帧回不上来 → 调大; 正常玩却发烫 → 调小。',
           '保持默认 0.5 附近, 一次别超过 ±0.2。'],
  },
  {
    path: 'pid.ki', label: '持续偏低补偿', type: 'num', min: 0, max: 2, step: 0.01, fb: '0.05',
    desc: ['积分系数: 帧率持续低于目标时逐步加力, 直到跟上为止。',
           '长时间重载能顶住目标帧率, 发热累积也更快。',
           '更保守, 持续高负载下允许略低于目标帧率运行。',
           '游戏十分钟后帧率慢慢掉 → 调大; 恒温优先 → 调小。',
           '0.05 起步, 幅度远小于灵敏度。'],
  },
  {
    path: 'pid.kd', label: '抖动抑制', type: 'num', min: 0, max: 5, step: 0.01, fb: '0.1',
    desc: ['微分系数: 帧时间突然跳变时先"预刹", 减少频率震荡。',
           '抗瞬时尖峰更强, 高刷新率屏幕上更稳。',
           '太大反而迟钝, 对真实掉帧反应慢。',
           '120 赫兹设备出现频率来回抽动 → 调大。',
           '60 赫兹保持 0.05~0.1; 90/120 赫兹可到 0.15。'],
  },
]

/** 帧率档位文本框 (fps_gears) 的五维说明 */
export const FAS_GEARS_DESC: [string, string, string, string, string] = [
  '帧平滑允许使用的目标帧率档位列表 (逗号分隔), 引擎按游戏实际渲染自动选档。',
  '(加入更高档) 支持 90/120 高刷满帧, 功耗上限提高。',
  '(只用低档) 发热与耗电更低。',
  '玩 120 赫兹游戏 → 加入 120; 只玩 60 帧网游 → 45,60 即可。',
  '30,60,90,120 全档覆盖。']

/* ==================== 核心开关 (红色) — hotplug/config.yaml, 改动自动生效 ==================== */

/** 保留核心勾选组 (亮屏组 + 息屏组共用一条五维) */
export const KEEP_DESC: [string, string, string, string, string] = [
  '无论怎么配, 核心 1 是启动核心必须常开; 保底至少两个核心防止全部休眠。',
  '勾得越多游戏越稳但越费电; 勾得越少越省电但亮屏切换可能变慢。',
  '息屏组建议只留 2 个小核心; 亮屏组按日常需要选择。',
  '亮屏觉得卡就多保留大核 (核心 5~8); 待机掉电快就减少息屏保留。',
  '亮屏默认保留核心 1~6, 息屏默认保留核心 1~2。']

export const HOTPLUG_PARAMS: ParamSpec[] = [
  {
    path: 'min_online_cores', label: '最少在线核心数', type: 'range', min: 2, max: 8, step: 1, unit: ' 个', fb: '4',
    desc: ['任何时刻至少保持这么多核心在线, 是关核的硬底线 (与保留核心叠加生效)。',
           '更抗突发负载, 通知/闹钟唤醒更快, 但待机功耗升高。',
           '更省电, 突发任务需要先唤醒核心, 可能轻微迟滞。',
           '接收消息多 → 保持 4; 极致待机 → 降为 2。',
           '4 个。'],
  },
  {
    path: 'off_threshold_idle_pct', label: '关核阈值', type: 'range', min: 50, max: 100, step: 1, scale: 1, unit: '%', fb: '95',
    desc: ['核心空闲度超过这个百分比且持续约 1 秒, 才会被关闭休眠。',
           '关核更保守, 负载低谷也尽量保持在线, 随时可干活但费电。',
           '更容易关核省电, 但负载刚落就被关, 再起量时要等唤醒。',
           '轻度使用嫌发热 → 调低试试; 追求响应 → 维持高位。',
           '95%; 波动型负载可降到 90。'],
  },
  {
    path: 'on_threshold_util_pct', label: '开核阈值', type: 'range', min: 5, max: 80, step: 1, unit: '%', fb: '30',
    desc: ['核心利用率超过这个百分比且连续两次采样达标, 立即唤醒该核心。',
           '一有活干就叫醒, 手感跟手, 待机功耗轻微上升。',
           '只在真正忙碌时开核, 最省电, 突发操作开头可能有几十毫秒延迟。',
           '游戏/抢购类瞬发场景 → 调低; 夜间待机发热排查 → 调高。',
           '30%。'],
  },
]

export const LOCKSCREEN_DESC: [string, string, string, string, string] = [
  '锁屏界面 (按键息屏但屏幕仍亮) 时是否允许动态休眠闲置核心。',
  '(开启) 锁屏也省电, 来通知唤醒核心稍慢一点点。',
  '(关闭) 锁屏保持全部核心在线, 锁屏时钟/通知更顺滑但费电。',
  '锁屏挂下载/导航 → 关; 一般使用 → 开。',
  '建议开启。']

export const SCREENS_OFF_DESC: [string, string, string, string, string] = [
  '完全黑屏时是否允许关核, 配合"息屏保留核心"工作。',
  '(开启) 黑屏只留保底核心, 待机掉电显著变慢。',
  '(关闭) 黑屏全核在线, 待机耗电明显增加。',
  '没有黑屏后台任务需求就保持开启。',
  '建议开启。']

/* ==================== 温度保护 (深红) ==================== */

export const TEMP_PARAM: ParamSpec = {
  path: 'thermal_force_all_on_c', label: '强制全核温度线', type: 'range', min: 45, max: 95, step: 1, unit: '°C', fb: '70',
  desc: ['处理器温度达到这条线时, 所有已休眠的核心立即强制拉回在线 — 过热比费电更危险。',
         '(调低这条线) 保护更早介入, 高温天性能更稳。',
         '(调高这条线) 尽量晚干预, 高负载机身更烫才触发。',
         '夏天烫手掉帧 → 下调到 65 提前保护。',
         '70°C 行业安全线, 一般不动。'],
}

/* ==================== 显卡 (紫色) / 触摸加速 (青色) / 内存 (绿色) — 自动管理说明 ==================== */

export const GPU_DESC: [string, string, string, string, string] = [
  '显卡的运行频率、档位由守护进程在游戏时实时计算, 掉帧瞬间拉频、空闲立刻回落。',
  '(如手动锁定高频) 发热剧增且多数内核会拒绝写入, 容易与其他模块冲突。',
  '(如锁定低频) 游戏必然掉帧, 帧平滑会不断纠正导致震荡。',
  '无需手动调整; 想要更高显卡性能直接用首页「性能」或「极速」模式。',
  '保持自动即可。']

export const TOUCH_DESC: [string, string, string, string, string] = [
  '手指触到屏幕的一瞬间立即唤醒全部核心并冻结关核约 0.2 秒, 保证滑动跟手。',
  '(延长保护窗) 更跟手, 但待机功耗略微增加。',
  '(缩短) 更省电, 快速滑动开始几帧可能轻微粘滞。',
  '无需设置; 点击偶发迟钝时优先检查「开核阈值」是否过高。',
  '内置策略自动运行。']

export const SWAP_DESC: [string, string, string, string, string] = [
  '内存交换倾向 (旧称"交换倾向"参数) 与压缩回收由系统压缩交换策略自动处理; 守护进程实时监测内存压力并让调度器让路。',
  '(倾向更高) 省内存但更容易卡顿。',
  '(倾向更低) 更流畅但更容易杀后台。',
  '交由系统自动调节是最优解, 无需手动设置。',
  '保持自动。']