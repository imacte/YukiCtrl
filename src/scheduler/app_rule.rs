/*
 * Copyright (C) 2026 yuki
 *
 * App 规则引擎 (Phase 2 / ticket-07)
 *
 * 目标:
 * - 让用户在 YAML 中按 "前台包名" 配置调度偏置 (针对具体游戏/App 调优)
 * - 偏置维度:
 *   1) target_pressure 偏移 (影响 FAS 综合压力目标)
 *   2) hotplug off_threshold_idle_pct 偏移 (Restrict 更保守关核,
 *      Boost 更激进关核)
 *   3) burst (Jank panic) 禁用开关 (Restrict 禁用, Boost 允许)
 *   4) FAS boost 触发阈值偏移 (Boost 降低触发阈值, 让低帧更易 panic)
 *
 * ⚠️ 生效范围限制 (Phase 2 / ticket-07-fix):
 *   App 规则对 CPU 频率的限制 (`target_util_offset` / `disable_burst` /
 *   `boost_threshold_offset`) **只在 FAS 模式下生效**.
 *   - FAS 模式 (前台 App 触发 mode = "fas"): 完整生效
 *   - 其他模式 (balance / powersave / performance 等, 由 CLG 接管):
 *     App 规则**不**影响 CPU 频率调频, 但 hotplug 关核阈值偏置仍然
 *     生效 (hotplug 与 mode 解耦, 每个 tick 自动重算).
 *
 *   WebUI 在 App 规则管理页面顶部需要给用户提示该限制, 否则用户
 *   可能误以为"规则没生效". 后续如需扩展到其他模式, 需要为 CLG
 *   加一层 AppRule 偏置接口 (类似 apply_app_rule_bias_to_clg),
 *   这是独立 ticket.
 *
 * 设计:
 * - AppRuleEngine 持有从 Config::app_rules 反序列化来的 AppRule 列表
 * - 匹配是 O(n) 线性 (n 一般 < 50), 不值得哈希化
 * - 偏置计算封装成 struct (AppRuleBias), 调用方读字段即可
 *   FAS / hotplug 各自只取自己关心的字段, 互不耦合
 */

use serde::Deserialize;

// ════════════════════════════════════════════════════════════════
//  规则类型与强度
// ════════════════════════════════════════════════════════════════

/// 规则类型: Restrict(限制) 或 Boost(加速)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    /// 限制模式: 调低目标频率, 禁 burst, 关核更保守
    Restrict,
    /// 加速模式: 调高目标频率, 允许 burst, 关核更激进
    Boost,
}

/// 强度档位 (缺省时按 strength 自动算出 max_freq_scale / target_util_offset)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuleStrength {
    /// 轻度: target_pressure ±10, max_freq_scale ±0.05
    Light,
    #[default]
    /// 中度 (默认): target_pressure ±20, max_freq_scale ±0.10
    Medium,
    /// 重度: target_pressure ±35, max_freq_scale ±0.20
    Heavy,
}

// ════════════════════════════════════════════════════════════════
//  规则条目
// ════════════════════════════════════════════════════════════════

/// 单条 App 规则
#[derive(Debug, Clone, Deserialize)]
pub struct AppRule {
    pub package: String,
    pub rule_type: RuleType,
    #[serde(default)]
    pub strength: RuleStrength,

    /// 可选覆盖; None 时按 strength 自动推导
    #[serde(default)]
    pub max_freq_scale: Option<f32>,
    /// 可选覆盖; None 时按 strength 自动推导
    #[serde(default)]
    pub target_util_offset: Option<i32>,

    /// Restrict 时建议 true (禁用 Jank panic); Boost 时建议 false
    #[serde(default)]
    pub disable_burst: bool,
    /// Boost 时建议负值 (例 -5: util 25% 就开核)
    #[serde(default)]
    pub boost_threshold_offset: i32,
}

impl AppRule {
    pub fn default_max_freq_scale(rule_type: RuleType, strength: RuleStrength) -> f32 {
        let mag = match strength {
            RuleStrength::Light => 0.05,
            RuleStrength::Medium => 0.10,
            RuleStrength::Heavy => 0.20,
        };
        match rule_type {
            RuleType::Restrict => 1.0 - mag,
            RuleType::Boost => 1.0 + mag,
        }
    }

    pub fn default_target_util_offset(rule_type: RuleType, strength: RuleStrength) -> i32 {
        let mag: i32 = match strength {
            RuleStrength::Light => 10,
            RuleStrength::Medium => 20,
            RuleStrength::Heavy => 35,
        };
        match rule_type {
            RuleType::Restrict => -mag,
            RuleType::Boost => mag,
        }
    }

    pub fn effective_max_freq_scale(&self) -> f32 {
        self.max_freq_scale.unwrap_or_else(|| {
            Self::default_max_freq_scale(self.rule_type, self.strength)
        })
    }

    pub fn effective_target_util_offset(&self) -> i32 {
        self.target_util_offset.unwrap_or_else(|| {
            Self::default_target_util_offset(self.rule_type, self.strength)
        })
    }
}

// ════════════════════════════════════════════════════════════════
//  规则引擎
// ════════════════════════════════════════════════════════════════

/// App 规则集合 + 匹配方法
#[derive(Debug, Default, Clone)]
pub struct AppRuleEngine {
    pub rules: Vec<AppRule>,
}

impl AppRuleEngine {
    pub fn new(rules: Vec<AppRule>) -> Self {
        Self { rules }
    }

    /// 按 package 精确匹配, 返回首个匹配的规则
    pub fn match_rule(&self, pkg: &str) -> Option<&AppRule> {
        if pkg.is_empty() {
            return None;
        }
        self.rules.iter().find(|r| r.package == pkg)
    }
}

// ════════════════════════════════════════════════════════════════
//  偏置结果 (调用方只读字段, 不耦合 AppRule)
// ════════════════════════════════════════════════════════════════

/// AppRule 偏置结果, 给 FAS / hotplug 各取所需.
///
/// 默认值即"无偏置"状态 (offset=0, scale=1.0, disable_burst=false,
/// hotplug_idle_offset=0.0), 调用方可以直接 `+= bias.target_util_offset`
/// `* bias.max_freq_scale`, 不需要 Option 处理.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AppRuleBias {
    pub target_util_offset: i32,
    pub max_freq_scale: f32,
    pub disable_burst: bool,
    /// Restrict +3 (关核更保守), Boost -3 (关核更激进)
    pub hotplug_idle_offset: f32,
    /// on_threshold_util 偏移 (Boost 时取负, 让低帧易开核)
    pub hotplug_util_offset: f32,
    /// 调试用: 当前偏置由哪个 AppRule 触发.
    /// Phase 2 / ticket-07-fix: 改成 String (原来是 &'static str),
    /// 因为 r.package 生命周期与 AppRule 绑定, 无法跨函数返回 'static.
    pub matched_pkg: Option<String>,
}

impl AppRuleBias {
    pub fn identity() -> Self {
        Self {
            target_util_offset: 0,
            max_freq_scale: 1.0,
            disable_burst: false,
            hotplug_idle_offset: 0.0,
            hotplug_util_offset: 0.0,
            matched_pkg: None,
        }
    }

    /// Restrict 提高 off 阈值 (更保守关核); Boost 降低 off 阈值 (更激进关核).
    /// 偏置量按 strength: Light ±2, Medium ±3, Heavy ±5
    fn hotplug_idle_offset_for(rule_type: RuleType, strength: RuleStrength) -> f32 {
        let mag = match strength {
            RuleStrength::Light => 2.0,
            RuleStrength::Medium => 3.0,
            RuleStrength::Heavy => 5.0,
        };
        match rule_type {
            RuleType::Restrict => mag,
            RuleType::Boost => -mag,
        }
    }

    pub fn from_rule(rule: Option<&AppRule>) -> Self {
        match rule {
            None => Self::identity(),
            Some(r) => Self {
                target_util_offset: r.effective_target_util_offset(),
                max_freq_scale: r.effective_max_freq_scale(),
                disable_burst: r.disable_burst,
                hotplug_idle_offset: Self::hotplug_idle_offset_for(r.rule_type, r.strength),
                hotplug_util_offset: r.boost_threshold_offset as f32,
                matched_pkg: Some(r.package.clone()),
            },
        }
    }
}

// ════════════════════════════════════════════════════════════════
//  单元测试
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(pkg: &str, t: RuleType, s: RuleStrength) -> AppRule {
        AppRule {
            package: pkg.to_string(),
            rule_type: t,
            strength: s,
            max_freq_scale: None,
            target_util_offset: None,
            disable_burst: false,
            boost_threshold_offset: 0,
        }
    }

    #[test]
    fn match_returns_first_matching_rule() {
        let eng = AppRuleEngine::new(vec![
            rule("com.a", RuleType::Boost, RuleStrength::Medium),
            rule("com.b", RuleType::Restrict, RuleStrength::Heavy),
        ]);
        assert_eq!(eng.match_rule("com.a").unwrap().rule_type, RuleType::Boost);
        assert_eq!(eng.match_rule("com.b").unwrap().rule_type, RuleType::Restrict);
        assert!(eng.match_rule("com.c").is_none());
        assert!(eng.match_rule("").is_none());
    }

    #[test]
    fn restrict_lowers_target_and_freq() {
        let r = rule("com.game", RuleType::Restrict, RuleStrength::Medium);
        assert_eq!(r.effective_target_util_offset(), -20);
        assert!((r.effective_max_freq_scale() - 0.90).abs() < 0.001);
    }

    #[test]
    fn boost_raises_target_and_freq() {
        let r = rule("com.game", RuleType::Boost, RuleStrength::Heavy);
        assert_eq!(r.effective_target_util_offset(), 35);
        assert!((r.effective_max_freq_scale() - 1.20).abs() < 0.001);
    }

    #[test]
    fn strength_heavy_bigger_than_light() {
        let light = rule("p", RuleType::Boost, RuleStrength::Light);
        let heavy = rule("p", RuleType::Boost, RuleStrength::Heavy);
        assert!(heavy.effective_target_util_offset().abs()
              > light.effective_target_util_offset().abs());
        assert!(heavy.effective_max_freq_scale() > light.effective_max_freq_scale());
    }

    #[test]
    fn explicit_overrides_win_over_strength_default() {
        let mut r = rule("p", RuleType::Boost, RuleStrength::Medium);
        assert_eq!(r.effective_target_util_offset(), 20);
        r.target_util_offset = Some(50);
        r.max_freq_scale = Some(1.30);
        assert_eq!(r.effective_target_util_offset(), 50);
        assert!((r.effective_max_freq_scale() - 1.30).abs() < 0.001);
    }

    #[test]
    fn bias_identity_when_no_rule() {
        let bias = AppRuleBias::from_rule(None);
        assert_eq!(bias.target_util_offset, 0);
        assert!((bias.max_freq_scale - 1.0).abs() < 0.001);
        assert!(!bias.disable_burst);
        assert_eq!(bias.matched_pkg, None);
    }

    /// Phase 2 / ticket-07-fix: matched_pkg 改成 Option<String>
    /// (原本是 Option<&'static str>, 编译失败).
    /// 验证 from_rule 正确 clone 出 owned String.
    #[test]
    fn matched_pkg_is_owned_string() {
        let r = rule("com.example.app", RuleType::Boost, RuleStrength::Medium);
        let pkg = r.package.clone(); // 与 bias 内部的 pkg 字符串分离
        let bias = AppRuleBias::from_rule(Some(&r));
        // drop r, bias.matched_pkg 不应受影响
        drop(r);
        match bias.matched_pkg {
            Some(ref s) => assert_eq!(s, &pkg),
            None => panic!("expected Some(pkg)"),
        }
    }

    #[test]
    fn restrict_bias_makes_hotplug_more_conservative() {
        let r = rule("com.e", RuleType::Restrict, RuleStrength::Medium);
        let bias = AppRuleBias::from_rule(Some(&r));
        assert!(bias.hotplug_idle_offset > 0.0);
        assert_eq!(bias.hotplug_idle_offset, 3.0);
        assert!(bias.target_util_offset < 0);
    }

    #[test]
    fn boost_bias_makes_hotplug_more_aggressive() {
        let r = rule("com.e", RuleType::Boost, RuleStrength::Medium);
        let bias = AppRuleBias::from_rule(Some(&r));
        assert!(bias.hotplug_idle_offset < 0.0);
        assert_eq!(bias.hotplug_idle_offset, -3.0);
        assert!(bias.target_util_offset > 0);
    }

    #[test]
    fn hotplug_idle_offset_scales_with_strength() {
        let light = AppRuleBias::from_rule(Some(&rule("p", RuleType::Restrict, RuleStrength::Light)));
        let heavy = AppRuleBias::from_rule(Some(&rule("p", RuleType::Restrict, RuleStrength::Heavy)));
        assert_eq!(light.hotplug_idle_offset, 2.0);
        assert_eq!(heavy.hotplug_idle_offset, 5.0);
    }

    #[test]
    fn disable_burst_propagates() {
        let mut r = rule("p", RuleType::Restrict, RuleStrength::Medium);
        r.disable_burst = true;
        let bias = AppRuleBias::from_rule(Some(&r));
        assert!(bias.disable_burst);
    }

    #[test]
    fn boost_threshold_offset_propagates() {
        let mut r = rule("p", RuleType::Boost, RuleStrength::Medium);
        r.boost_threshold_offset = -5;
        let bias = AppRuleBias::from_rule(Some(&r));
        assert!((bias.hotplug_util_offset - (-5.0)).abs() < 0.001);
    }

    #[test]
    fn yaml_deserialize_basic() {
        let yaml = "
- package: com.tencent.tmgp.pubgmhd
  rule_type: boost
  strength: heavy
- package: com.android.settings
  rule_type: restrict
  strength: light
  disable_burst: true
";
        let rules: Vec<AppRule> = serde_yaml::from_str(yaml).expect("yaml parse");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].rule_type, RuleType::Boost);
        assert_eq!(rules[0].strength, RuleStrength::Heavy);
        assert_eq!(rules[1].rule_type, RuleType::Restrict);
        assert!(rules[1].disable_burst);
    }
}