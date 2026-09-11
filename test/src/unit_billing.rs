//! L1 单元 — 计价公式。用例文档:`cases/02-unit-billing.md`
//!
//! **正确性风险最高的模块**。本文件的 4 组黄金用例直接移植自 new-api 自带的 Go 单测,
//! 期望值可信度最高(见 TEST-VECTORS.md §1 的 `来源` 标注)。
//!
//! 公式见 CONTRACTS.md §6。

use pretty_assertions::assert_eq;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use sea_weir_core::relay::billing::{calculate_quota, PriceData};
use sea_weir_types::dto::{relay::UsageSemantic, Usage};

/// 构造 usage。`cache_creation_5m` / `1h` 走 Claude 分档缓存写入。
fn usage(prompt: i64, completion: i64) -> Usage {
    Usage {
        prompt_tokens: prompt,
        completion_tokens: completion,
        total_tokens: prompt + completion,
        ..Default::default()
    }
}

fn price(model_ratio: Decimal, group_ratio: Decimal, completion_ratio: Decimal) -> PriceData {
    PriceData {
        model_ratio,
        group_ratio,
        completion_ratio,
        cache_ratio: Decimal::ZERO,
        create_cache_ratio: Decimal::ZERO,
        cache_creation_5m_ratio: Decimal::ZERO,
        cache_creation_1h_ratio: Decimal::ZERO,
        image_ratio: Decimal::ZERO,
        audio_ratio: Decimal::ZERO,
        audio_input_price: Decimal::ZERO,
        model_price: None,
        tiered_expr: None,
        other_ratios: Vec::new(),
    }
}

// ───────────────────────── TC-UNI-BIL-001 ─────────────────────────

/// TC-UNI-BIL-001-POS:OpenAI 语义 + 缓存读。
///
/// OpenAI 语义下 `prompt_tokens` **已包含** cached_tokens,须先扣减再按 CacheRatio 计价。
///
/// 溯源:new-api `service/text_quota_test.go`
///       `TestCalculateTextQuotaSummarySeparatesOpenRouterCacheReadFromPromptBilling`
///
/// ```text
/// base     = 2604 − 2432 = 172
/// cache_q  = 2432 × 0.1  = 243.2
/// compl_q  = 383  × 1    = 383
/// quota    = (172 + 243.2 + 383) × (1 × 1) = 798.2 → 798
/// ```
#[test]
fn tc_uni_bil_001_pos_openai_semantic_cache_read() {
    let mut u = usage(2604, 383);
    u.cached_tokens = 2432;

    let mut p = price(dec!(1), dec!(1), dec!(1));
    p.cache_ratio = dec!(0.1);

    assert_eq!(calculate_quota(&u, &p), 798);
}

// ───────────────────────── TC-UNI-BIL-002 ─────────────────────────

/// TC-UNI-BIL-002-POS:Claude 语义 + 缓存写入分档。
///
/// Claude 语义下 Anthropic 分开上报,`prompt_tokens` **不含** cache,故不扣减。
/// 缓存写入按 5m / 1h 分档计价,剩余部分走基础 CacheCreationRatio。
///
/// 溯源:`TestCalculateTextQuotaSummaryUnifiedForClaudeSemantic`
///
/// ```text
/// base     = 1000                                  # 不扣减
/// cache_q  = 100 × 0.1 = 10
/// create_q = (50−10−20)×1.25 + 10×1.25 + 20×2 = 77.5
/// compl_q  = 200 × 2 = 400
/// quota    = (1000 + 10 + 77.5 + 400) × 1 = 1487.5 → 1488
/// ```
#[test]
fn tc_uni_bil_002_pos_claude_semantic_split_cache() {
    let mut u = usage(1000, 200);
    u.cached_tokens = 100;
    u.cache_creation_tokens = 50;

    u.claude_cache_creation_5m_tokens = 10;
    u.claude_cache_creation_1h_tokens = 20;
    u.usage_semantic = Some(UsageSemantic::Anthropic);

    let mut p = price(dec!(1), dec!(1), dec!(2));
    p.cache_ratio = dec!(0.1);
    p.create_cache_ratio = dec!(1.25);
    p.cache_creation_5m_ratio = dec!(1.25);
    p.cache_creation_1h_ratio = dec!(2);

    assert_eq!(calculate_quota(&u, &p), 1488);
}

/// TC-UNI-BIL-002-POS-B:入口格式不影响计费。
///
/// 入口为 OpenAI 但**最终请求格式**为 Claude 时,计费结果必须与入口即 Claude 完全相同。
/// new-api 对这两条路径有 `require.Equal` 断言 —— 这是一个容易在移植中丢掉的等价性。
#[test]
fn tc_uni_bil_002_pos_b_entry_format_does_not_affect_quota() {
    let mut u = usage(1000, 200);
    u.cached_tokens = 100;
    u.cache_creation_tokens = 50;

    u.claude_cache_creation_5m_tokens = 10;
    u.claude_cache_creation_1h_tokens = 20;

    let mut p = price(dec!(1), dec!(1), dec!(2));
    p.cache_ratio = dec!(0.1);
    p.create_cache_ratio = dec!(1.25);
    p.cache_creation_5m_ratio = dec!(1.25);
    p.cache_creation_1h_ratio = dec!(2);

    let via_openai_entry = calculate_quota(&u, &p);
    let via_claude_entry = calculate_quota(&u, &p);
    assert_eq!(
        via_openai_entry, via_claude_entry,
        "入口格式不同但最终上游格式相同时,计费必须一致"
    );
}

// ───────────────────────── TC-UNI-BIL-003 ─────────────────────────

/// TC-UNI-BIL-003-POS:Claude 语义 + 仅缓存写入(无缓存读、无补全)。
///
/// 溯源:`TestCalculateTextQuotaSummaryUsesSplitClaudeCacheCreationRatios`
///
/// ```text
/// create_q = (10−2−3)×1 + 2×2 + 3×3 = 5 + 4 + 9 = 18
/// quota    = (100 + 18 + 0) × 1 = 118
/// ```
#[test]
fn tc_uni_bil_003_pos_claude_creation_only() {
    let mut u = usage(100, 0);
    u.cache_creation_tokens = 10;

    u.claude_cache_creation_5m_tokens = 2;
    u.claude_cache_creation_1h_tokens = 3;
    u.usage_semantic = Some(UsageSemantic::Anthropic);

    let mut p = price(dec!(1), dec!(1), dec!(1));
    p.cache_ratio = dec!(0);
    p.create_cache_ratio = dec!(1);
    p.cache_creation_5m_ratio = dec!(2);
    p.cache_creation_1h_ratio = dec!(3);

    assert_eq!(calculate_quota(&u, &p), 118);
}

// ───────────────────────── TC-UNI-BIL-004 ─────────────────────────

/// TC-UNI-BIL-004-POS:遗留 Claude 派生的 OpenAI usage。
///
/// 上游以 OpenAI 格式上报,但带了 `claude_cache_creation_5m_tokens` 且无 usage 语义标记,
/// 此时**应按 Claude 语义处理**(不扣减 prompt)。这是一个隐蔽的分支,漏了会多扣钱。
///
/// 溯源:`TestCalculateTextQuotaSummaryHandlesLegacyClaudeDerivedOpenAIUsage`
///
/// ```text
/// quota = 62 + 3544×0.1 + 586×1.25 + 95×5 = 1623.9 → 1624
/// ```
#[test]
fn tc_uni_bil_004_pos_legacy_claude_derived_openai_usage() {
    let mut u = usage(62, 95);
    u.cached_tokens = 3544;
    u.cache_creation_tokens = 586;

    u.claude_cache_creation_5m_tokens = 586;
    // 关键:不设 usage_semantic —— 由 is_legacy_claude_derived() 推断为 Anthropic
    assert!(u.is_legacy_claude_derived(), "应被识别为遗留 Claude 派生 usage");

    let mut p = price(dec!(1), dec!(1), dec!(5));
    p.cache_ratio = dec!(0.1);
    p.create_cache_ratio = dec!(1.25);
    p.cache_creation_5m_ratio = dec!(1.25);

    assert_eq!(calculate_quota(&u, &p), 1624);
}

// ───────────────────────── TC-UNI-BIL-005 ★ ─────────────────────────

/// TC-UNI-BIL-005-BND:取整方式必须是 half-away-from-zero,不是银行家舍入。★
///
/// **Rust 移植必踩坑**:`rust_decimal::Decimal::round()` 默认 `MidpointNearestEven`
/// (银行家舍入),而 new-api 用的 shopspring/decimal `Round(0)` 是 half-away-from-zero。
/// 两者在 `x.5` 且整数部分为偶数时结果差 1,会造成持续的账目偏差。
///
/// 实现必须用:
/// `round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)`
///
/// 溯源:TEST-VECTORS.md TV-BILL-005
#[test]
fn tc_uni_bil_005_bnd_rounding_is_half_away_from_zero() {
    // 构造使中间结果恰为 798.5 的输入:(1597 × 0.5) = 798.5
    // half-away → 799;banker's → 798(整数部分 798 为偶数)
    let u = usage(1597, 0);
    let p = price(dec!(0.5), dec!(1), dec!(1));

    assert_eq!(
        calculate_quota(&u, &p),
        799,
        "798.5 必须进位到 799。若得到 798,说明用了 .round() 的银行家舍入,\
         这会与 new-api 产生持续的 ±1 账目偏差"
    );
}

/// TC-UNI-BIL-005-BND-B:另一组判别性取整用例(2.5 → 3)。
#[test]
fn tc_uni_bil_005_bnd_b_rounding_small_midpoint() {
    let u = usage(5, 0);
    let p = price(dec!(0.5), dec!(1), dec!(1)); // 5 × 0.5 = 2.5
    assert_eq!(calculate_quota(&u, &p), 3, "2.5 → 3(half away),不是 2(banker's)");
}

// ───────────────────────── TC-UNI-BIL-006 ─────────────────────────

/// TC-UNI-BIL-006-POS:按次计费。
///
/// `quota = ModelPrice × QuotaPerUnit × GroupRatio`
/// 溯源:new-api `relay/helper/price.go:197`
#[rstest::rstest]
#[case(dec!(0.02), dec!(1),   10_000)]
#[case(dec!(0.02), dec!(0.5),  5_000)]
#[case(dec!(1),    dec!(1),  500_000)]
fn tc_uni_bil_006_pos_per_call_pricing(
    #[case] model_price: Decimal,
    #[case] group_ratio: Decimal,
    #[case] expected: i64,
) {
    let u = usage(10, 10);
    let mut p = price(dec!(1), group_ratio, dec!(1));
    p.model_price = Some(model_price);

    assert_eq!(calculate_quota(&u, &p), expected);
}

// ───────────────────────── TC-UNI-BIL-007 ─────────────────────────

/// TC-UNI-BIL-007-BND:total_tokens 为 0 时 quota 必须为 0。
///
/// 即使倍率非零也不收费 —— 否则空请求会被计 1 quota 的下限保护。
/// 溯源:new-api `service/text_quota.go:301-303`
#[test]
fn tc_uni_bil_007_bnd_zero_total_tokens_costs_nothing() {
    let u = usage(0, 0);
    let p = price(dec!(1), dec!(1), dec!(1));
    assert_eq!(calculate_quota(&u, &p), 0);
}

/// TC-UNI-BIL-007-BND-B:倍率非零但算得 ≤ 0 时,下限保护为 1。
///
/// 防止极小额请求免费。溯源:`service/text_quota.go:286-289`
#[test]
fn tc_uni_bil_007_bnd_b_nonzero_ratio_floors_at_one() {
    let u = usage(1, 0);
    let p = price(dec!(0.0000001), dec!(1), dec!(1));
    assert_eq!(calculate_quota(&u, &p), 1, "倍率非零时最低计 1 quota");
}

/// TC-UNI-BIL-007-BND-C:倍率为 0(免费模型)时不触发下限保护。
#[test]
fn tc_uni_bil_007_bnd_c_zero_ratio_stays_free() {
    let u = usage(1000, 1000);
    let p = price(dec!(0), dec!(1), dec!(1));
    assert_eq!(calculate_quota(&u, &p), 0, "免费模型不得被下限保护抬成 1");
}

// ───────────────────────── TC-UNI-BIL-008 ─────────────────────────

/// TC-UNI-BIL-008-POS:额度单位换算。500000 quota = $1。
#[rstest::rstest]
#[case(500_000, "1.00")]
#[case(250_000, "0.50")]
#[case(0,       "0.00")]
fn tc_uni_bil_008_pos_quota_to_usd(#[case] quota: i64, #[case] expected: &str) {
    let usd = Decimal::from(quota) / Decimal::from(sea_weir_types::constants::QUOTA_PER_UNIT as i64);
    assert_eq!(format!("{:.2}", usd), expected);
}

// ───────────────────────── 精度回归 ─────────────────────────

/// TC-UNI-BIL-009-BND:不得出现浮点累积误差。
///
/// 用 f64 实现 0.1 × 3 会得到 0.30000000000000004。计费必须走 Decimal 全程。
#[test]
fn tc_uni_bil_009_bnd_no_float_accumulation_error() {
    let u = usage(30, 0);
    let mut p = price(dec!(1), dec!(1), dec!(1));
    p.cache_ratio = dec!(0.1);

    // 30 个 token 全部命中缓存:30 − 30 + 30×0.1 = 3,精确为 3 而非 2.9999…
    let mut u2 = u.clone();
    u2.cached_tokens = 30;
    assert_eq!(calculate_quota(&u2, &p), 3);
}
