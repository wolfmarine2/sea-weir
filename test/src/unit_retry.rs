//! L1 单元 — 重试与自动禁用判定。用例文档:`cases/03-unit-select-retry.md`
//!
//! 表驱动测试。判定**有严格短路顺序**,顺序错了会得到不同结果(尤其"渠道错误优先于
//! 次数判定"这条)。
//!
//! 溯源:new-api `controller/relay.go:319-349`、
//!       `setting/operation_setting/status_code_ranges.go`;TEST-VECTORS.md §4

use pretty_assertions::assert_eq;
use sea_weir_core::relay::autoban::{should_disable, should_retry, RetryContext};
use sea_weir_types::NewApiError;

fn err(status: u16, code: &str) -> NewApiError {
    NewApiError {
        status_code: status,
        error_code: code.into(),
        error_type: "upstream_error".into(),
        message: "test".into(),
        local_error: false,
        skip_retry: false,
        record_error_log: true,
    }
}

fn ctx() -> RetryContext {
    RetryContext { retry_times_left: 3, has_specific_channel: false, affinity_skip_retry: false }
}

// ───────────── 状态码判定矩阵(TEST-VECTORS §4.2)─────────────

/// TC-UNI-RTY-001-BND:状态码 → 是否重试的完整矩阵。★
///
/// 400 / 408 / 504 / 524 是审查中核实过的排除项,必须逐个守住。
#[rstest::rstest]
#[case(100, true)]
#[case(200, false)]  // 2xx 一律不重试
#[case(201, false)]
#[case(204, false)]
#[case(301, true)]
#[case(399, true)]
#[case(400, false)]  // ★ 明确排除
#[case(401, true)]
#[case(407, true)]
#[case(408, false)]  // ★ 明确排除
#[case(409, true)]
#[case(429, true)]
#[case(499, true)]
#[case(500, true)]
#[case(503, true)]
#[case(504, false)]  // ★ alwaysSkipRetryStatusCodes
#[case(505, true)]
#[case(523, true)]
#[case(524, false)]  // ★ alwaysSkipRetryStatusCodes
#[case(525, true)]
#[case(599, true)]
#[case(0,   true)]   // 超出 100..=599 视为网络异常
#[case(99,  true)]
#[case(600, true)]
fn tc_uni_rty_001_bnd_status_code_matrix(#[case] status: u16, #[case] expected: bool) {
    let e = err(status, "upstream_error");
    assert_eq!(
        should_retry(&e, &ctx()),
        expected,
        "status={status} 的重试判定与 new-api 不符"
    );
}

// ───────────── 短路顺序(TEST-VECTORS §4.1)─────────────

/// TC-UNI-RTY-002-POS:渠道错误优先于次数判定。★
///
/// 判定顺序第 3 条在第 5 条之前:即使 `retry_times_left == 0`,
/// 渠道类错误(`channel:*`)仍然返回 true。顺序写反会导致渠道故障时不切换渠道。
#[test]
fn tc_uni_rty_002_pos_channel_error_beats_retry_count() {
    let e = err(500, "channel:invalid_key");
    let exhausted = RetryContext { retry_times_left: 0, ..ctx() };
    assert!(
        should_retry(&e, &exhausted),
        "渠道错误的判定必须在次数判定之前短路返回 true"
    );
}

/// TC-UNI-RTY-003-NEG:亲和性失败跳过重试,优先级最高。
///
/// 判定顺序第 2 条:即使是渠道错误也不重试(粘性路由的语义要求)。
#[test]
fn tc_uni_rty_003_neg_affinity_skip_wins_over_channel_error() {
    let e = err(500, "channel:invalid_key");
    let c = RetryContext { affinity_skip_retry: true, ..ctx() };
    assert!(!should_retry(&e, &c), "亲和性跳过标记优先级高于渠道错误");
}

/// TC-UNI-RTY-004-NEG:指定渠道时不重试。
///
/// `sk-xxx-{channelId}` 语义是"我就要这个渠道",换渠道违背用户意图。
#[test]
fn tc_uni_rty_004_neg_specific_channel_never_retries() {
    let e = err(500, "upstream_error");
    let c = RetryContext { has_specific_channel: true, ..ctx() };
    assert!(!should_retry(&e, &c));
}

/// TC-UNI-RTY-005-NEG:次数耗尽后非渠道错误不重试。
#[test]
fn tc_uni_rty_005_neg_exhausted_retry_count() {
    let e = err(500, "upstream_error");
    let c = RetryContext { retry_times_left: 0, ..ctx() };
    assert!(!should_retry(&e, &c));
}

/// TC-UNI-RTY-006-NEG:`skip_retry` 标记生效。
#[test]
fn tc_uni_rty_006_neg_skip_retry_flag() {
    let mut e = err(500, "upstream_error");
    e.skip_retry = true;
    assert!(!should_retry(&e, &ctx()));
}

/// TC-UNI-RTY-007-NEG:适配器本地错误既不重试也不禁用。★
///
/// `local_error = true` 表示是我方解析出错,不是渠道的锅。
/// 误判会导致好渠道被批量禁用。
#[test]
fn tc_uni_rty_007_neg_local_error_neither_retries_nor_disables() {
    let mut e = err(500, "channel:bad_response");
    e.local_error = true;
    assert!(!should_retry(&e, &ctx()), "本地错误不应重试");
    assert!(!should_disable(&e, true), "本地错误不应禁用渠道");
}

// ───────────── 自动禁用(TEST-VECTORS §4.3)─────────────

/// TC-UNI-RTY-010-BND:自动禁用的状态码判定。默认区间仅 401。
#[rstest::rstest]
#[case(401, true)]
#[case(400, false)]
#[case(403, false)]
#[case(429, false)]
#[case(500, false)]
#[case(502, false)]
fn tc_uni_rty_010_bnd_disable_status_matrix(#[case] status: u16, #[case] expected: bool) {
    let e = err(status, "channel:error");
    assert_eq!(should_disable(&e, true), expected, "status={status}");
}

/// TC-UNI-RTY-011-NEG:`auto_ban` 关闭时恒不禁用。
///
/// 渠道级开关,运维用来保护关键渠道不被自动摘除。
#[test]
fn tc_uni_rty_011_neg_auto_ban_disabled() {
    let e = err(401, "channel:invalid_key");
    assert!(!should_disable(&e, false), "auto_ban=false 时任何错误都不得禁用");
}

/// TC-UNI-RTY-012-POS:是否渠道错误的判别。
#[rstest::rstest]
#[case("channel:invalid_key",  true)]
#[case("channel:rate_limit",   true)]
#[case("insufficient_quota",   false)]
#[case("invalid_request",      false)]
#[case("upstream_error",       false)]
fn tc_uni_rty_012_pos_is_channel_error(#[case] code: &str, #[case] expected: bool) {
    assert_eq!(err(500, code).is_channel_error(), expected, "code={code}");
}
