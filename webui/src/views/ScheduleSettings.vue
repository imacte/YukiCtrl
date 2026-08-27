<!--
  src/views/ScheduleSettings.vue — 调度页 (任务 B 彻底重做)

  八张主题色卡片, 每个参数下方直接展示五维说明:
    这是什么 / 调高会怎样 / 调低会怎样 / 什么情况建议调 / 建议值

  数据源与生效方式 (全部热更新, 无需重启守护进程):
    - 核心开关卡: hotplug/config.yaml (daemon 200ms tick 轮询)
    - 处理器 / 读写卡: config/config.yaml (inotify 热重载, ~1s 生效)
    - 帧平滑卡: rules.yaml fas_rules (inotify → ConfigReload 热重载)
    - 显卡 / 内存 / 触摸 / 温度实时值: sense/snapshot.yaml (只读监控)

  与旧版差异: 移除假面板 (swappiness/dirty_ratio/GPU 锁频等 daemon 不消费的参数),
  只保留真实生效或如实标注"自动管理"的条目.
-->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { h, defineComponent } from 'vue'
import yaml from 'js-yaml'
import { Bridge } from '@/utils/bridge'
import { useSchedulerStore } from '@/stores/scheduler'
import {
  fetchHotplugState, fetchHotplugConfig, saveHotplugConfig,
  sanitizeKeepCores, type HotplugState, type HotplugConfig,
} from '@/api/hotplug'
import { fetchSenseSnapshot, type SenseSnapshot } from '@/api/sense'

const store = useSchedulerStore()

/* ================= 全局数据 ================= */
const hpState = ref<HotplugState | null>(null)
const hpCfg = ref<HotplugConfig>({
  lockscreen_onoff: true, screens_onoff: true,
  off_threshold_idle_pct: 95, on_threshold_util_pct: 30,
  min_online_cores: 4, thermal_force_all_on_c: 70,
  screen_on_keep_cores: [0, 1, 2, 3, 4, 5], screen_off_keep_cores: [0, 1],
})
const mainCfg = ref<any>(null)          // config/config.yaml 整体
const rulesCfg = ref<any>(null)         // rules.yaml 整体
const sense = ref<SenseSnapshot | null>(null)
const loading = ref(true)
const errMsg = ref('')
const okMsg = ref('')

let pollTimer: number | null = null

function flashOk(msg: string) {
  okMsg.value = msg
  setTimeout(() => { okMsg.value = '' }, 1800)
}

/** 深路径读写 (如 'balance.cpu_load_governor.up_threshold') */
function getP(path: string, _spec?: unknown): any {
  let cur = mainCfg.value
  for (const k of path.split('.')) { cur = cur?.[k]; if (cur === undefined) return undefined }
  return cur
}
function setP(path: string, v: any) {
  const keys = path.split('.')
  let cur = mainCfg.value
  for (let i = 0; i < keys.length - 1; i++) {
    if (cur[keys[i]] === undefined || cur[keys[i]] === null) cur[keys[i]] = {}
    cur = cur[keys[i]]
  }
  cur[keys[keys.length - 1]] = v
}

async function loadAll() {
  loading.value = true
  errMsg.value = ''
  try {
    // config/config.yaml 必须存在 (daemon 启动包自带); 若缺失给空对象兜底
    try { mainCfg.value = await Bridge.getMainConfig() } catch { mainCfg.value = {} }
    try { rulesCfg.value = await Bridge.getRulesConfig() } catch { rulesCfg.value = {} }
    hpCfg.value = await fetchHotplugConfig()
    hpState.value = await fetchHotplugState()
    sense.value = await fetchSenseSnapshot()
  } catch (e) {
    errMsg.value = String(e)
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  await store.initData()
  await loadAll()
  pollTimer = window.setInterval(async () => {
    try {
      hpState.value = await fetchHotplugState()
      sense.value = await fetchSenseSnapshot()
    } catch { /* 单次轮询失败忽略 */ }
  }, 1500)
  afterLoadSync()
})
onUnmounted(() => { if (pollTimer !== null) window.clearInterval(pollTimer) })

/* ================= 保存动作 ================= */
let hpSaveTimer: number | null = null
/** hotplug 配置防抖保存: 改完 600ms 自动写盘, daemon 下个 tick 生效 */
function persistHp() {
  if (hpSaveTimer !== null) window.clearTimeout(hpSaveTimer)
  hpSaveTimer = window.setTimeout(async () => {
    try {
      await saveHotplugConfig(hpCfg.value)
      flashOk('核心开关已生效')
    } catch (e) { errMsg.value = String(e) }
  }, 600)
}
async function saveMain() {
  try {
    await Bridge.saveMainConfig(yaml.load(yaml.dump(mainCfg.value)))
    flashOk('已保存, 约 1 秒内生效')
  } catch (e) { errMsg.value = String(e) }
}
async function saveFas() {
  try {
    await Bridge.saveRulesConfig(rulesCfg.value)
    flashOk('帧平滑参数已生效')
  } catch (e) { errMsg.value = String(e) }
}

/* ================= 参数声明 (五维说明直接内联) ================= */
interface ParamSpec {
  path: string            // mainCfg 深路径 / CLG 内字段名 / fas_rules 内路径
  label: string
  type: 'range' | 'select' | 'num'
  min?: number; max?: number; step?: number
  scale?: number          // 显示换算: 显示值 = 实际值 * scale (CLG 0..1 → 百分比用 100)
  unit?: string
  fb?: string             // 磁盘上缺省该键时的展示兜底值
  options?: { v: string | number; n: string }[]
  desc: [string, string, string, string, string]  // 是什么/调高/调低/何时调/建议值
}

const fmtVal = (p: ParamSpec, v: any): string => {
  const s = p.scale ?? 1
  return `${(Number(v) * s).toFixed(s === 1 ? 0 : (p.step && p.step < 1 ? 2 : 0))}${p.unit ?? ''}`
}

/* --- 处理器设置 (蓝色) — 当前模式的负载调速器 CLG --- */
const clgMode = ref('balance')
const modeNames: Record<string, string> = {
  powersave: '省电', balance: '均衡', performance: '性能', fast: '极速',
}
const clgParams: ParamSpec[] = [
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
const clgBase = computed(() => `${clgMode.value}.cpu_load_governor`)

/* --- 读写设置 (橙色) — 存储调度 --- */
const ioParams: ParamSpec[] = [
  {
    path: 'IO_Settings.scheduler', label: '存储调度算法', type: 'select', fb: '',
    options: [
      { v: '', n: '保持内核默认' }, { v: 'mq-deadline', n: 'mq-deadline (低延迟)' },
      { v: 'bfq', n: 'bfq (公平流畅)' }, { v: 'kyber', n: 'kyber (均衡)' },
      { v: 'none', n: 'none (多核直通)' },
    ],
    desc: ['决定系统如何排队读写请求, 影响应用打开速度和滑动流畅度。',
           '(选更激进的算法) 操作响应更快, 但后台大量下载时前台可能被抢延迟。',
           '(选偏保守的算法) 后台任务更稳定, 但点开应用的瞬间响应略慢。',
           'UFS 机型的日常使用选 mq-deadline 即可; 感觉滑动掉帧再试 bfq。',
           'mq-deadline; 不要随意改 none, 部分内核会直接拒绝写入。'],
  },
  {
    path: 'IO_Settings.read_ahead_kb', label: '预读缓存', type: 'range', min: 32, max: 2048, step: 32,
    unit: ' KB', fb: '128',
    desc: ['读到一块数据时, 提前把后面相邻的数据也读进缓存。适合顺序大文件场景。',
           '刷视频、看图更快; 占用内存变多, 小随机读取可能浪费带宽。',
           '省内存, 但启动应用、加载大图会慢。',
           '经常看在线视频或本地相册卡顿时可以调大。',
           '128~512 KB; 超过 1024 一般收益递减。'],
  },
  {
    path: 'IO_Settings.nomerges', label: '请求合并控制', type: 'select', fb: '2',
    options: [{ v: '0', n: '允许合并 (吞吐优先)' }, { v: '1', n: '只禁读合并' }, { v: '2', n: '全部禁用 (延迟优先)' }],
    desc: ['控制内核是否把相邻的读写请求合并成一个大请求。',
           '"禁用合并"时每个请求独立执行, 单次操作延迟更低, 连续读写吞吐下降。',
           '"允许合并"时连续小读写拼成大请求, 拷贝文件更快, 个别请求等待变长。',
           '重度下载/拷贝选允许合并; 追求点击跟手可试 2。',
           '保持 0; 游戏机型可选 2 微调手感。'],
  },
]

/* --- 帧平滑 (粉色) — FAS 规则 (rules.yaml, 即刻热重载) --- */
const fasParams: ParamSpec[] = [
  {
    path: 'fps_margin', label: '帧率容差', type: 'range', min: 0.5, max: 10, step: 0.5, fb: '3',
    unit: ' 帧',
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
           '120Hz 设备出现频率来回抽动 → 调大。',
           '60Hz 保持 0.05~0.1; 90/120Hz 可到 0.15。'],
  },
]

/* ================= 通用小组件 / 辅助 ================= */
const DescBlock = defineComponent({
  props: { desc: { type: Array as () => string[], required: true } },
  setup(props) {
    const keys = ['这是什么', '调高会怎样', '调低会怎样', '何时建议调', '建议值']
    return () => h('div', { class: 'd-wrap' },
      props.desc.map((d, i) => h('div', { class: 'd-line', key: i }, [
        h('span', { class: 'd-k' }, (keys[i] ?? '') + ' ·'),
        h('span', { class: 'd-v' }, d),
      ])),
    )
  },
})

/** 卡片左侧主题色条 */
function cardStyle(color: string) {
  return { borderLeft: `4px solid ${color}` }
}

/* --- rules.yaml fas_rules 路径读写 --- */
function getFas(path: string, _spec?: unknown): any {
  let cur = rulesCfg.value?.fas_rules
  for (const k of path.split('.')) { cur = cur?.[k]; if (cur === undefined) return undefined }
  return cur
}
function setFas(path: string, v: any) {
  if (!rulesCfg.value.fas_rules) rulesCfg.value.fas_rules = {}
  const keys = path.split('.')
  let cur = rulesCfg.value.fas_rules
  for (let i = 0; i < keys.length - 1; i++) {
    if (!cur[keys[i]]) cur[keys[i]] = {}
    cur = cur[keys[i]]
  }
  cur[keys[keys.length - 1]] = v
}
const fmtNum = (v: any): string => (Number.isFinite(Number(v)) ? String(v) : '--')

/* 目标帧率档位 (逗号分隔文本框 <-> 数组) */
const fasGearsText = ref('30,60,90,120')
function syncGearsText() {
  const gears = getFas('fps_gears')
  if (Array.isArray(gears)) fasGearsText.value = gears.join(',')
}
function applyFasGears() {
  const arr = fasGearsText.value
    .split(',').map(s => parseFloat(s.trim()))
    .filter(n => Number.isFinite(n) && n > 0)
  if (arr.length === 0) { syncGearsText(); return }
  setFas('fps_gears', arr)
}

/** 数据加载完成后把磁盘值同步进文本框 */
function afterLoadSync() {
  syncGearsText()
}

/* ================= 核心开关卡辅助 ================= */
const CORES = Array.from({ length: 8 }, (_, i) => i)
const keepGroups = [
  {
    key: 'screen_on_keep_cores' as const,
    title: '亮屏时保留的核心',
    hint: '屏幕亮着时这些核心永不关闭, 其余核心按负载动态休眠',
  },
  {
    key: 'screen_off_keep_cores' as const,
    title: '息屏时保留的核心',
    hint: '黑屏待机时只保底这些核心, 更省电',
  },
]
function toggleKeep(group: 'screen_on_keep_cores' | 'screen_off_keep_cores', core: number) {
  const list = hpCfg.value[group]
  const idx = list.indexOf(core)
  if (idx >= 0) {
    // 取消勾选: cpu0 不允许取消 (boot CPU); 至少留 2 个由保存前兜底补齐
    if (core === 0) return
    list.splice(idx, 1)
    if (list.length < 2) list.push(list.includes(1) ? 0 : 1)
    hpCfg.value[group] = sanitizeKeepCores([...list])
  } else {
    list.push(core)
    hpCfg.value[group] = sanitizeKeepCores([...list])
  }
  persistHp()
}
const activeKeepNums = computed<number[]>(() =>
  (hpState.value?.active_keep_cores ?? '')
    .split(',').map(s => parseInt(s.trim(), 10)).filter(n => !Number.isNaN(n)))
const onlineCount = computed(() =>
  hpState.value ? maskToCount(hpState.value.online_mask) : '--')
function maskToCount(mask: number): number {
  let n = 0
  for (let i = 0; i < 8; i++) if (mask & (1 << i)) n++
  return n
}
</script>
<template>
  <div class="sched-page">
    <header class="page-head">
      <span class="page-title">调度设置</span>
      <span class="page-sub">所有修改自动生效, 无需重启</span>
    </header>

    <div v-if="errMsg" class="banner err">⚠ {{ errMsg }}</div>
    <div v-if="okMsg" class="banner ok">{{ okMsg }}</div>
    <div v-if="loading" class="banner">读取配置中...</div>

    <!-- ── 处理器设置 (蓝色) ── -->
    <section class="card" :style="cardStyle('#3b82f6')">
      <div class="card-head">
        <span class="card-name">处理器设置</span>
        <button class="save-btn" @click="saveMain">保存</button>
      </div>
      <p class="card-intro">按模式独立记忆。先选模式, 再调参数; 保存后约 1 秒生效。</p>
      <div class="chip-row">
        <button v-for="(n, k) in modeNames" :key="k"
                class="chip" :class="{ on: clgMode === k }" @click="clgMode = k">
          {{ n }}
        </button>
      </div>

      <div v-for="p in clgParams" :key="p.label" class="param">
        <div class="param-head">
          <span class="p-label">{{ p.label }}</span>
          <span class="p-val">{{ fmtVal(p, getP(clgBase + '.' + p.path, p) ?? p.fb ?? 0) }}</span>
        </div>
        <input type="range" class="p-range" :min="p.min" :max="p.max" :step="p.step"
               :value="Number(getP(clgBase + '.' + p.path, p) ?? p.fb ?? 0)"
               @input="(e: Event) => { setP(clgBase + '.' + p.path, Number((e.target as HTMLInputElement).value)) }" />
        <DescLines :desc="p.desc" />
      </div>
    </section>

    <!-- ── 显卡设置 (紫色) — 只读监控 ── -->
    <section class="card" :style="cardStyle('#8b5cf6')">
      <div class="card-head"><span class="card-name">显卡设置</span><span class="auto-tag">自动管理</span></div>
      <p class="card-intro">显卡频率由帧平滑引擎根据游戏负载自动调节 (它比手动锁频更聪明:
        掉帧瞬间拉频、空闲立刻回落)。此处展示实时负载:</p>
      <div class="readout">
        <div><span>显卡负载</span><b>{{ sense ? Math.round(sense.gpu_load_pct) : '--' }}%</b></div>
        <div><span>屏幕帧率</span><b>{{ sense?.fps || '--' }} 帧/秒</b></div>
      </div>
      <DescLines :desc="[
        '显卡的运行频率、档位由守护进程在游戏时实时计算。',
        '(如手动锁定高频) 发热剧增且多数内核会拒绝写入, 容易与其他模块冲突。',
        '(如锁定低频) 游戏必然掉帧, 帧平滑会不断纠正导致震荡。',
        '无需手动调整; 想要更高显卡性能直接用首页「性能」或「极速」模式。',
        '保持自动即可。']" />
    </section>

    <!-- ── 读写设置 (橙色) ── -->
    <section class="card" :style="cardStyle('#f59e0b')">
      <div class="card-head">
        <span class="card-name">读写设置</span>
        <button class="save-btn" @click="saveMain">保存</button>
      </div>
      <p class="card-intro">影响应用打开速度、滑动加载速度; 保存后约 1 秒热生效。</p>
      <template v-for="p in ioParams" :key="p.path">
        <div v-if="p.type === 'select'" class="param">
          <div class="param-head"><span class="p-label">{{ p.label }}</span></div>
          <select class="p-select" :value="String(getP(p.path, p) ?? p.fb ?? '')"
                  @change="(e: Event) => setP(p.path, (e.target as HTMLSelectElement).value)">
            <option v-for="o in p.options" :key="o.v" :value="o.v">{{ o.n }}</option>
          </select>
          <DescLines :desc="p.desc" />
        </div>
        <div v-else class="param">
          <div class="param-head">
            <span class="p-label">{{ p.label }}</span>
            <span class="p-val">{{ fmtVal(p, getP(p.path, p) ?? p.fb ?? 0) }}</span>
          </div>
          <input type="range" class="p-range" :min="p.min" :max="p.max" :step="p.step"
                 :value="Number(getP(p.path, p) ?? p.fb ?? 0)"
                 @input="(e: Event) => setP(p.path, Number((e.target as HTMLInputElement).value))" />
          <DescLines :desc="p.desc" />
        </div>
      </template>
    </section>
    <!-- ── 内存设置 (绿色) — 只读监控 ── -->
    <section class="card" :style="cardStyle('#10b981')">
      <div class="card-head"><span class="card-name">内存设置</span><span class="auto-tag">自动管理</span></div>
      <div class="readout">
        <div><span>内存压力</span><b>{{ sense ? sense.mem_full_pct.toFixed(1) : '--' }}%</b></div>
        <div><span>ZRAM 已用</span><b>{{ sense?.swap_used_mb ?? '--' }} MB</b></div>
      </div>
      <p class="card-intro">
        内存交换倾向 (旧称 swappiness)、压缩回收由系统 ZRAM 策略自动处理;
        守护进程实时监测内存压力并让调度器让路。交换倾向调高省内存但卡顿、
        调低流畅但易杀后台——交由系统自动调节是最优解。</p>
    </section>

    <!-- ── 核心开关 (红色) — hotplug 即时生效 ── -->
    <section class="card" :style="cardStyle('#ef4444')">
      <div class="card-head"><span class="card-name">核心开关</span><span class="live-tag">改动即生效</span></div>

      <div v-for="g in keepGroups" :key="g.key" class="keep-block">
        <div class="keep-title">{{ g.title }}</div>
        <div class="keep-hint">{{ g.hint }}</div>
        <div class="keep-grid">
          <button v-for="c in CORES" :key="c"
                  class="keep-btn"
                  :class="{ on: hpCfg[g.key].includes(c), locked: c === 0 }"
                  @click="toggleKeep(g.key, c)">
            核心{{ c + 1 }}<small v-if="c === 0">必留</small>
          </button>
        </div>
      </div>
      <DescBlock :desc="[
        '无论怎么配, 核心1 (cpu0) 是启动核心必须常开; 保底至少两个核心防止全部休眠。',
        '勾得越多游戏越稳但越费电; 勾得越少越省电但亮屏切换可能变慢。',
        '息屏组建议只留 2 个小核心; 亮屏组按日常需要选择。',
        '亮屏觉得卡就多保留大核(核心5~8); 待机掉电快就减少息屏保留。',
        '亮屏默认保留核心1~6, 息屏默认保留核心1~2.']" />

      <div class="param">
        <div class="param-head"><span class="p-label">最少在线核心数</span>
          <span class="p-val">{{ hpCfg.min_online_cores }} 个</span></div>
        <input type="range" class="p-range" min="2" max="8" step="1"
               v-model.number="hpCfg.min_online_cores" @input="persistHp" />
        <DescBlock :desc="[
          '任何时刻至少保持这么多核心在线, 是关核的硬底线 (与保留核心叠加生效)。',
          '更抗突发负载, 通知/闹钟唤醒更快, 但待机功耗升高。',
          '更省电, 突发任务需要先唤醒核心, 可能轻微迟滞。',
          '接收消息多 → 保持 4; 极致待机 → 降为 2。',
          '4 个。']" />
      </div>
      <div class="param">
        <div class="param-head"><span class="p-label">关核阈值</span>
          <span class="p-val">{{ hpCfg.off_threshold_idle_pct }}%</span></div>
        <input type="range" class="p-range" min="50" max="100" step="1"
               v-model.number="hpCfg.off_threshold_idle_pct" @input="persistHp" />
        <DescBlock :desc="[
          '核心空闲度超过这个百分比且持续约 1 秒, 才会被关闭休眠。',
          '关核更保守, 负载低谷也尽量保持在线, 随时可干活但费电。',
          '更容易关核省电, 但负载刚落就被关, 再起量时要等唤醒。',
          '轻度使用嫌发热 → 调低试试; 追求响应 → 维持高位。',
          '95%; 波动型负载可降到 90。']" />
      </div>
      <div class="param">
        <div class="param-head"><span class="p-label">开核阈值</span>
          <span class="p-val">{{ hpCfg.on_threshold_util_pct }}%</span></div>
        <input type="range" class="p-range" min="5" max="80" step="1"
               v-model.number="hpCfg.on_threshold_util_pct" @input="persistHp" />
        <DescBlock :desc="[
          '核心利用率超过这个百分比且连续两次采样达标, 立即唤醒该核心。',
          '一有活干就叫醒, 手感跟手, 待机功耗轻微上升。',
          '只在真正忙碌时开核, 最省电, 突发操作开头可能有几十毫秒延迟。',
          '游戏/抢购类瞬发场景 → 调低; 夜间待机发热排查 → 调高。',
          '30%。']" />
      </div>

      <div class="switch-row">
        <div><b>锁屏时允许关核</b><small>锁屏界面仍动态休眠闲置核心</small></div>
        <van-switch v-model="hpCfg.lockscreen_onoff" size="22px" @change="persistHp" />
      </div>
      <div class="switch-row">
        <div><b>灭屏时允许关核</b><small>配合息屏保留核心工作, 推荐开启</small></div>
        <van-switch v-model="hpCfg.screens_onoff" size="22px" @change="persistHp" />
      </div>
      <div class="state-line">
        当前状态: {{ hpState?.screen_on ? '亮屏' : '息屏' }} · 在线 {{ onlineCount }}/8 ·
        生效白名单 [{{ hpState?.active_keep_cores ?? '--' }}] · 温度 {{ hpState?.thermal_c?.toFixed(1) ?? '--' }}°C
      </div>
    </section>

    <!-- ── 触摸加速 (青色) — 自动旁路 ── -->
    <section class="card" :style="cardStyle('#06b6d4')">
      <div class="card-head"><span class="card-name">触摸加速</span><span class="auto-tag">自动触发</span></div>
      <div class="readout"><div><span>当前触摸</span><b>{{ sense?.touch_down ? '按下中' : '未按下' }}</b></div></div>
      <DescBlock :desc="[
        '手指触到屏幕的一瞬间立即唤醒全部核心并冻结关核约 0.2 秒, 保证滑动跟手。',
        '(延长保护窗) 更跟手, 但待机功耗略微增加。',
        '(缩短) 更省电, 快速滑动开始几帧可能轻微粘滞。',
        '无需设置; 点击偶发迟钝时优先检查「开核阈值」是否过高。',
        '内置策略自动运行。']" />
    </section>

    <!-- ── 帧平滑 (粉色) — FAS ── -->
    <section class="card" :style="cardStyle('#ec4899')">
      <div class="card-head">
        <span class="card-name">帧平滑</span>
        <button class="save-btn" @click="saveFas">保存</button>
      </div>
      <p class="card-intro">仅对规则页中标记为 FAS 的前台应用生效。当前调度模式: {{ store.currentMode }}</p>
      <div class="param">
        <div class="param-head"><span class="p-label">目标帧率档位</span></div>
        <input type="text" class="p-input" v-model="fasGearsText"
               placeholder="如 30,60,90,120" @change="applyFasGears" />
        <DescBlock :desc="[
          '帧平滑允许使用的目标帧率档位列表 (逗号分隔), 引擎按游戏实际渲染自动选档。',
          '(加入更高档) 支持 90/120 高刷满帧, 功耗上限提高。',
          '(只用低档) 发热与耗电更低。',
          '玩 120Hz 游戏 → 加入 120; 只玩 60 帧网游 → 45,60 即可。',
          '30,60,90,120 全档覆盖。']" />
      </div>
      <div v-for="p in fasParams" :key="p.path" class="param">
        <div class="param-head">
          <span class="p-label">{{ p.label }}</span>
          <span class="p-val">{{ fmtNum(getFas(p.path, p)) }}</span>
        </div>
        <input v-if="p.type === 'range'" type="range" class="p-range" :min="p.min" :max="p.max" :step="p.step"
               :value="Number(getFas(p.path, p) ?? p.fb ?? 0)"
               @input="(e: Event) => setFas(p.path, Number((e.target as HTMLInputElement).value))" />
        <input v-else type="number" class="p-input num" :min="p.min" :max="p.max" :step="p.step"
               :value="Number(getFas(p.path, p) ?? p.fb ?? 0)"
               @change="(e: Event) => setFas(p.path, Number((e.target as HTMLInputElement).value))" />
        <DescBlock :desc="p.desc" />
      </div>
    </section>

    <!-- ── 温度保护 (深红) ── -->
    <section class="card" :style="cardStyle('#991b1b')">
      <div class="card-head"><span class="card-name">温度保护</span><span class="live-tag">改动即生效</span></div>
      <div class="readout"><div><span>当前温度</span><b>{{ hpState?.thermal_c?.toFixed(1) ?? '--' }}°C</b></div></div>
      <div class="param">
        <div class="param-head"><span class="p-label">强制全核温度线</span>
          <span class="p-val">{{ hpCfg.thermal_force_all_on_c }}°C</span></div>
        <input type="range" class="p-range" min="45" max="95" step="1"
               v-model.number="hpCfg.thermal_force_all_on_c" @input="persistHp" />
        <DescBlock :desc="[
          '处理器温度达到这条线时, 所有已休眠的核心立即强制拉回在线 — 过热比费电更危险。',
          '(调低这条线) 保护更早介入, 高温天性能更稳。',
          '(调高这条线) 尽量晚干预, 高负载机身更烫才触发。',
          '夏天烫手掉帧 → 下调到 65 提前保护。',
          '70°C 行业安全线, 一般不动。']" />
      </div>
      <p class="card-intro">触发后守护进程暂停一切关核动作直到温度回落; 该保护优先级最高。</p>
    </section>

    <footer class="foot">核心领航员 · 所有修改实时写入设备并由守护进程热加载</footer>
  </div>
</template>

<style scoped>
.sched-page { padding: 16px; max-width: 600px; margin: 0 auto; }
.page-head { display: flex; flex-direction: column; gap: 2px; padding: 6px 4px 14px; }
.page-title { font-size: 22px; font-weight: 700; }
.page-sub { font-size: 12px; color: var(--text-muted); }

.banner { padding: 8px 12px; border-radius: 8px; margin-bottom: 10px; font-size: 13px; }
.banner.err { background: var(--danger-soft); color: var(--danger); }
.banner.ok { background: var(--success-soft); color: var(--success); }

.card {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 14px;
  margin-bottom: 12px;
}
.card-head { display: flex; justify-content: space-between; align-items: center; }
.card-name { font-size: 16px; font-weight: 700; }
.save-btn {
  border: none; background: var(--accent); color: #fff;
  padding: 7px 18px; border-radius: 18px; font-size: 13px;
}
.save-btn:active { opacity: .85; }
.auto-tag, .live-tag { font-size: 11px; padding: 3px 10px; border-radius: 10px; }
.auto-tag { color: var(--text-secondary); background: rgba(0,0,0,.06); }
.live-tag { color: var(--success); background: var(--success-soft); }

.card-intro { font-size: 12.5px; color: var(--text-secondary); line-height: 1.65; margin: 10px 0 4px; }

.chip-row { display: flex; gap: 8px; margin: 10px 0 4px; }
.chip {
  flex: 1; border: 1.5px solid var(--border); background: var(--bg-base);
  border-radius: 16px; padding: 7px 0; font-size: 13px; color: var(--text-secondary);
}
.chip.on { border-color: var(--accent); color: var(--accent); background: rgba(59,130,246,.10); font-weight: 600; }

.param { margin-top: 16px; padding-top: 12px; border-top: 1px dashed var(--border); }
.param:first-of-type { border-top: none; margin-top: 4px; }
.param-head { display: flex; justify-content: space-between; align-items: baseline; }
.p-label { font-size: 14.5px; font-weight: 600; color: var(--text-primary); }
.p-val { font-size: 13.5px; font-weight: 600; color: var(--accent); font-variant-numeric: tabular-nums; }

.p-range { width: 100%; accent-color: var(--accent); height: 30px; }
.p-select {
  width: 100%; margin-top: 6px; padding: 9px 10px;
  border: 1px solid var(--border-strong); border-radius: 8px;
  background: var(--bg-card); color: var(--text-primary); font-size: 13.5px;
}
.p-input {
  width: 100%; box-sizing: border-box; margin-top: 6px; padding: 9px 10px;
  border: 1px solid var(--border-strong); border-radius: 8px;
  font-size: 14px; color: var(--text-primary);
}
.p-input.num { width: 120px; }

/* 五维说明 */
.d-wrap {
  margin-top: 7px; padding: 9px 10px;
  background: var(--bg-base); border-radius: 8px;
}
.d-line { display: flex; gap: 6px; font-size: 12px; line-height: 1.65; color: var(--text-secondary); }
.d-k { flex-shrink: 0; color: var(--accent); font-weight: 600; white-space: nowrap; }
.d-v { flex: 1; }

.readout { display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px; margin-top: 10px; }
.readout div {
  background: var(--bg-base); border-radius: 8px; padding: 9px 10px;
  display: flex; justify-content: space-between; align-items: center;
}
.readout span { font-size: 12px; color: var(--text-muted); }
.readout b { font-size: 15px; font-variant-numeric: tabular-nums; }

/* 保留核心开关组 */
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

.foot { text-align: center; font-size: 11px; color: var(--text-muted); padding: 8px 0 20px; }
</style>
