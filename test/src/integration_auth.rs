//! L2 集成 — 认证与鉴权。用例文档:`cases/07-integration-auth.md`
//!
//! 安全边界,优先级仅次于账务。fail-close 语义需要真实缓存才能验证。
//! 溯源:ADR-004、TEST-VECTORS.md §7

use crate::require_dep;
use pretty_assertions::assert_eq;

// ───────────── sk-token 提取优先级(TEST-VECTORS §7.1)─────────────

/// TC-INT-AUT-001-POS:五个来源的提取优先级。★
///
/// 同时提供多个来源时按顺序取第一个命中。顺序错了会导致
/// Claude/Gemini 客户端用错 key。
#[test]
fn tc_int_aut_001_pos_sk_token_extraction_priority() {
    use sea_weir_server::middleware::auth::extract_sk_token;
    let mut h = http::HeaderMap::new();
    h.insert("authorization", "Bearer sk-AAA".parse().unwrap());
    h.insert("x-api-key", "sk-BBB".parse().unwrap());
    h.insert("x-goog-api-key", "sk-CCC".parse().unwrap());

    assert_eq!(
        extract_sk_token(&h, "key=sk-DDD").as_deref(),
        Some("sk-AAA"),
        "Authorization 优先级最高"
    );
}

/// TC-INT-AUT-002-POS:逐级回落。
#[rstest::rstest]
#[case(vec![("x-api-key", "sk-BBB")],                       "",            Some("sk-BBB"))]
#[case(vec![("x-goog-api-key", "sk-CCC")],                  "",            Some("sk-CCC"))]
#[case(vec![],                                              "key=sk-DDD",  Some("sk-DDD"))]
#[case(vec![("mj-api-secret", "sk-EEE")],                   "",            Some("sk-EEE"))]
#[case(vec![],                                              "",            None)]
fn tc_int_aut_002_pos_sk_token_fallback(
    #[case] headers: Vec<(&str, &str)>,
    #[case] query: &str,
    #[case] expected: Option<&str>,
) {
    use sea_weir_server::middleware::auth::extract_sk_token;
    let mut h = http::HeaderMap::new();
    for (k, v) in headers {
        h.insert(
            http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            v.parse().unwrap(),
        );
    }
    assert_eq!(extract_sk_token(&h, query).as_deref(), expected);
}

// ───────────── 令牌校验(TEST-VECTORS §7.2)─────────────

/// TC-INT-AUT-010-BND:令牌状态与过期判定。
#[rstest::rstest]
#[case(1, -1,           true,  "启用且永不过期")]
#[case(2, -1,           false, "已禁用")]
#[case(1, 1_000_000_000, false, "已过期")]
#[case(1, 99_999_999_999, true, "未来过期时间")]
#[tokio::test]
async fn tc_int_aut_010_bnd_token_status_and_expiry(
    #[case] status: i32,
    #[case] expired_time: i64,
    #[case] should_pass: bool,
    #[case] desc: &str,
) {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 建对应令牌,发起中继请求,断言是否通过
    let _ = (status, expired_time, should_pass, desc);
}

/// TC-INT-AUT-011-BND:额度判定与 unlimited_quota。
#[rstest::rstest]
#[case(false, 0,    false, "非无限且余额 0 → 拒绝")]
#[case(false, 1,    true,  "非无限且余额 1 → 放行")]
#[case(true,  0,    true,  "无限额度,余额 0 也放行")]
#[case(true,  -100, true,  "无限额度,负余额也放行")]
#[tokio::test]
async fn tc_int_aut_011_bnd_quota_check(
    #[case] unlimited: bool,
    #[case] remain: i64,
    #[case] should_pass: bool,
    #[case] desc: &str,
) {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    let _ = (unlimited, remain, should_pass, desc);
}

/// TC-INT-AUT-012-NEG:IP 白名单未命中 → 403。
#[tokio::test]
async fn tc_int_aut_012_neg_ip_allowlist() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): allow_ips="10.0.0.0/8",客户端 IP 1.2.3.4 → 403
}

/// TC-INT-AUT-013-NEG:归属用户被禁用 → 401。
///
/// 封禁用户后其令牌必须立即失效,不能等缓存过期。
#[tokio::test]
async fn tc_int_aut_013_neg_banned_user_token_rejected() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 禁用用户 → 断言其令牌立即被拒(缓存已批量失效)
}

/// TC-INT-AUT-014-NEG:模型不在 model_limits 内 → 403。
#[tokio::test]
async fn tc_int_aut_014_neg_model_limit() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): model_limits=["gpt-3.5"],请求 gpt-4 → 403 permission_error
}

// ───────────── 管理面会话(TEST-VECTORS §7.3)─────────────

/// TC-INT-AUT-020-NEG:缺 New-Api-User 头 → 401。
#[tokio::test]
async fn tc_int_aut_020_neg_missing_new_api_user_header() {
    let _url = require_dep!(crate::common::TestEnv::get().http_base_url, "SEAWEIR_HTTP_BASE_URL");
    // TODO(实现): 带合法 session cookie 但不带 New-Api-User → 401
}

/// TC-INT-AUT-021-NEG:New-Api-User 与会话不一致 → 401(防串号)。★
///
/// 这是 new-api 的防串号设计:即使拿到了别人的 session,
/// 头里的 user id 对不上也无法冒用。
#[tokio::test]
async fn tc_int_aut_021_neg_new_api_user_mismatch() {
    let _url = require_dep!(crate::common::TestEnv::get().http_base_url, "SEAWEIR_HTTP_BASE_URL");
    // TODO(实现): user=1 的 session + New-Api-User: 2 → 401
}

/// TC-INT-AUT-022-NEG:会话吊销后立即失效。
#[tokio::test]
async fn tc_int_aut_022_neg_revoked_session() {
    let _url = require_dep!(crate::common::TestEnv::get().redis_url, "REDIS_URL");
    // TODO(实现): 登出 → jti 入吊销列表 → 同一 cookie 再请求 → 401
}

/// TC-INT-AUT-023-POS:无会话时回落 access token。
#[tokio::test]
async fn tc_int_aut_023_pos_access_token_fallback() {
    let _url = require_dep!(crate::common::TestEnv::get().http_base_url, "SEAWEIR_HTTP_BASE_URL");
    // TODO(实现): 无 cookie + Authorization: Bearer <access_token> → 放行
}

/// TC-INT-AUT-024-NEG:缓存不可达时鉴权 fail-close。★★
///
/// **安全关键**。ADR-007 规定鉴权类路径 fail-close:缓存挂了宁可拒绝服务,
/// 也不能放行未经校验的请求。若实现为 fail-open,缓存故障 = 全站鉴权失效。
#[tokio::test]
async fn tc_int_aut_024_neg_fail_close_on_cache_outage() {
    let _url = require_dep!(crate::common::TestEnv::get().redis_url, "REDIS_URL");
    // TODO(实现):
    //   1. 建立合法会话
    //   2. 停掉 Valkey 容器
    //   3. 发起管理面请求 → 断言 401(不是 200)
    //
    //   对照组:非关键路径(如定价视图)应 fail-open 回源 DB
    use sea_weir_repository::cache::{should_fail_open, CachePathKind};
    assert!(!should_fail_open(CachePathKind::Auth), "鉴权路径必须 fail-close");
    assert!(should_fail_open(CachePathKind::Data), "数据路径可 fail-open 回源");
}

// ───────────── 敏感操作凭证 ─────────────

/// TC-INT-AUT-030-POS:敏感凭证一次性消费。★
///
/// 取渠道密钥的凭证若可重放,等于长期授权。
#[tokio::test]
async fn tc_int_aut_030_pos_secure_credential_single_use() {
    let _url = require_dep!(crate::common::TestEnv::get().redis_url, "REDIS_URL");
    // TODO(实现):
    //   1. POST /api/verify 换凭证
    //   2. 用凭证取渠道密钥 → 200
    //   3. 用同一凭证再取 → 401
}

/// TC-INT-AUT-031-NEG:凭证过期失效。
#[tokio::test]
async fn tc_int_aut_031_neg_secure_credential_expiry() {
    let _url = require_dep!(crate::common::TestEnv::get().redis_url, "REDIS_URL");
    // TODO(实现): TTL 设 1s,等待 2s 后使用 → 401
}

/// TC-INT-AUT-032-NEG:普通用户不得指定渠道。
///
/// 溯源:new-api `middleware/auth.go:429-437`
#[tokio::test]
async fn tc_int_aut_032_neg_normal_user_cannot_pin_channel() {
    let _url = require_dep!(crate::common::TestEnv::get().http_base_url, "SEAWEIR_HTTP_BASE_URL");
    // TODO(实现): role=1 用户使用 sk-xxx-123 → 403「普通用户不支持指定渠道」
}

/// TC-INT-AUT-033-POS:admin 可指定渠道。
#[tokio::test]
async fn tc_int_aut_033_pos_admin_can_pin_channel() {
    let _url = require_dep!(crate::common::TestEnv::get().http_base_url, "SEAWEIR_HTTP_BASE_URL");
    // TODO(实现): role=10 用户使用 sk-xxx-123 → 路由到 channel 123
}
