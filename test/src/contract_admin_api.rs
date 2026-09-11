//! L3 契约 — 管理面。用例文档:`cases/09-contract-admin.md`
//!
//! **本层直接对应 §1.2 目标 1(契约兼容)**,是重构项目最该投入的一层。
//!
//! 判定基准是从 new-api 实例**录制的真实响应**(`cases/fixtures/baseline/`),
//! 不是设计文档 —— v1.0 文档曾把 camelCase 和 0 起分页当成契约写进去。
//!
//! 录制:`bash test/env/record-baseline.sh`

use crate::common::{assert_all_snake_case, assert_no_missing_fields, load_baseline};
use pretty_assertions::assert_eq;

/// 从基线 fixture 中取出响应体。
fn baseline_body(name: &str) -> serde_json::Value {
    let f = load_baseline(name);
    f["response"]["body"].clone()
}

// ───────────── 字段命名(审查发现的 P0)─────────────

/// TC-CTR-API-001-POS:管理面响应字段一律 snake_case。★★
///
/// **审查发现的 P0 偏差**:v1.0 文档规定 camelCase 并声称"与 new-api 逐字段一致",
/// 而 new-api 全量 snake_case(`display_name`、`remain_quota`、`model_limits_enabled`…)。
/// 按 camelCase 实现会让所有存量客户端与前端失效,直接推翻灰度替换的目标。
#[rstest::rstest]
#[case("api_user_self")]
#[case("api_token_list")]
#[case("api_channel_list")]
#[case("api_log_self")]
#[case("api_status")]
fn tc_ctr_api_001_pos_all_fields_snake_case(#[case] fixture: &str) {
    let baseline = baseline_body(fixture);
    assert_all_snake_case(&baseline);

    // TODO(实现): 取 sea-weir 对同一端点的响应,做同样断言 + 字段集比对
    // let actual = call_seaweir(...);
    // assert_all_snake_case(&actual);
    // assert_no_missing_fields(&baseline, &actual);
}

/// TC-CTR-API-002-POS:字段集不得缺失(允许新增)。★
///
/// 缺一个字段就可能让某个客户端功能静默失效。允许新增是为了留出演进空间。
#[rstest::rstest]
#[case("api_user_self")]
#[case("api_token_list")]
#[case("api_channel_list")]
fn tc_ctr_api_002_pos_no_missing_fields(#[case] fixture: &str) {
    let baseline = baseline_body(fixture);
    // TODO(实现): let actual = call_seaweir(fixture_endpoint(fixture));
    let actual = baseline.clone(); // 占位:实现后替换为真实调用
    assert_no_missing_fields(&baseline, &actual);
}

// ───────────── 统一响应包裹 ─────────────

/// TC-CTR-API-010-POS:成功响应恰为 `{success,message,data}`。
#[test]
fn tc_ctr_api_010_pos_success_envelope_shape() {
    let b = baseline_body("api_user_self");
    let obj = b.as_object().expect("响应应为对象");
    assert_eq!(obj["success"], true);
    assert!(obj.contains_key("message"), "必须有 message 字段(成功时为空串)");
    assert!(obj.contains_key("data"));
    assert_eq!(obj["message"], "", "成功时 message 为空串而非 null");
}

/// TC-CTR-API-011-POS:业务错误是 HTTP 200 + success:false。★★
///
/// 最容易被"改成 RESTful"而破坏的一条。前端的 axios 拦截器与所有存量客户端
/// 都按"200 + success 判定"写的,改成 4xx 会让它们全部走错分支。
#[test]
fn tc_ctr_api_011_pos_business_error_is_http_200() {
    let f = load_baseline("api_error_business");
    assert_eq!(
        f["response"]["status"], 200,
        "管理面业务错误必须 HTTP 200。若基线里是 4xx,说明录制的不是业务错误场景"
    );
    assert_eq!(f["response"]["body"]["success"], false);
    assert!(
        f["response"]["body"]["message"].as_str().is_some_and(|m| !m.is_empty()),
        "业务错误必须带非空 message"
    );
}

/// TC-CTR-API-012-POS:未认证是 401,**角色不足是 200**。★
///
/// 录制实测校正。首版按"未授权 → 403"写,与 new-api 实际行为不符。
/// 溯源:TEST-VECTORS §6 F5
#[rstest::rstest]
#[case("api_error_unauthorized",        401)]
#[case("api_error_insufficient_root",   200)]
#[case("api_error_insufficient_admin",  200)]
fn tc_ctr_api_012_pos_auth_error_status(#[case] fixture: &str, #[case] status: u16) {
    let f = load_baseline(fixture);
    assert_eq!(f["response"]["status"], status, "fixture={fixture}");
    assert_eq!(f["response"]["body"]["success"], false);
}

/// TC-CTR-API-013-POS:角色不足的 message 文案。
///
/// 前端据此给用户提示,改了文案等于改了用户可见行为。
#[rstest::rstest]
#[case("api_error_insufficient_root")]
#[case("api_error_insufficient_admin")]
fn tc_ctr_api_013_pos_insufficient_privilege_message(#[case] fixture: &str) {
    let f = load_baseline(fixture);
    let msg = f["response"]["body"]["message"].as_str().unwrap_or_default();
    assert!(
        msg.contains("insufficient privileges") || msg.contains("权限"),
        "角色不足的提示文案应可识别,实际:{msg}"
    );
}

// ───────────── 分页(审查发现的 P1)─────────────

/// TC-CTR-API-020-POS:分页响应形状与页码基数。★
///
/// 守住审查发现的 P1 偏差:页码 1 起,不是 0 起。
#[test]
fn tc_ctr_api_020_pos_pagination_contract() {
    let b = baseline_body("api_token_list");
    let data = &b["data"];
    for k in ["items", "total", "page", "page_size"] {
        assert!(data.get(k).is_some(), "分页响应缺字段 {k}");
    }
    assert!(data.get("pageSize").is_none(), "字段名是 page_size,不是 pageSize");
    assert_eq!(
        data["page"], 1,
        "请求 p=1 时响应 page 应为 1(页码 1 起)"
    );
}

/// TC-CTR-API-021-POS:`p=0` 与 `p=1` 返回同一页。
///
/// new-api 把 `p<1` 归一为 1,客户端传 0 不应报错也不应返回空。
#[test]
fn tc_ctr_api_021_pos_page_zero_normalizes_to_one() {
    // TODO(实现): 分别用 p=0 与 p=1 调用,断言 items 完全相同
}

// ───────────── 敏感字段 ─────────────

/// TC-CTR-API-030-NEG:响应不得包含密码。
#[test]
fn tc_ctr_api_030_neg_no_password_in_response() {
    let b = baseline_body("api_user_self");
    let s = b.to_string();
    assert!(!s.contains("\"password\""), "用户响应不得包含 password 字段");
}

/// TC-CTR-API-031-POS:令牌列表 key 已脱敏。
#[test]
fn tc_ctr_api_031_pos_token_key_masked_in_list() {
    let b = baseline_body("api_token_list");
    let items = b["data"]["items"].as_array().expect("items 应为数组");
    for it in items {
        if let Some(k) = it.get("key").and_then(|v| v.as_str()) {
            assert!(
                k.contains('*'),
                "列表接口的 key 必须脱敏,实际:{k}"
            );
            assert!(k.len() <= 18, "脱敏后长度不应超过 18");
        }
    }
}

/// TC-CTR-API-032-NEG:渠道列表不得返回明文密钥。
#[test]
fn tc_ctr_api_032_neg_channel_key_not_exposed() {
    let b = baseline_body("api_channel_list");
    let items = b["data"]["items"].as_array().expect("items 应为数组");
    for it in items {
        if let Some(k) = it.get("key").and_then(|v| v.as_str()) {
            assert!(!k.starts_with("sk-"), "渠道列表不得返回可用的明文密钥");
        }
    }
}

// ───────────── 关键端点逐个 ─────────────

/// TC-CTR-API-040-POS:`/api/status` 的开关集合完整。
///
/// 前端 statusStore 是全站配置的唯一来源,缺字段会让整块功能在前端消失。
#[test]
fn tc_ctr_api_040_pos_status_feature_flags() {
    let b = baseline_body("api_status");
    let data = b["data"].as_object().expect("data 应为对象");
    // 录制基线里有哪些开关,实现就必须有哪些
    assert!(!data.is_empty(), "status 不应为空");
    // TODO(实现): 与 sea-weir 响应做字段集比对
}

/// TC-CTR-API-041-POS:`/api/pricing` 可匿名访问。
#[test]
fn tc_ctr_api_041_pos_pricing_allows_anonymous() {
    let f = load_baseline("api_pricing_anonymous");
    assert_eq!(f["response"]["status"], 200, "定价视图应支持匿名(TryUserAuth)");
}

/// TC-CTR-API-042-POS:`/api/user/self` 的额度字段类型为整数。
///
/// 额度是 i64,若序列化成浮点或字符串,前端换算会出错。
#[test]
fn tc_ctr_api_042_pos_quota_fields_are_integers() {
    let b = baseline_body("api_user_self");
    for k in ["quota", "used_quota", "request_count", "aff_quota"] {
        if let Some(v) = b["data"].get(k) {
            assert!(v.is_i64() || v.is_u64(), "{k} 应为整数,实际 {v:?}");
        }
    }
}
