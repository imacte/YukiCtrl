// src/utils/mock.ts
import yaml from 'js-yaml';

// 完整的 rules.yaml 模拟数据
const mockRules = {
  yumi_scheduler: true,
  dynamic_enabled: true,
  global_mode: "balance",
  fas_rules: {
    fps_gears: [30.0, 60.0, 90.0, 120.0, 144.0],
    fps_margin: "3",
    latency_threshold: "",
    poll_interval_ms: ""
  },
  app_modes: {
    'com.tencent.mm': 'powersave',
    'com.android.chrome': 'fast'
  }
};

// 完整的 config.yaml 模拟数据
const mockConfig = {
  meta: { name: "default_config", author: "yuki", language: "en", loglevel: "INFO" },
  function: { AffinitySetter: false, CpuIdleScalingGovernor: false, EasScheduler: false, cpuset: false, LoadBalancing: false, EnableFeas: false, AdjIOScheduler: false, AppLaunchBoost: false },
  AppLaunchBoostSettings: { BoostRateMs: 200, SmallCoreBoostFreq: "", MediumCoreBoostFreq: "", BigCoreBoostFreq: "", SuperBigCoreBoostFreq: "" },
  CoreAllocation: { CpuSetCore: "4-7" },
  CoreFramework: { SmallCorePath: 0, MediumCorePath: 4, BigCorePath: 7, SuperBigCorePath: -1 },
  IO_Settings: { Scheduler: "", IO_optimization: true },
  CompletelyFairSchedulerValue: { sched_child_runs_first: "", sched_rt_period_us: "", sched_rt_runtime_us: "" },
  CpuIdle: { current_governor: "" },
  Cpuset: { top_app: "0-7", foreground: "0-7", restricted: "0-5", system_background: "1-2", background: "0-2" },
  Bus_dcvs_Path: { CPUllccminPath: "", CPUllccmaxPath: "", CPUddrminPath: "", CPUddrmaxPath: "" },
  pGovPath: {
    schedutil: { path1: "", path2: "", path3: "" },
    walt: { path1: "", path2: "", path3: "" }
  },
  powersave: {
    Governor: { global: "schedutil", SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
    Freq: { SmallCoreMinFreq: "min", SmallCoreMaxFreq: 1500000, MediumCoreMinFreq: "min", MediumCoreMaxFreq: 1800000, BigCoreMinFreq: "min", BigCoreMaxFreq: 1700000, SuperBigCoreMinFreq: "min", SuperBigCoreMaxFreq: 1700000 },
    Uclamp: { UclampTopAppMin: "0", UclampTopAppMax: "100", UclampTopApplatency_sensitive: "0", UclampForeGroundMin: "0", UclampForeGroundMax: "70", UclampBackGroundMin: "0", UclampBackGroundMax: "50" },
    Bus_dcvs: { CPUllccmin: "", CPUllccmax: "", CPUddrmin: "", CPUddrmax: "" },
    Govsets: {
      schedutil: {
        path1: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
        path2: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
        path3: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" }
      }
    },
    Other: { ufsClkGate: false }
  },
  balance: {
    Governor: { global: "schedutil", SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
    Freq: { SmallCoreMinFreq: "min", SmallCoreMaxFreq: 1500000, MediumCoreMinFreq: "min", MediumCoreMaxFreq: 2000000, BigCoreMinFreq: "min", BigCoreMaxFreq: 2300000, SuperBigCoreMinFreq: "min", SuperBigCoreMaxFreq: 2300000 },
    Uclamp: { UclampTopAppMin: "0", UclampTopAppMax: "100", UclampTopApplatency_sensitive: "0", UclampForeGroundMin: "0", UclampForeGroundMax: "70", UclampBackGroundMin: "0", UclampBackGroundMax: "50" },
    Bus_dcvs: { CPUllccmin: "", CPUllccmax: "", CPUddrmin: "", CPUddrmax: "" },
    Govsets: {
      schedutil: {
        path1: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
        path2: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
        path3: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" }
      }
    },
    Other: { ufsClkGate: false }
  },
  performance: {
    Governor: { global: "schedutil", SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
    Freq: { SmallCoreMinFreq: "min", SmallCoreMaxFreq: 1800000, MediumCoreMinFreq: "min", MediumCoreMaxFreq: 2500000, BigCoreMinFreq: "min", BigCoreMaxFreq: 3000000, SuperBigCoreMinFreq: "min", SuperBigCoreMaxFreq: 3000000 },
    Uclamp: { UclampTopAppMin: "0", UclampTopAppMax: "100", UclampTopApplatency_sensitive: "1", UclampForeGroundMin: "0", UclampForeGroundMax: "80", UclampBackGroundMin: "0", UclampBackGroundMax: "50" },
    Bus_dcvs: { CPUllccmin: "", CPUllccmax: "", CPUddrmin: "", CPUddrmax: "" },
    Govsets: {
      schedutil: {
        path1: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
        path2: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
        path3: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" }
      }
    },
    Other: { ufsClkGate: false }
  },
  fast: {
    Governor: { global: "schedutil", SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
    Freq: { SmallCoreMinFreq: "max", SmallCoreMaxFreq: "max", MediumCoreMinFreq: "max", MediumCoreMaxFreq: "max", BigCoreMinFreq: "max", BigCoreMaxFreq: "max", SuperBigCoreMinFreq: "max", SuperBigCoreMaxFreq: "max" },
    Uclamp: { UclampTopAppMin: "10", UclampTopAppMax: "100", UclampTopApplatency_sensitive: "1", UclampForeGroundMin: "0", UclampForeGroundMax: "80", UclampBackGroundMin: "0", UclampBackGroundMax: "50" },
    Bus_dcvs: { CPUllccmin: "", CPUllccmax: "", CPUddrmin: "", CPUddrmax: "" },
    Govsets: {
      schedutil: {
        path1: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
        path2: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" },
        path3: { SmallCore: "", MediumCore: "", BigCore: "", SuperBigCore: "" }
      }
    },
    Other: { ufsClkGate: true }
  }
};

const mockApps = ['com.android.chrome', 'com.tencent.mm', 'com.google.android.youtube', 'com.yuki.controller'];

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
let simulatedModeTxt = "balance";

// ===== 模拟 CPU cpufreq sysfs 数据 =====
// 模拟一个典型骁龙平台：policy0=小核, policy4=中核, policy7=大核
const mockCpuPolicies: number[] = [0, 4, 7];

const mockFreqsMap: Record<number, string[]> = {
  0: ['300000', '576000', '768000', '1017600', '1248000', '1324800', '1497600', '1612800', '1708800', '1804800'],
  4: ['710400', '921600', '1075200', '1228800', '1382400', '1536000', '1651200', '1804800', '1920000', '2073600', '2169600', '2265600'],
  7: ['844800', '1075200', '1190400', '1305600', '1478400', '1555200', '1708800', '1862400', '2035200', '2169600', '2304000', '2400000'],
};

const mockGovernors: string[] = ['schedutil', 'performance', 'powersave', 'userspace', 'ondemand'];

export const MockBridge = {
  async isDaemonRunning(): Promise<boolean> { await delay(100); return true; },
  async getCurrentMode(): Promise<string> { await delay(200); return simulatedModeTxt; },
  async setMode(mode: string): Promise<void> { 
    await delay(200); mockRules.global_mode = mode; 
    setTimeout(() => { simulatedModeTxt = mode; }, 800);
  },
  async getInstalledApps(): Promise<string[]> { await delay(500); return mockApps; },
  async getAppRules(): Promise<Record<string, string>> { await delay(300); return mockRules.app_modes; },
  async saveAppRule(pkg: string, mode: string): Promise<void> { await delay(200); mockRules.app_modes[pkg as keyof typeof mockRules.app_modes] = mode; },
  async getRulesConfig(): Promise<any> { await delay(300); return JSON.parse(JSON.stringify(mockRules)); },
  async saveRulesConfig(config: any): Promise<void> { await delay(400); Object.assign(mockRules, config); },
  async getMainConfig(): Promise<any> { await delay(300); return JSON.parse(JSON.stringify(mockConfig)); },
  async saveMainConfig(config: any): Promise<void> { await delay(400); Object.assign(mockConfig, config); },
  
  async getDaemonLog(): Promise<string> {
    await delay(300);
    return `[2026-02-23 02:31:07] [INFO] [yumi] yumi-module 统一启动中...
[2026-02-23 02:31:07] [INFO] [yumi::scheduler] 应用启动加速 (AppLaunchBoost) 线程已创建
[2026-02-23 02:31:07] [INFO] [yumi::monitor] 正在启动 yumo-monitor 模块...
[2026-02-23 02:48:18] [INFO] [yumi::scheduler] Entered FAS mode, FAS controller is now taking over CPU frequencies.`;
  },

  // ===== 新增：模拟 CPU cpufreq sysfs 读取 =====

  async getCpuPolicies(): Promise<number[]> {
    await delay(100);
    return [...mockCpuPolicies];
  },

  async getAvailableFreqs(policyNum: number): Promise<string[]> {
    await delay(80);
    return mockFreqsMap[policyNum] ? [...mockFreqsMap[policyNum]] : [];
  },

  async getAvailableGovernors(policyNum: number): Promise<string[]> {
    await delay(80);
    // 所有 policy 的调速器一般相同
    return [...mockGovernors];
  }
};