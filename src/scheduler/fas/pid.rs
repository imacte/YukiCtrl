/*
 * Copyright (C) 2026 yuki
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

pub(super) struct PidController {
    // 用户配置的基准系数 (基于 60fps 场景调优)
    pub(super) base_kp: f32, pub(super) base_ki: f32, pub(super) base_kd: f32,
    // 运行时实际使用的动态系数 (根据 target_fps 和场景自动缩放)
    kp: f32, ki: f32, kd: f32,
    integral: f32, prev_error: f32,
    filtered_deriv: f32,
    integral_limit: f32,
    // 缓存当前适配的目标帧率，避免重复计算
    adapted_fps: f32,
}

impl PidController {
    pub(super) fn new(kp: f32, ki: f32, kd: f32) -> Self {
        Self {
            base_kp: kp, base_ki: ki, base_kd: kd,
            kp, ki, kd,
            integral: 0.0, prev_error: 0.0,
            filtered_deriv: 0.0, integral_limit: 0.15,
            adapted_fps: 60.0,
        }
    }

    /// 根据 target_fps 动态缩放 PID 系数
    ///
    /// 核心思想:
    /// 高刷下帧间隔 budget 更短 (144fps → 6.9ms vs 60fps → 16.7ms)，
    /// 同样 1ms 的帧时间偏差在高刷下"严重程度"更高，
    /// 因此 P/I/D 三个通道的增益都需要随 target_fps 缩放，
    /// 但缩放系数不同：P 最激进，D 最保守 (高刷噪声大)。
    pub(super) fn adapt_to_target_fps(&mut self, target_fps: f32) {
        // 防御非法 target_fps（0/负/NaN/Inf），避免 PID 系数与积分限幅被污染
        if !target_fps.is_finite() || target_fps <= 0.0 { return; }
        if (target_fps - self.adapted_fps).abs() < 0.5 { return; }
        self.adapted_fps = target_fps;

        let ratio = target_fps / 60.0;
        // kp: 线性缩放 — 高刷时每 ms 偏差代表更大的帧率损失
        self.kp = self.base_kp * ratio;
        // ki: sqrt 缩放 — 高刷帧多，积分器积累更快，弱化以防过冲
        self.ki = self.base_ki * ratio.sqrt();
        // kd: 保守 0.3 次幂 — 高刷帧间噪声更大，微分项放大噪声
        self.kd = self.base_kd * ratio.powf(0.3);

        // 积分限幅：高刷下缩小，防止积分器饱和导致频率虚高
        self.integral_limit = 0.15 * (60.0 / target_fps.max(1.0)).sqrt();
        // 不 reset 积分器（保持连续性），只做 clamp
        self.integral = self.integral.clamp(-self.integral_limit, self.integral_limit);
    }

    /// 带综合压力指数感知的 PID 计算 (Phase 2 / ticket-06)
    ///
    /// 主输入 `pressure_norm` ∈ [0.0, 1.0], 来自八路感知合成的综合压力指数 / 100.
    /// 当 `pressure_norm` 可用 (> 0.01, 表明八路感知已初始化) → PID 的 P 项增益
    /// 由此驱动:
    ///   - pressure_norm ∈ (0.01, 0.30) → 系统压力低 (GPU/IO/memory bound),
    ///     衰减 P 项, 避免无效拉频
    ///   - pressure_norm ∈ [0.30, 1.0] → 系统繁忙, 正常增益
    ///
    /// `fg_util` 仅作为 fallback: 当 `pressure_norm <= 0.01` (八路感知未启动
    /// 或 compute_pressure_index 返回 0) → 自动退回原 fg_util < 0.30 衰减逻辑,
    /// 保证早期启动期间 PID 仍能工作.
    ///
    /// 这个替换不会改变 PID 的 error / integral / derivative 计算, 只会修改
    /// P 项的 util_gain 系数. 既保留了原 CPU bound 全力拉频语义, 又额外识别了
    /// GPU bound / IO bound 等其他瓶颈场景.
    pub(super) fn compute(
        &mut self,
        error: f32,
        inst_error: f32,
        norm: f32,
        fg_util: f32,
        pressure_norm: f32,
    ) -> f32 {
        let safe_norm = norm.clamp(0.5, 2.5);

        if error < 0.0 {
            self.integral += error * safe_norm;
        } else {
            let leak = (0.70 + safe_norm * 0.08).clamp(0.70, 0.85);
            self.integral *= leak;
        }
        let dyn_limit = self.integral_limit * safe_norm.clamp(0.7, 1.3);
        self.integral = self.integral.clamp(-dyn_limit, dyn_limit);

        let raw_deriv = (error - self.prev_error) / safe_norm;
        // 动态低通滤波: 高刷下帧间微小抖动在微秒级被放大,
        // alpha 随 target_fps 升高而降低: 60fps=0.30, 120fps=0.21, 144fps=0.19
        let d_alpha = (0.30 * (60.0 / self.adapted_fps.max(1.0)).sqrt()).clamp(0.10, 0.30);
        self.filtered_deriv = self.filtered_deriv * (1.0 - d_alpha) + raw_deriv * d_alpha;
        self.prev_error = error;

        // 增益调制 (Phase 2 / ticket-06 改造)
        //
        // 优先级: pressure_norm > fg_util (fallback)
        //
        // 情形 1: pressure_norm > 0.01 → 八路感知在线, 用综合压力指数判断
        //   - pressure_norm ∈ (0.01, 0.30) → 系统压力低, 衰减 P 项
        //     (例: GPU bound 但 CPU 闲, 拉 CPU 频率救不了帧率, 白给功耗)
        //   - 否则 → 1.0 (全力拉频)
        // 情形 2: pressure_norm <= 0.01 → 感知未启动, 退回原 fg_util 逻辑
        //   - fg_util ∈ (0.01, 0.30) → CPU 闲, 衰减 P 项
        //   - 否则 → 1.0
        //
        // 即使压力低 (gain < 1.0) PID 也保留最低 0.30 的 gain,
        // 不会完全压死, 这样短促的 jank 仍能及时拉频.
        let util_gain: f32 = if pressure_norm > 0.01 {
            if pressure_norm < 0.30 {
                0.3 + pressure_norm * 2.3  // 0.3 ~ 0.99
            } else {
                1.0
            }
        } else if fg_util > 0.01 && fg_util < 0.30 {
            0.3 + fg_util * 2.3  // fallback: 与原行为一致
        } else {
            1.0
        };

        let p_term = self.kp * inst_error * util_gain;
        let i_term = self.ki * self.integral;
        let d_term = self.kd * self.filtered_deriv;

        p_term + i_term + d_term
    }

    pub(super) fn reset(&mut self) {
        self.integral = 0.0; self.prev_error = 0.0; self.filtered_deriv = 0.0;
    }

    pub(super) fn update_coefficients(&mut self, kp: f32, ki: f32, kd: f32) {
        self.base_kp = kp; self.base_ki = ki; self.base_kd = kd;
        // 重新按当前 adapted_fps 缩放
        let fps = self.adapted_fps;
        self.adapted_fps = 0.0; // 强制刷新
        self.adapt_to_target_fps(fps);
        self.reset();
    }
}

// ════════════════════════════════════════════════════════════════
//  工具函数
// ════════════════════════════════════════════════════════════════

#[inline]
pub(super) fn fps_norm(target_fps: f32) -> f32 {
    (60.0 / target_fps.max(1.0)).sqrt()
}

#[inline]
pub(super) fn scale_frames(base: u32, target_fps: f32) -> u32 {
    ((base as f32 * target_fps / 60.0).max(base as f32 * 0.4)) as u32
}

// ════════════════════════════════════════════════════════════════
//  Phase 2 / ticket-06 单元测试
// ════════════════════════════════════════════════════════════════
//
// 验证 PID 的 P 项增益正确切到"综合压力指数"通道:
// 1. GPU 负载高 (但 CPU util 低) → PID 应基于压力指数仍能拉高输出
// 2. 所有感知可用时 → 压力指数直接决定 P 项 (不是 fg_util)
// 3. 压力指数不可用 (≤ 0.01) → 自动回退到 fg_util (向后兼容)

#[cfg(test)]
mod pressure_aware_tests {
    use super::*;

    fn make_pid() -> PidController {
        // kp 选个明显能体现差异的值
        let mut p = PidController::new(0.5, 0.0, 0.0);
        p.adapt_to_target_fps(60.0);
        p
    }

    /// 场景 1: GPU bound (CPU util 低但压力高) 时 PID 输出应高于纯 CPU 模式.
    ///
    /// `error=-1.0` (帧晚 1ms), `inst_error=-1.0` (本帧也晚), norm=1.0.
    /// CPU 闲 (fg_util=0.10) 但 GPU 压力大 (pressure_norm=0.85).
    /// 旧逻辑: util_gain = 0.3 + 0.10*2.3 = 0.53 → P = 0.5 * (-1.0) * 0.53 = -0.265
    /// 新逻辑: pressure_norm=0.85 > 0.30 → util_gain = 1.0 → P = -0.5
    /// 差值 |0.235|, 新逻辑输出更"猛" (绝对值更大), 这正是 GPU bound 应有的行为:
    /// 既然压力大, PID 应当积极响应, 不该被"CPU 闲"误导.
    #[test]
    fn pid_uses_pressure_when_cpu_idle_but_gpu_busy() {
        let mut pid_a = make_pid();  // 旧逻辑: 仅 fg_util=0.10
        let mut pid_b = make_pid();  // 新逻辑: pressure_norm=0.85

        let err = -1.0_f32;
        let inst = -1.0_f32;
        let norm = 1.0_f32;
        let fg_util = 0.10_f32;

        let out_old = pid_a.compute(err, inst, norm, fg_util, 0.0);
        let out_new = pid_b.compute(err, inst, norm, fg_util, 0.85);

        assert!(out_new.abs() > out_old.abs(),
            "压力指数驱动应比 fg_util 驱动更强: new={} old={}",
            out_new, out_old);
        // 新输出应该是 kp * inst * 1.0 = 0.5 * (-1.0) * 1.0 = -0.5
        assert!((out_new - (-0.5)).abs() < 0.001, "got {out_new}");
        // 旧输出应该是 kp * inst * 0.53 = -0.265
        assert!((out_old - (-0.265)).abs() < 0.01, "got {out_old}");
    }

    /// 场景 2: 所有感知数据可用 → PID 输入等于综合压力指数 (而非 fg_util).
    ///
    /// 给一个"高 fg_util 但低压力指数"的场景, 验证 PID 用的是压力指数.
    /// 例: CPU util=0.90 但 GPU 极闲 (gpu 0, io 0, mem 0, frame false) →
    /// pressure_norm = 0.90 * (0.40 / 0.65) = 0.554 > 0.30 → 增益 1.0.
    /// 压力指数确实"接管"了 fg_util 通道.
    #[test]
    fn pid_uses_pressure_norm_not_fg_util_when_both_available() {
        let mut pid_a = make_pid();
        let mut pid_b = make_pid();
        let err = -1.0_f32;
        let inst = -1.0_f32;
        let norm = 1.0_f32;

        // fg_util=0.10 旧逻辑会衰减 (gain=0.53), 但 pressure_norm=0.85 不衰减
        let out_low_pressure = pid_a.compute(err, inst, norm, 0.10, 0.85);
        // 反向: fg_util=0.90 旧逻辑不衰减 (gain=1.0), 但 pressure_norm=0.10 衰减
        let out_high_cpu_low_pressure = pid_b.compute(err, inst, norm, 0.90, 0.10);

        // 两者应都约 -0.5 (gain=1.0), 即压力指数覆盖了 fg_util 的影响
        assert!((out_low_pressure - (-0.5)).abs() < 0.001,
            "压力指数 0.85 应覆盖低 fg_util: got {out_low_pressure}");
        // 第二个: pressure_norm=0.10 → gain = 0.3 + 0.10*2.3 = 0.53 → P=-0.265
        assert!((out_high_cpu_low_pressure - (-0.265)).abs() < 0.01,
            "压力指数 0.10 应覆盖高 fg_util: got {out_high_cpu_low_pressure}");
    }

    /// 场景 3: 压力指数不可用 (≤ 0.01, 八路感知未启动) → 自动回退到 fg_util.
    ///
    /// 验证向后兼容: 旧调用方传 pressure_norm=0.0 时, 行为与原版完全一致.
    #[test]
    fn pid_falls_back_to_fg_util_when_pressure_unavailable() {
        let mut pid_old_only = make_pid();
        let mut pid_legacy = make_pid();
        let err = -1.0_f32;
        let inst = -1.0_f32;
        let norm = 1.0_f32;
        let fg_util = 0.10_f32;

        // pressure_norm=0.0 → 应当走 fallback, 行为与旧 compute 一致
        let out = pid_legacy.compute(err, inst, norm, fg_util, 0.0);
        // 直接调用旧公式 (在测试里模拟)
        let legacy_gain = if fg_util > 0.01 && fg_util < 0.30 {
            0.3 + fg_util * 2.3
        } else {
            1.0
        };
        let legacy_out = 0.5 * inst * legacy_gain;
        assert!((out - legacy_out).abs() < 0.001,
            "fallback 应等同旧逻辑: got {out} legacy {legacy_out}");

        // 与"无 fallback 路径"对比: pressure_norm=0.0 + fg_util=0.10 → gain=0.53
        // 但 fg_util=0.90 (CPU 忙) → gain=1.0
        let _ = pid_old_only.compute(err, inst, norm, 0.90, 0.0);
        let out_cpu_busy = pid_old_only.compute(err, inst, norm, 0.90, 0.0);
        assert!((out_cpu_busy - (-0.5)).abs() < 0.001,
            "压力不可用 + CPU 忙 → gain 1.0: got {out_cpu_busy}");
    }

    /// 场景 4: 压力指数边界值 — 0.30 应当是 gain 从 0.99 跳到 1.0 的阈值.
    #[test]
    fn pid_pressure_threshold_at_0_30() {
        let mut p_below = make_pid();
        let mut p_at = make_pid();
        let mut p_above = make_pid();
        let err = -1.0_f32;
        let inst = -1.0_f32;
        let norm = 1.0_f32;

        // 0.20 < 0.30 → gain ≈ 0.76
        let out_below = p_below.compute(err, inst, norm, 0.5, 0.20);
        // 0.30 → 不在 (< 0.30) 分支 → gain = 1.0
        let out_at = p_at.compute(err, inst, norm, 0.5, 0.30);
        // 0.50 → gain = 1.0
        let out_above = p_above.compute(err, inst, norm, 0.5, 0.50);

        assert!(out_at.abs() > out_below.abs(), "0.30 应比 0.20 更强");
        assert!((out_at - out_above).abs() < 0.001, "0.30 与 0.50 应等价");
        assert!((out_at - (-0.5)).abs() < 0.001, "got {out_at}");
    }
}
