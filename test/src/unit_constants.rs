//! L1 单元 — 常量与角色闸门。用例文档:`cases/01-unit-types.md`
//!
//! 这些测试的价值在于**锁死契约取值**:一旦有人改动,测试必须红。
//! 取值溯源见 TEST-VECTORS.md 与 adr-review-report.md「源码复核明细」。

use pretty_assertions::assert_eq;
use sea_weir_types::constants::{self, role, LogType};

/// TC-UNI-TYP-001-POS:额度单位。溯源 new-api `common/constants.go:22`。
#[test]
fn tc_uni_typ_001_pos_quota_per_unit() {
    assert_eq!(constants::QUOTA_PER_UNIT, 500_000.0, "500000 quota = $1,改动即破坏计费兼容");
}

/// TC-UNI-TYP-002-POS:角色取值。溯源 `common/constants.go:148-151`。
#[test]
fn tc_uni_typ_002_pos_role_values() {
    assert_eq!(role::GUEST, 0);
    assert_eq!(role::COMMON, 1);
    assert_eq!(role::ADMIN, 10);
    assert_eq!(role::ROOT, 100);
}

/// TC-UNI-TYP-002-NEG:非法角色值必须被拒绝。
#[rstest::rstest]
#[case(0, true)]
#[case(1, true)]
#[case(10, true)]
#[case(100, true)]
#[case(2, false)]
#[case(11, false)]
#[case(99, false)]
#[case(-1, false)]
#[case(101, false)]
fn tc_uni_typ_002_neg_role_validity(#[case] r: i32, #[case] valid: bool) {
    assert_eq!(role::is_valid(r), valid, "role={r}");
}

/// TC-UNI-TYP-003-BND:角色闸门边界。
///
/// 闸门是 `>=` 而非 `==` —— role=99 能过 AdminAuth 但过不了 RootAuth。
/// 溯源:TEST-VECTORS.md §7.4
#[rstest::rstest]
#[case(0,   false, false, false)]
#[case(1,   true,  false, false)]
#[case(9,   true,  false, false)]
#[case(10,  true,  true,  false)]
#[case(99,  true,  true,  false)]
#[case(100, true,  true,  true)]
fn tc_uni_typ_003_bnd_role_gates(
    #[case] r: i32,
    #[case] user_auth: bool,
    #[case] admin_auth: bool,
    #[case] root_auth: bool,
) {
    assert_eq!(r >= role::COMMON, user_auth, "UserAuth role={r}");
    assert_eq!(r >= role::ADMIN, admin_auth, "AdminAuth role={r}");
    assert_eq!(r >= role::ROOT, root_auth, "RootAuth role={r}");
}

/// TC-UNI-TYP-004-POS:日志类型判别值。溯源 `model/log.go:46-52`。
///
/// 这些数字直接进数据库与 API 响应,错位会让历史日志被归错类。
#[test]
fn tc_uni_typ_004_pos_log_type_discriminants() {
    assert_eq!(LogType::Unknown as i32, 0);
    assert_eq!(LogType::Topup as i32, 1);
    assert_eq!(LogType::Consume as i32, 2);
    assert_eq!(LogType::Manage as i32, 3);
    assert_eq!(LogType::System as i32, 4);
    assert_eq!(LogType::Error as i32, 5);
    assert_eq!(LogType::Refund as i32, 6);
}

/// TC-UNI-TYP-005-POS:令牌 key 长度。溯源 `common/utils.go:251-253`。
#[test]
fn tc_uni_typ_005_pos_token_key_length() {
    assert_eq!(constants::TOKEN_KEY_LEN, 48, "sk- 前缀不入库,库内恰好 48 位");
}

/// TC-UNI-TYP-006-POS:默认周期。溯源见各常量注释。
#[test]
fn tc_uni_typ_006_pos_default_intervals() {
    use constants::defaults::*;
    assert_eq!(SYNC_FREQUENCY_SECS, 60);
    assert_eq!(TASK_POLL_INTERVAL_SECS, 15);
    assert_eq!(SSE_PING_INTERVAL_SECS, 10);
    assert_eq!(CACHE_TTL_SECS, 60);
    assert_eq!(EMAIL_CODE_TTL_SECS, 600);
    assert_eq!(SESSION_TTL_DAYS, 30);
}

/// TC-UNI-TYP-007-POS:默认重试区间展开后不含被排除的状态码。
///
/// 这是把区间配置转成判定的自检:区间写错会静默改变重试行为。
/// 溯源:`setting/operation_setting/status_code_ranges.go:20-33`
#[test]
fn tc_uni_typ_007_pos_default_retry_ranges_exclude_expected() {
    use constants::retry_policy::*;
    let in_range = |c: u16| DEFAULT_RETRY_RANGES.iter().any(|(s, e)| c >= *s && c <= *e);

    // 明确排除
    for code in [400u16, 408] {
        assert!(!in_range(code), "{code} 不应在可重试区间内");
    }
    // 2xx 一律不重试
    for code in [200u16, 201, 204, 299] {
        assert!(!in_range(code), "2xx({code})不应重试");
    }
    // alwaysSkip 优先级更高,但也不应出现在区间里
    for code in ALWAYS_SKIP_RETRY {
        assert!(!in_range(*code), "{code} 属于 alwaysSkip,不应在区间内");
    }
    // 典型可重试
    for code in [401u16, 407, 409, 429, 499, 500, 503, 505, 523, 525, 599] {
        assert!(in_range(code), "{code} 应在可重试区间内");
    }
}

/// TC-UNI-TYP-008-POS:默认自动禁用区间仅 401。
#[test]
fn tc_uni_typ_008_pos_default_disable_range_is_401_only() {
    use constants::retry_policy::DEFAULT_DISABLE_RANGES;
    assert_eq!(DEFAULT_DISABLE_RANGES, &[(401u16, 401u16)]);
}

/// TC-UNI-TYP-009-POS:状态常量三态语义。
///
/// 渠道的 2(手动禁用)与 3(自动禁用)必须区分 —— 只有自动禁用才可被渠道测试恢复。
#[test]
fn tc_uni_typ_009_pos_status_semantics() {
    use constants::status::*;
    assert_eq!(ENABLED, 1);
    assert_eq!(DISABLED, 2);
    assert_eq!(AUTO_DISABLED, 3);
    assert_ne!(DISABLED, AUTO_DISABLED, "手动禁用与自动禁用必须可区分");
}
