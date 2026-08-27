// tests/percpu_state.rs
//
// Phase 1 / Ticket-03 RED tests for per-CPU state split.
//
// Seam A: CpuState 独立 PID — 每个 CpuState 拥有自己的 P/I/D 累积和频率,
// 两个 CpuState 实例之间不应互相影响.
//
// 这些测试在 RED 阶段应该失败, 因为 src/scheduler/cpu_load_governor.rs
// 当前只有 ClusterState (per-policy), 还没有 CpuState (per-cpu).

use core-pilot::scheduler::cpu_load_governor::{CpuState, CpuPidConfig, CpuStateError};

/// 构造一个最小可用的 CpuPidConfig 用于测试.
/// 频率表必须单调递增 (CPU 驱动的实际可用频率表都是这样).
fn test_config() -> CpuPidConfig {
    CpuPidConfig {
        cpu_id: 0,
        available_freqs_khz: vec![300_000, 600_000, 1_200_000, 1_800_000, 2_400_000, 3_000_000],
        min_freq_khz: 300_000,
        max_freq_khz: 3_000_000,
        target_util_pct: 70.0,
        pid_p: 0.5,
        pid_i: 0.1,
        pid_d: 0.05,
        hysteresis_khz: 100_000,
    }
}

/// 测试 1: 两个 CpuState 实例彼此独立.
///
/// 同一 util 输入, 两个 cpu 应该得到相同 freq (因为 PID 算法相同).
/// 不同 util 输入, 两个 cpu 应该得到不同 freq (这是核心需求).
#[test]
fn two_cpu_states_are_independent() {
    let mut cpu0 = CpuState::new(test_config()).expect("cpu0 init");
    let mut cpu5 = CpuState::new(CpuPidConfig {
        cpu_id: 5,
        ..test_config()
    })
    .expect("cpu5 init");

    // cpu0 喂高 util (90%) — 应该升频
    let freq0_after_high = cpu0.step(90.0);
    // cpu5 喂低 util (10%) — 应该降频
    let freq5_after_low = cpu5.step(10.0);

    assert!(
        freq0_after_high > freq5_after_low,
        "cpu0 (util=90%) freq {} should be > cpu5 (util=10%) freq {}",
        freq0_after_high,
        freq5_after_low
    );

    // 再喂一次, 验证 cpu0 的 PID 累积没有"漏"到 cpu5
    let freq0_again = cpu0.step(90.0);
    let freq5_again = cpu5.step(10.0);
    assert_eq!(freq0_after_high, freq0_again, "cpu0 freq should not jump");
    assert_eq!(freq5_after_low, freq5_again, "cpu5 freq should not jump");
}

/// 测试 2: CpuState 不会跨越 min/max 区间.
///
/// 即使用户传极端 util (0% 或 100%), freq 必须在 [min_freq, max_freq] 内.
#[test]
fn cpu_state_clamps_to_user_range() {
    let mut cpu = CpuState::new(test_config()).expect("cpu init");

    for _ in 0..10 {
        let f = cpu.step(100.0);
        assert!(f <= 3_000_000, "freq {} exceeded max 3_000_000", f);
        assert!(f >= 300_000, "freq {} below min 300_000", f);
    }

    for _ in 0..10 {
        let f = cpu.step(0.0);
        assert!(f <= 3_000_000, "freq {} exceeded max 3_000_000", f);
        assert!(f >= 300_000, "freq {} below min 300_000", f);
    }
}

/// 测试 3: CpuState 持有自己的 P/I/D 累积变量, reset() 后回到初始行为.
///
/// 这是独立性的关键 — 累积项 integral 不能在 reset() 后还残留.
#[test]
fn cpu_state_reset_clears_integral() {
    let mut cpu = CpuState::new(test_config()).expect("cpu init");

    // 跑几轮, 让积分项累积
    for _ in 0..20 {
        cpu.step(95.0);
    }
    let freq_before_reset = cpu.current_freq();

    cpu.reset();
    let freq_after_reset = cpu.current_freq();

    assert_eq!(
        freq_after_reset, 300_000,
        "after reset, freq must be at min {} (got {})",
        300_000, freq_after_reset
    );
    assert_ne!(
        freq_before_reset, freq_after_reset,
        "reset should change freq from {} to {}",
        freq_before_reset, freq_after_reset
    );
}

/// 测试 4: CpuState 返回 freq 必须在 available_freqs_khz 内 (不允许插值到非法频率).
#[test]
fn cpu_state_picks_only_available_freqs() {
    let mut cpu = CpuState::new(test_config()).expect("cpu init");
    let allowed: std::collections::HashSet<u32> = test_config()
        .available_freqs_khz
        .iter()
        .copied()
        .collect();

    for util_pct in [10, 30, 50, 70, 85, 95, 100] {
        let f = cpu.step(util_pct as f32);
        assert!(
            allowed.contains(&f),
            "freq {} (util={}) not in allowed set {:?}",
            f,
            util_pct,
            allowed
        );
    }
}

/// 测试 5: 错误的 config 应该报错而不是 panic.
#[test]
fn invalid_config_returns_error() {
    // min > max 是非法配置
    let bad = CpuPidConfig {
        cpu_id: 0,
        available_freqs_khz: vec![1_000_000, 2_000_000],
        min_freq_khz: 2_000_000,
        max_freq_khz: 1_000_000,
        target_util_pct: 70.0,
        pid_p: 0.5,
        pid_i: 0.1,
        pid_d: 0.05,
        hysteresis_khz: 100_000,
    };
    let r = CpuState::new(bad);
    assert!(matches!(r, Err(CpuStateError::InvalidConfig(_))));
}