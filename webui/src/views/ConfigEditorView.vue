<!--
  src/views/ConfigEditorView.vue

  \u8c03\u5ea6\u914d\u7f6e\u9875\u9762 (\u6697\u8272)
  - \u4eae\u5c4f/\u606f\u5c4f\u5207\u6362
  - \u5361\u7247\u5f0f\u5206\u7ec4: CPU / GPU / IO / Swap / \u70ed\u63d2\u62d4 / \u5e27\u5e73\u6ed1 / \u6e29\u5ea6
  - \u6bcf\u9879: slider + \u6570\u5b57 + HelpTooltip
-->
<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import HelpTooltip from '@/components/HelpTooltip.vue'
import { Bridge } from '@/utils/bridge'

const { t } = useI18n()

type Profile = 'screen_on' | 'screen_off'
const profile = ref<Profile>('screen_on')

// \u8c03\u5ea6\u53c2\u6570\u578b\u5b9a\u4e49
interface SchedulerConfig {
  // CPU
  cpu_min_freq: number
  cpu_max_freq: number
  cpu_governor: string
  // GPU
  gpu_min_freq: number
  gpu_max_freq: number
  gpu_governor: string
  // IO
  io_scheduler: string
  io_read_ahead_kb: number
  // Swap
  vm_swappiness: number
  vm_dirty_ratio: number
  // Hotplug
  hotplug_off_idle_pct: number
  hotplug_on_util_pct: number
  // Frame smoothing
  frame_target_fps: number
  // Temp
  thermal_max_c: number
}

const defaults: SchedulerConfig = {
  cpu_min_freq: 0, cpu_max_freq: 0, cpu_governor: 'schedutil',
  gpu_min_freq: 0, gpu_max_freq: 0, gpu_governor: 'performance',
  io_scheduler: 'cfq', io_read_ahead_kb: 128,
  vm_swappiness: 100, vm_dirty_ratio: 20,
  hotplug_off_idle_pct: 95, hotplug_on_util_pct: 30,
  frame_target_fps: 60,
  thermal_max_c: 70,
}

const screenOnCfg = ref<SchedulerConfig>({ ...defaults })
const screenOffCfg = ref<SchedulerConfig>({ ...defaults })

const currentCfg = computed(() => profile.value === 'screen_on' ? screenOnCfg : screenOffCfg)

const loading = ref(false)
const saveMsg = ref('')
const errorMsg = ref('')

const loadData = async () => {
  loading.value = true
  errorMsg.value = ''
  try {
    const raw = await Bridge.getMainConfig()
    // \u5982\u679c\u540e\u7aef\u6709 profiles.\u5b50\u5b57\u6bb5, \u52a0\u8f7d\u8fd8\u539f\u5408\u5e76
    if (raw && raw.screen_on) Object.assign(screenOnCfg.value, raw.screen_on)
    if (raw && raw.screen_off) Object.assign(screenOffCfg.value, raw.screen_off)
  } catch (e) {
    errorMsg.value = String(e)
  } finally {
    loading.value = false
  }
}

const saveData = async () => {
  loading.value = true
  try {
    await Bridge.saveMainConfig({
      screen_on: { ...screenOnCfg.value },
      screen_off: { ...screenOffCfg.value },
    })
    saveMsg.value = t('core_config_saved') as string
    setTimeout(() => saveMsg.value = '', 1500)
  } catch (e) {
    errorMsg.value = String(e)
  } finally {
    loading.value = false
  }
}

onMounted(loadData)
</script>
<template>
  <div class="config-page">
    <div class="page-header">
      <span class="page-title">{{ t('detailed_config') }}</span>
      <button class="save-btn" :disabled="loading" @click="saveData">\u4fdd\u5b58</button>
    </div>
    <div v-if="errorMsg" class="banner-error">\u26a0\ufe0f {{ errorMsg }}</div>
    <div v-if="saveMsg" class="banner-ok">{{ saveMsg }}</div>
    <div class="profile-switch">
      <button class="seg-btn" :class="{ active: profile === 'screen_on' }" @click="profile = 'screen_on'">
        <van-icon name="eye-o" size="14" />\u4eae\u5c4f\u53c2\u6570
      </button>
      <button class="seg-btn" :class="{ active: profile === 'screen_off' }" @click="profile = 'screen_off'">
        <van-icon name="closed-eye-o" size="14" />\u606f\u5c4f\u53c2\u6570
      </button>
    </div>
    <div class="card">
      <div class="card-title">CPU \u8c03\u5ea6</div>
      <div class="config-row">
        <div class="row-text">
          <span>\u6700\u4f4e\u9891\u7387 (MHz)</span>
          <HelpTooltip text="\u9501\u5b9a CPU \u6700\u4f4e\u8fd0\u884c\u9891\u7387. \u8c03\u9ad8\u53ef\u907f\u514d\u8fc7\u4f4e\u9891\u5d29\u6e83, \u4f46\u8017\u7535\u589e\u52a0." />
        </div>
        <input type="number" v-model.number="currentCfg.cpu_min_freq" class="num-input" />
      </div>
      <div class="config-row">
        <div class="row-text">
          <span>\u6700\u9ad8\u9891\u7387 (MHz)</span>
          <HelpTooltip text="\u9650\u5236 CPU \u6700\u9ad8\u9891\u7387. \u9650\u9891\u53ef\u8282\u7ea6\u7535\u6c60, \u4f46\u9ad8\u8d1f\u8f7d\u4e0b\u4f1a\u5361." />
        </div>
        <input type="number" v-model.number="currentCfg.cpu_max_freq" class="num-input" />
      </div>
      <div class="config-row">
        <div class="row-text">
          <span>\u8c03\u5ea6\u7b56\u7565</span>
          <HelpTooltip text="schedutil: \u8c03\u5ea6\u5668\u9a71\u52a8. ondemand: \u5229\u7528\u7387\u9a71\u52a8. performance: \u7ec8\u9ad8\u9891." />
        </div>
        <select v-model="currentCfg.cpu_governor" class="select-input">
          <option value="schedutil">schedutil</option>
          <option value="ondemand">ondemand</option>
          <option value="performance">performance</option>
          <option value="powersave">powersave</option>
          <option value="conservative">conservative</option>
        </select>
      </div>
    </div>
    <div class="card">
      <div class="card-title">GPU \u8c03\u5ea6</div>
      <div class="config-row">
        <div class="row-text">
          <span>\u6700\u4f4e\u9891\u7387 (MHz)</span>
          <HelpTooltip text="GPU \u6700\u4f4e\u8fd0\u884c\u9891\u7387." />
        </div>
        <input type="number" v-model.number="currentCfg.gpu_min_freq" class="num-input" />
      </div>
      <div class="config-row">
        <div class="row-text">
          <span>\u6700\u9ad8\u9891\u7387 (MHz)</span>
          <HelpTooltip text="GPU \u6700\u9ad8\u8fd0\u884c\u9891\u7387. \u9650\u5236\u6700\u9ad8\u9891\u53ef\u8282\u7701\u7535." />
        </div>
        <input type="number" v-model.number="currentCfg.gpu_max_freq" class="num-input" />
      </div>
      <div class="config-row">
        <div class="row-text">
          <span>\u8c03\u5ea6\u7b56\u7565</span>
          <HelpTooltip text="GPU \u8c03\u9891\u7b56\u7565. performance: \u7ec8\u9ad8\u9891. userspace: \u624b\u52a8\u8c03." />
        </div>
        <select v-model="currentCfg.gpu_governor" class="select-input">
          <option value="performance">performance</option>
          <option value="userspace">userspace</option>
          <option value="simple_ondemand">simple_ondemand</option>
        </select>
      </div>
    </div>
    <div class="card">
      <div class="card-title">IO \u8c03\u5ea6</div>
      <div class="config-row">
        <div class="row-text">
          <span>\u8c03\u5ea6\u7b97\u6cd5</span>
          <HelpTooltip text="cfq: \u516c\u5e73\u961f\u5217. mq-deadline: \u5ef6\u8fdf. bfq: \u54cd\u5e94\u4f18\u5148." />
        </div>
        <select v-model="currentCfg.io_scheduler" class="select-input">
          <option value="cfq">cfq</option>
          <option value="mq-deadline">mq-deadline</option>
          <option value="bfq">bfq</option>
          <option value="noop">noop</option>
        </select>
      </div>
      <div class="config-row">
        <div class="row-text">
          <span>\u9884\u8bfb (KB)</span>
          <HelpTooltip text="\u9884\u8bfb\u53c2\u6570. \u589e\u5927\u53ef\u63d0\u5347\u987a\u5e8f\u8bfb, \u4f46\u4e32\u884c\u8bfb\u53cd\u800c\u53d8\u6162." />
        </div>
        <input type="number" v-model.number="currentCfg.io_read_ahead_kb" class="num-input" />
      </div>
    </div>
    <div class="card">
      <div class="card-title">Swap / VM</div>
      <div class="config-row">
        <div class="row-text">
          <span>swappiness</span>
          <HelpTooltip text="\u5185\u5b58\u4f18\u5148\u7ea7. \u9ad8 = \u4f18\u5148 swap. \u4f4e = \u54cd\u5e94\u5feb." />
        </div>
        <input type="range" min="0" max="200" v-model.number="currentCfg.vm_swappiness" class="range" />
        <span class="val">{{ currentCfg.vm_swappiness }}</span>
      </div>
      <div class="config-row">
        <div class="row-text">
          <span>dirty_ratio</span>
          <HelpTooltip text="\u9875\u9762\u53d1\u751f\u6700\u5927\u767e\u5206\u6bd4." />
        </div>
        <input type="range" min="1" max="90" v-model.number="currentCfg.vm_dirty_ratio" class="range" />
        <span class="val">{{ currentCfg.vm_dirty_ratio }}</span>
      </div>
    </div>
    <div class="card">
      <div class="card-title">\u5e27\u5e73\u6ed1</div>
      <div class="config-row">
        <div class="row-text">
          <span>\u76ee\u6807\u5e27\u7387</span>
          <HelpTooltip text="\u671f\u671b\u4fdd\u6301\u7684\u5237\u65b0\u7387." />
        </div>
        <select v-model.number="currentCfg.frame_target_fps" class="select-input">
          <option :value="30">30</option>
          <option :value="45">45</option>
          <option :value="60">60</option>
          <option :value="90">90</option>
          <option :value="120">120</option>
        </select>
      </div>
    </div>
    <div class="card">
      <div class="card-title">\u70ed\u63d2\u62d4</div>
      <div class="config-row">
        <div class="row-text">
          <span>\u5173\u6838\u9608\u503c (idle %)</span>
        </div>
        <input type="range" min="50" max="100" v-model.number="currentCfg.hotplug_off_idle_pct" class="range" />
        <span class="val">{{ currentCfg.hotplug_off_idle_pct }}%</span>
      </div>
      <div class="config-row">
        <div class="row-text">
          <span>\u5f00\u6838\u9608\u503c (util %)</span>
        </div>
        <input type="range" min="5" max="80" v-model.number="currentCfg.hotplug_on_util_pct" class="range" />
        <span class="val">{{ currentCfg.hotplug_on_util_pct }}%</span>
      </div>
      <router-link to="/hotplug" class="link">\u8fdb\u5165\u70ed\u63d2\u62d4\u9875 \u2192</router-link>
    </div>
    <div class="card">
      <div class="card-title">\u6e29\u5ea6\u9650\u9891</div>
      <div class="config-row">
        <div class="row-text">
          <span>\u9650\u9891\u6e29\u5ea6</span>
          <HelpTooltip text="\u8d85\u8fc7\u8be5\u6e29\u5ea6\u5219\u9650\u9891. 70\u00b0C \u4e3a\u5e38\u89c1\u4e0a\u9650." />
        </div>
        <input type="range" min="40" max="85" v-model.number="currentCfg.thermal_max_c" class="range" />
        <span class="val">{{ currentCfg.thermal_max_c }}\u00b0C</span>
      </div>
    </div>
  </div>
</template>
<style scoped>
.config-page {
  padding: 16px;
  max-width: 600px;
  margin: 0 auto;
}
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 4px 16px;
}
.page-title { font-size: 20px; font-weight: 600; }
.save-btn {
  background: var(--accent);
  color: white;
  border: 0;
  border-radius: 16px;
  padding: 6px 16px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
.save-btn:disabled { opacity: 0.5; }
.save-btn:active { background: var(--accent-hover); }

.banner-error {
  background: rgba(239, 68, 68, 0.15);
  border: 1px solid var(--danger);
  color: var(--danger);
  padding: 8px 12px; border-radius: 8px;
  margin-bottom: 12px; font-size: 12px;
}
.banner-ok {
  background: rgba(16, 185, 129, 0.15);
  border: 1px solid var(--success);
  color: var(--success);
  padding: 8px 12px; border-radius: 8px;
  margin-bottom: 12px; font-size: 12px;
}

.profile-switch {
  display: flex;
  background: var(--bg-card);
  border-radius: 10px;
  padding: 4px;
  margin-bottom: 16px;
  border: 1px solid var(--border);
}
.seg-btn {
  flex: 1;
  background: transparent;
  border: 0;
  padding: 8px;
  border-radius: 8px;
  color: var(--text-muted);
  font-size: 13px;
  cursor: pointer;
  display: flex; align-items: center; justify-content: center; gap: 4px;
}
.seg-btn.active {
  background: var(--accent);
  color: white;
}

.card {
  background: var(--bg-card);
  border-radius: 12px;
  padding: 16px;
  margin-bottom: 12px;
  border: 1px solid var(--border);
}
.card-title {
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 12px;
  display: flex; align-items: center; gap: 6px;
  color: var(--accent);
}
.config-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 0;
  border-top: 1px solid var(--border);
  gap: 8px;
}
.config-row:first-of-type { border-top: 0; }
.row-text {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  font-size: 13px;
  color: var(--text-primary);
}
.num-input, .select-input {
  background: var(--bg-base);
  color: var(--text-primary);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 4px 8px;
  font-size: 13px;
  width: 80px;
  text-align: right;
}
.select-input {
  width: 120px;
  text-align: left;
}
.range {
  flex: 1;
  max-width: 160px;
  -webkit-appearance: none;
  appearance: none;
  height: 4px;
  background: rgba(0,0,0,0.08);
  border-radius: 2px;
  outline: none;
}
.range::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 14px; height: 14px;
  background: var(--accent);
  border-radius: 50%;
  cursor: pointer;
}
.range::-moz-range-thumb {
  width: 14px; height: 14px;
  background: var(--accent);
  border-radius: 50%;
  cursor: pointer;
  border: 0;
}
.val {
  font-size: 12px;
  color: var(--accent);
  font-weight: 600;
  width: 40px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.link {
  display: block;
  margin-top: 8px;
  font-size: 12px;
  color: var(--accent);
}
</style>
