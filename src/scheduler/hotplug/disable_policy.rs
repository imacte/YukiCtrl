/*
 * Copyright (C) 2026 yuki
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

//! 关核策略 (hyperos 关核逻辑融合 - ticket)
//! 移植自 hyperos_power_tuner/lib/apply_cpu.sh _hotplug_off 的秒级延迟策略
//! (SO_HOTPLUG_DELAY_7=5, SO_HOTPLUG_DELAY_6=8), 以及 safe_write 失败时的
//! FreqFloor 降级写 scaling_max_freq 伪关核 (类似 hyperos takeover_apply Off).

/// 大核 cpu7 关核后, 再次 disable 需在线 >= 5s
pub const MIN_OFFLINE_DURATION_SEC_CPU7: i64 = 5;
/// 大核 cpu6 关核后, 再次 disable 需在线 >= 8s
pub const MIN_OFFLINE_DURATION_SEC_CPU6: i64 = 8;
/// 不适用延迟关核的 CPU (小核 A510/A55) 默认值
pub const DEFAULT_MIN_OFFLINE_DURATION_SEC: i64 = 0;

/// 该 CPU 所需的最小在线时长(秒) (hyperos 策略)
pub fn min_offline_duration_sec(cpu_id: u32) -> i64 {
    match cpu_id {
        7 => MIN_OFFLINE_DURATION_SEC_CPU7,
        6 => MIN_OFFLINE_DURATION_SEC_CPU6,
        _ => DEFAULT_MIN_OFFLINE_DURATION_SEC,
    }
}

/// 返回 true 表示应跳过本轮 disable (cpu 在线时长不足)
pub fn should_skip_disable_for_min_duration(
    cpu_id: u32,
    last_enable_unix_ms: i64,
    now_unix_ms: i64,
) -> bool {
    let needed_ms = min_offline_duration_sec(cpu_id) * 1000;
    if needed_ms <= 0 {
        return false;
    }
    if last_enable_unix_ms == 0 {
        return false;
    }
    now_unix_ms - last_enable_unix_ms < needed_ms
}

/// 决定 apply_disable 的下一步动作
#[derive(Debug, PartialEq, Eq)]
pub enum DisableOutcome {
    SkippedMinDuration,
    WriteOnline,
    FreqFloorFallback,
}

pub fn decide_disable(
    cpu_id: u32,
    last_enable_unix_ms: i64,
    now_unix_ms: i64,
    online_write_succeeded: bool,
) -> DisableOutcome {
    if should_skip_disable_for_min_duration(cpu_id, last_enable_unix_ms, now_unix_ms) {
        return DisableOutcome::SkippedMinDuration;
    }
    if online_write_succeeded {
        DisableOutcome::WriteOnline
    } else {
        DisableOutcome::FreqFloorFallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_duration_per_cpu() {
        assert_eq!(min_offline_duration_sec(7), 5);
        assert_eq!(min_offline_duration_sec(6), 8);
        assert_eq!(min_offline_duration_sec(5), 0);
        assert_eq!(min_offline_duration_sec(0), 0);
        assert_eq!(min_offline_duration_sec(2), 0);
    }

    #[test]
    fn skip_when_cpu7_enabled_4s_ago() {
        let now = 10_000;
        let last_enable = now - 4_000;
        assert!(should_skip_disable_for_min_duration(7, last_enable, now));
    }

    #[test]
    fn allow_when_cpu7_enabled_6s_ago() {
        let now = 10_000;
        let last_enable = now - 6_000;
        assert!(!should_skip_disable_for_min_duration(7, last_enable, now));
    }

    #[test]
    fn allow_at_exact_threshold() {
        let now = 10_000;
        let last_enable = now - 5_000;
        assert!(!should_skip_disable_for_min_duration(7, last_enable, now));
    }

    #[test]
    fn skip_when_cpu6_enabled_7s_ago() {
        let now = 20_000;
        let last_enable = now - 7_000;
        assert!(should_skip_disable_for_min_duration(6, last_enable, now));
    }

    #[test]
    fn allow_when_cpu6_enabled_9s_ago() {
        let now = 20_000;
        let last_enable = now - 9_000;
        assert!(!should_skip_disable_for_min_duration(6, last_enable, now));
    }

    #[test]
    fn allow_cpu5_never_enabled() {
        assert!(!should_skip_disable_for_min_duration(5, 0, 1_000));
    }

    #[test]
    fn allow_cpu2_never_enabled() {
        assert!(!should_skip_disable_for_min_duration(2, 0, 1_000));
    }

    #[test]
    fn allow_cpu7_never_enabled_recorded() {
        assert!(!should_skip_disable_for_min_duration(7, 0, 1_000));
    }

    #[test]
    fn decide_disable_min_duration_skipped() {
        let now = 10_000;
        let last_enable = now - 1_000;
        let out = decide_disable(7, last_enable, now, false);
        assert_eq!(out, DisableOutcome::SkippedMinDuration);
    }

    #[test]
    fn decide_disable_write_online_success() {
        let now = 10_000;
        let last_enable = now - 6_000;
        let out = decide_disable(7, last_enable, now, true);
        assert_eq!(out, DisableOutcome::WriteOnline);
    }

    #[test]
    fn decide_disable_freq_fallback_when_online_fail() {
        let now = 10_000;
        let last_enable = now - 6_000;
        let out = decide_disable(7, last_enable, now, false);
        assert_eq!(out, DisableOutcome::FreqFloorFallback);
    }

    #[test]
    fn decide_disable_min_duration_wins_over_write_fail() {
        let now = 10_000;
        let last_enable = now - 1_000;
        let out = decide_disable(7, last_enable, now, false);
        assert_eq!(out, DisableOutcome::SkippedMinDuration);
    }

    #[test]
    fn decide_disable_cpu2_no_min_duration() {
        let now = 10_000;
        let last_enable = now - 1_000;
        let out = decide_disable(2, last_enable, now, false);
        assert_eq!(out, DisableOutcome::FreqFloorFallback);
        let out = decide_disable(2, last_enable, now, true);
        assert_eq!(out, DisableOutcome::WriteOnline);
    }
}
