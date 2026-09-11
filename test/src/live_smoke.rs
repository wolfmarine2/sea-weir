//! L4 端到端 — 部署后冒烟。用例文档:`cases/12-live-smoke.md`
//!
//! 针对**已部署实例**的黑盒测试。prod 环境只跑这一层(加审查与单元)。
//! 不做破坏性操作,不写业务数据。

use pretty_assertions::assert_eq;

/// 取 HTTP 客户端与基址。
///
/// 缺 `SEAWEIR_HTTP_BASE_URL` 时:严格模式(test 环境)panic,否则返回 None 让调用方跳过。
/// 不用 `require_dep!` 是因为该宏的 `return;` 与本函数的元组返回类型冲突。
fn client() -> Option<(reqwest::Client, String)> {
    match crate::common::TestEnv::get().http_base_url.as_ref() {
        Some(base) => Some((reqwest::Client::new(), base.clone())),
        None => {
            if crate::common::TestEnv::get().strict {
                panic!("STRICT=1 但缺少 SEAWEIR_HTTP_BASE_URL —— 拒绝跳过冒烟套件");
            }
            eprintln!("[skip] 缺少 SEAWEIR_HTTP_BASE_URL,跳过冒烟(dev 策略)");
            None
        }
    }
}

/// TC-E2E-SMK-001-POS:`/api/status` 可达且形状正确。
///
/// 最基础的存活探针:能返回合法 status 说明配置加载、数据库连接、
/// 进程内配置缓存都正常。
#[tokio::test]
async fn tc_e2e_smk_001_pos_status_endpoint_alive() {
    let Some((c, base)) = client() else { return };
    let resp = c.get(format!("{base}/api/status")).send().await.expect("请求失败");
    assert_eq!(resp.status(), 200);
    let b: serde_json::Value = resp.json().await.expect("响应非 JSON");
    assert_eq!(b["success"], true);
    assert!(b["data"].is_object());
}

/// TC-E2E-SMK-002-POS:未认证访问受保护端点返回 401。
///
/// 反向验证鉴权层确实挂上了 —— 如果这里返回 200,说明中间件漏挂,是重大安全问题。
#[tokio::test]
async fn tc_e2e_smk_002_pos_protected_endpoint_requires_auth() {
    let Some((c, base)) = client() else { return };
    let resp = c.get(format!("{base}/api/user/self")).send().await.expect("请求失败");
    assert_eq!(resp.status(), 401, "未认证访问 /api/user/self 必须 401");
}

/// TC-E2E-SMK-003-POS:中继面无令牌返回 401(OpenAI 错误格式)。
#[tokio::test]
async fn tc_e2e_smk_003_pos_relay_requires_token() {
    let Some((c, base)) = client() else { return };
    let resp = c
        .post(format!("{base}/v1/chat/completions"))
        .json(&serde_json::json!({"model":"gpt-4","messages":[]}))
        .send()
        .await
        .expect("请求失败");
    assert_eq!(resp.status(), 401);
    let b: serde_json::Value = resp.json().await.expect("响应非 JSON");
    assert!(b.get("error").is_some(), "中继面错误必须是 OpenAI 格式");
}

/// TC-E2E-SMK-004-POS:`/api/pricing` 匿名可访问。
#[tokio::test]
async fn tc_e2e_smk_004_pos_pricing_anonymous() {
    let Some((c, base)) = client() else { return };
    let resp = c.get(format!("{base}/api/pricing")).send().await.expect("请求失败");
    assert_eq!(resp.status(), 200);
}

/// TC-E2E-SMK-005-POS:SPA fallback 生效。
///
/// 深链接直接访问必须返回 index.html 而非 404,否则用户刷新页面就白屏。
#[tokio::test]
async fn tc_e2e_smk_005_pos_spa_fallback() {
    let Some((c, base)) = client() else { return };
    let resp = c.get(format!("{base}/console/token")).send().await.expect("请求失败");
    assert_eq!(resp.status(), 200, "SPA 深链接应回落 index.html");
    let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(ct.contains("text/html"), "应返回 HTML,实际 content-type={ct}");
}

/// TC-E2E-SMK-006-POS:SSE 路径未被缓冲。★
///
/// nginx 若没关 `proxy_buffering`,流式响应会被攒着一次性下发,
/// 表现为"客户端等很久然后一次性收到全部内容"。这是部署配置最常见的坑。
#[tokio::test]
async fn tc_e2e_smk_006_pos_sse_not_buffered() {
    let Some((_c, _base)) = client() else { return };
    // TODO(实现): 用有效令牌发起 stream=true 请求,
    //   断言首字节到达时间 < 5s 且后续 chunk 是逐步到达的(记录 chunk 间隔)
}

/// TC-E2E-SMK-007-POS:响应头含 request-id。
///
/// 线上排障依赖它把客户端报错与服务端日志对上。
#[tokio::test]
async fn tc_e2e_smk_007_pos_request_id_header() {
    let Some((c, base)) = client() else { return };
    let resp = c.get(format!("{base}/api/status")).send().await.expect("请求失败");
    assert!(
        resp.headers().keys().any(|k| k.as_str().contains("request-id")),
        "响应应回显 request-id 以便排障"
    );
}

/// TC-E2E-SMK-008-POS:健康检查不依赖鉴权。
#[tokio::test]
async fn tc_e2e_smk_008_pos_health_no_auth() {
    let Some((c, base)) = client() else { return };
    let resp = c.get(format!("{base}/api/status")).send().await.expect("请求失败");
    assert_eq!(resp.status(), 200, "探针端点不得要求鉴权,否则 K8s 探针会失败");
}
