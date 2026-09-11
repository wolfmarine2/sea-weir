//! L1 单元 — 路由注册完整性。用例文档:`cases/11-route-completeness.md`
//!
//! 与 CONTRACTS.md 附录 A 的 305 条清单对表。**漏注册一条就是一个接口不可用**,
//! 而这类缺陷靠人工 review 很难发现。
//!
//! 溯源:CONTRACTS.md 附录 A;TEST-VECTORS.md §11

use pretty_assertions::assert_eq;

/// 附录 A 的分面计数。改动此处必须同步改文档,反之亦然。
mod expected {
    pub const ADMIN: usize = 236;
    pub const RELAY: usize = 54;
    pub const VIDEO: usize = 11;
    pub const DASHBOARD: usize = 4;
    pub const TOTAL: usize = ADMIN + RELAY + VIDEO + DASHBOARD; // 305
}

/// TC-UNI-RTE-001-POS:总路由数 == 305。★
///
/// 这条断言把文档与实现绑在一起:任何一方改了而另一方没跟上,CI 就红。
#[test]
fn tc_uni_rte_001_pos_total_route_count() {
    assert_eq!(expected::TOTAL, 305, "附录 A 分面计数之和应为 305");

    // TODO(实现): 从 router::build() 产出的 Router 提取已注册路由并计数。
    //   axum 未暴露路由枚举 API,实现时的可选方案:
    //   1. 注册时同步维护一份 RouteManifest(推荐,顺带可做角色闸门遍历断言)
    //   2. 用 axum-test 逐条打表探测
    let actual = sea_weir_server::router::registered_route_count();
    assert_eq!(actual, expected::TOTAL, "注册路由数与附录 A 清单不符(漏注册或重复注册)");
}

/// TC-UNI-RTE-002-POS:分面计数逐一对表。
#[test]
fn tc_uni_rte_002_pos_per_surface_count() {
    let m = sea_weir_server::router::route_manifest();
    assert_eq!(m.count_by_surface("api"), expected::ADMIN, "管理面");
    assert_eq!(m.count_by_surface("relay"), expected::RELAY, "中继面");
    assert_eq!(m.count_by_surface("video"), expected::VIDEO, "视频任务面");
    assert_eq!(m.count_by_surface("dashboard"), expected::DASHBOARD, "兼容面");
}

/// TC-UNI-RTE-003-POS:MJ 双前缀注册。★
///
/// 同一组 16 条须同时注册在 `/mj/**` 与 `/:mode/mj/**`。
/// 漏掉 `:mode` 变体会导致部分存量客户端全部 404。
#[test]
fn tc_uni_rte_003_pos_mj_dual_prefix() {
    let m = sea_weir_server::router::route_manifest();
    let plain = m.paths_with_prefix("/mj/");
    let moded = m.paths_with_prefix("/:mode/mj/");
    assert_eq!(plain.len(), 16, "/mj/** 应有 16 条");
    assert_eq!(moded.len(), 16, "/:mode/mj/** 应有 16 条(双前缀注册)");

    let strip = |v: Vec<String>, p: &str| -> Vec<String> {
        let mut r: Vec<_> = v.iter().map(|s| s.replace(p, "")).collect();
        r.sort();
        r
    };
    assert_eq!(
        strip(plain, "/mj/"),
        strip(moded, "/:mode/mj/"),
        "两个前缀下的路径集合必须完全相同"
    );
}

/// TC-UNI-RTE-004-POS:受保护路由都挂了角色闸门。★
///
/// 遍历断言,防止新增端点时漏挂中间件 —— 这类漏挂是最典型的越权漏洞成因。
#[test]
fn tc_uni_rte_004_pos_every_protected_route_has_gate() {
    let m = sea_weir_server::router::route_manifest();
    let ungated: Vec<_> = m
        .routes()
        .iter()
        .filter(|r| r.requires_auth && r.auth_layer.is_none())
        .map(|r| format!("{} {}", r.method, r.path))
        .collect();
    assert!(ungated.is_empty(), "以下受保护路由未挂角色闸门(越权风险):{ungated:?}");
}

/// TC-UNI-RTE-005-POS:取渠道密钥端点须同时挂 RootAuth 与二次验证。
///
/// 这是全系统权限最高的端点,单挂 RootAuth 不够。
#[test]
fn tc_uni_rte_005_pos_channel_key_endpoint_double_guarded() {
    let m = sea_weir_server::router::route_manifest();
    let r = m
        .find("POST", "/api/channel/:id/key")
        .expect("取渠道密钥端点必须存在");
    assert_eq!(r.auth_layer.as_deref(), Some("RootAuth"));
    assert!(
        r.extra_layers.iter().any(|l| l == "SecureVerificationRequired"),
        "取渠道密钥必须要求敏感操作凭证"
    );
}

/// TC-UNI-RTE-006-POS:公开端点不得挂 auth 层。
///
/// 反向断言:setup / status / 支付回调挂了 auth 会导致首装与支付回调失败。
#[rstest::rstest]
#[case("GET",  "/api/status")]
#[case("GET",  "/api/setup")]
#[case("POST", "/api/setup")]
#[case("GET",  "/api/notice")]
#[case("POST", "/api/stripe/webhook")]
#[case("POST", "/api/user/epay/notify")]
fn tc_uni_rte_006_pos_public_routes_have_no_auth(#[case] method: &str, #[case] path: &str) {
    let m = sea_weir_server::router::route_manifest();
    let r = m.find(method, path).unwrap_or_else(|| panic!("路由不存在:{method} {path}"));
    assert!(r.auth_layer.is_none(), "{method} {path} 是公开端点,不应挂 auth");
}

/// TC-UNI-RTE-007-POS:11 条占位端点存在且不消耗额度。
///
/// 注意(录制实测):这 11 条有**两种**实际行为 ——
/// POST 类带 body 能到达 handler,返回 501 `api_not_implemented`;
/// GET 类无 body,在 Distribute 中间件解析请求体时就 400 了,根本到不了 handler。
/// sea-weir 需保持两种行为一致,见 TEST-VECTORS §6。
#[test]
fn tc_uni_rte_007_pos_not_implemented_placeholders() {
    let m = sea_weir_server::router::route_manifest();
    let placeholders = [
        ("POST", "/v1/images/variations"),
        ("GET", "/v1/files"),
        ("POST", "/v1/files"),
        ("GET", "/v1/files/:id"),
        ("GET", "/v1/files/:id/content"),
        ("DELETE", "/v1/files/:id"),
        ("POST", "/v1/fine-tunes"),
        ("GET", "/v1/fine-tunes"),
        ("GET", "/v1/fine-tunes/:id"),
        ("POST", "/v1/fine-tunes/:id/cancel"),
        ("GET", "/v1/fine-tunes/:id/events"),
    ];
    assert_eq!(placeholders.len(), 11);
    for (method, path) in placeholders {
        let r = m.find(method, path).unwrap_or_else(|| panic!("占位端点缺失:{method} {path}"));
        assert!(r.is_placeholder, "{method} {path} 应标记为占位端点");
    }
}

/// TC-UNI-RTE-008-POS:`/pg/chat/completions` 走 UserAuth 而非 TokenAuth。
///
/// Playground 是登录用户的会话态调试,不接受 sk-token。挂错会导致越权调用。
#[test]
fn tc_uni_rte_008_pos_playground_uses_user_auth() {
    let m = sea_weir_server::router::route_manifest();
    let r = m.find("POST", "/pg/chat/completions").expect("Playground 端点必须存在");
    assert_eq!(r.auth_layer.as_deref(), Some("UserAuth"));
}
