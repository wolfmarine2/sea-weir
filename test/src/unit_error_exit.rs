//! L1 单元 — 错误分类与出口形状。用例文档:`cases/01-unit-types.md`
//!
//! **管理面业务错误是 HTTP 200** 这条最容易被"改成 RESTful"而破坏契约。
//! 溯源:ADR-008、TEST-VECTORS.md §6

use pretty_assertions::assert_eq;
use sea_weir_types::{AppError, ErrorClass, NewApiError, RelayFormat};

/// TC-UNI-TYP-030-POS:错误的恢复语义分类。
#[rstest::rstest]
#[case(AppError::Database("x".into()),      ErrorClass::Transient)]
#[case(AppError::Cache("x".into()),         ErrorClass::Transient)]
#[case(AppError::Upstream("x".into()),      ErrorClass::Transient)]
#[case(AppError::Biz("x".into()),           ErrorClass::Permanent)]
#[case(AppError::Unauthorized("x".into()),  ErrorClass::Permanent)]
#[case(AppError::Forbidden("x".into()),     ErrorClass::Permanent)]
#[case(AppError::BadRequest("x".into()),    ErrorClass::Permanent)]
#[case(AppError::NotFound("x".into()),      ErrorClass::Permanent)]
#[case(AppError::QuotaExceeded,             ErrorClass::Recoverable)]
#[case(AppError::RateLimited { retry_after_secs: 1 }, ErrorClass::Recoverable)]
#[case(AppError::Config("x".into()),        ErrorClass::Unrecoverable)]
#[case(AppError::Internal("x".into()),      ErrorClass::Unrecoverable)]
fn tc_uni_typ_030_pos_error_class(#[case] e: AppError, #[case] expected: ErrorClass) {
    assert_eq!(e.class(), expected, "{e:?} 的分类");
}

/// TC-UNI-TYP-031-POS:管理面业务错误必须是 HTTP 200。★
///
/// 契约(CONTRACTS.md §统一响应包裹):管理面**业务错误也返回 200**,
/// 由 `success:false` 表达;只有鉴权/限流走真实状态码。
/// 若实现"改良"成 400/500,前端与所有存量客户端都会走错分支。
#[test]
fn tc_uni_typ_031_pos_admin_business_error_is_200() {
    assert_eq!(
        AppError::Biz("余额不足".into()).admin_status(),
        http::StatusCode::OK,
        "管理面业务错误必须 HTTP 200 + success:false,不得改成 4xx"
    );
    assert_eq!(AppError::BadRequest("x".into()).admin_status(), http::StatusCode::OK);
    assert_eq!(AppError::QuotaExceeded.admin_status(), http::StatusCode::OK);
}

/// TC-UNI-TYP-032-POS:管理面只有**未认证**与**限流**走真实状态码。★
///
/// 录制实测校正:首版断言 `Forbidden → 403` 是错的。
/// new-api 的 `authHelper` 在 `role < minRole` 时走
/// `c.JSON(http.StatusOK, {success:false, "Unauthorized, insufficient privileges"})`,
/// 即**角色不足是 200**。403 只出现在中继面(如"无权访问 X 分组")。
///
/// 若按 403 实现,前端 axios 拦截器会把越权当网络错误,丢失真实提示文案。
///
/// 溯源:`middleware/auth.go` authHelper;
///       `baseline/api_error_insufficient_root.json`(role=1 访问 RootAuth 端点 → 200)
#[test]
fn tc_uni_typ_032_pos_auth_status_mapping() {
    assert_eq!(
        AppError::Unauthorized("x".into()).admin_status(),
        http::StatusCode::UNAUTHORIZED,
        "未认证(无凭证)是 401"
    );
    assert_eq!(
        AppError::Forbidden("x".into()).admin_status(),
        http::StatusCode::OK,
        "管理面角色不足是 200 + success:false,不是 403"
    );
    assert_eq!(
        AppError::RateLimited { retry_after_secs: 5 }.admin_status(),
        http::StatusCode::TOO_MANY_REQUESTS
    );
}

/// TC-UNI-TYP-033-POS:中继面错误体形状(OpenAI 格式)。
#[test]
fn tc_uni_typ_033_pos_relay_error_openai_shape() {
    let e = NewApiError {
        status_code: 403,
        error_code: "insufficient_quota".into(),
        error_type: "insufficient_quota".into(),
        message: "余额不足".into(),
        local_error: false,
        skip_retry: false,
        record_error_log: true,
    };
    let body = e.to_body(RelayFormat::OpenAi);
    let err = body.get("error").expect("OpenAI 格式必须有顶层 error 对象");
    assert!(err.get("message").is_some());
    assert!(err.get("type").is_some());
    assert!(err.get("code").is_some());
}

/// TC-UNI-TYP-034-POS:Claude 格式错误体是 `{"type":"error","error":{...}}`。
///
/// 与 OpenAI 形状不同 —— Claude SDK 会校验顶层 `type` 字段。
#[test]
fn tc_uni_typ_034_pos_relay_error_claude_shape() {
    let e = NewApiError {
        status_code: 401,
        error_code: "authentication_error".into(),
        error_type: "authentication_error".into(),
        message: "invalid key".into(),
        local_error: false,
        skip_retry: false,
        record_error_log: true,
    };
    let body = e.to_body(RelayFormat::Claude);
    assert_eq!(body.get("type").and_then(|v| v.as_str()), Some("error"));
    assert!(body.get("error").is_some());
}

/// TC-UNI-TYP-035-POS:MJ 格式错误体与 code=30 → 429 映射。
#[test]
fn tc_uni_typ_035_pos_relay_error_mj_shape() {
    let e = NewApiError {
        status_code: 429,
        error_code: "30".into(),
        error_type: "mj_error".into(),
        message: "queue full".into(),
        local_error: false,
        skip_retry: false,
        record_error_log: true,
    };
    let body = e.to_body(RelayFormat::Midjourney);
    for k in ["code", "description", "result"] {
        assert!(body.get(k).is_some(), "MJ 错误体缺字段 {k}");
    }
    assert_eq!(body["code"], 30);
}

/// TC-UNI-TYP-036-POS:AppError → NewApiError 的映射(录制实测校正)。★
///
/// 首版按 OpenAI 标准错误类型写的(`authentication_error` / `invalid_request_error`),
/// **录制实测证明 new-api 不是这样**:
/// - `error.type` 恒为 `new_api_error`
/// - `error.code` 在鉴权与参数错误时是**空串**,只有业务可识别的错误才填值
/// - 无可用渠道是 **503**,不是 404
///
/// 溯源:`baseline/relay_error_no_token.json`、`relay_error_no_channel.json`;
///       TEST-VECTORS §6 F1/F2
#[rstest::rstest]
#[case(AppError::QuotaExceeded,            403, "insufficient_quota")]
#[case(AppError::Unauthorized("x".into()), 401, "")]
#[case(AppError::Forbidden("x".into()),    403, "")]
#[case(AppError::BadRequest("x".into()),   400, "")]
#[case(AppError::NotFound("x".into()),     503, "model_not_found")]
fn tc_uni_typ_036_pos_app_error_to_relay(
    #[case] e: AppError,
    #[case] status: u16,
    #[case] code: &str,
) {
    let n: NewApiError = e.into();
    assert_eq!(n.status_code, status, "状态码");
    assert_eq!(n.error_code, code, "error.code");
    assert_eq!(n.error_type, "new_api_error", "中继面 error.type 恒为 new_api_error");
}

/// TC-UNI-TYP-037-NEG:错误出口不得泄漏密钥。
///
/// 上游返回的错误里常带回显的 Authorization 头,直接透传会把渠道密钥暴露给终端用户。
#[test]
fn tc_uni_typ_037_neg_no_secret_leak_in_error() {
    let raw = "upstream rejected key sk-abcdefghijklmnopqrstuvwxyz012345";
    let masked = sea_weir_types::error::mask_sensitive(raw);
    assert!(
        !masked.contains("sk-abcdefghijklmnopqrstuvwxyz012345"),
        "错误出口必须对 sk- 密钥打码,实际:{masked}"
    );
}

/// TC-UNI-TYP-038-POS:panic 兜底错误体含 request id。
#[test]
fn tc_uni_typ_038_pos_panic_body_carries_request_id() {
    let body = sea_weir_types::relay_error::panic_body("req-12345");
    let s = body.to_string();
    assert!(s.contains("new_api_panic"), "panic 错误体应标 type=new_api_panic");
    assert!(s.contains("req-12345"), "panic 错误体必须回显 request id 以便排障");
}
