//! L3 契约 — 中继面。用例文档:`cases/10-contract-relay.md`
//!
//! 中继面的契约对象是**上游原生协议**,不是我们自己的 DTO。
//! 判定基准同样是录制的 new-api 响应。

use crate::common::load_baseline;
use pretty_assertions::assert_eq;

/// TC-CTR-RLY-001-POS:OpenAI 非流式响应结构。
#[test]
fn tc_ctr_rly_001_pos_openai_chat_completion_shape() {
    let f = load_baseline("relay_openai_chat_nonstream");
    let b = &f["response"]["body"];
    for k in ["id", "object", "created", "model", "choices", "usage"] {
        assert!(b.get(k).is_some(), "OpenAI 响应缺字段 {k}");
    }
    assert_eq!(b["object"], "chat.completion");
    let ch = b["choices"].as_array().expect("choices 应为数组");
    assert!(ch[0].get("message").is_some());
    assert!(ch[0].get("finish_reason").is_some());
}

/// TC-CTR-RLY-002-POS:usage 字段完整。
///
/// 客户端用 usage 做成本统计,缺字段会让统计为 0。
#[test]
fn tc_ctr_rly_002_pos_usage_fields() {
    let f = load_baseline("relay_openai_chat_nonstream");
    let u = &f["response"]["body"]["usage"];
    for k in ["prompt_tokens", "completion_tokens", "total_tokens"] {
        assert!(u.get(k).is_some(), "usage 缺字段 {k}");
        assert!(u[k].is_i64() || u[k].is_u64(), "{k} 应为整数");
    }
}

/// TC-CTR-RLY-003-POS:Claude 原生响应结构与 usage 字段名。★
///
/// 该基线是「OpenAI 上游 → Claude 出口」的转换结果,同时验证了转换正确性。
///
/// **关键**:Claude 分档缓存的字段名是 `claude_cache_creation_5_m_tokens`
/// (`5` 与 `m` 之间**有下划线**),不是直觉上的 `5m`。这是 Go 侧驼峰转 snake 的
/// 产物,录制实测确认。写成 `5m` 会让该字段在反序列化时恒为 0 —— 静默少算钱。
#[test]
fn tc_ctr_rly_003_pos_claude_message_shape() {
    let f = load_baseline("relay_claude_messages");
    let b = &f["response"]["body"];
    assert_eq!(b["type"], "message");
    assert_eq!(b["role"], "assistant");
    assert!(b["content"].is_array(), "Claude 的 content 是块数组");
    assert_eq!(b["content"][0]["type"], "text");
    assert!(b.get("stop_reason").is_some(), "Claude 用 stop_reason 而非 finish_reason");

    let u = &b["usage"];
    for k in [
        "input_tokens",
        "output_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
        "claude_cache_creation_5_m_tokens",
        "claude_cache_creation_1_h_tokens",
    ] {
        assert!(u.get(k).is_some(), "Claude usage 缺字段 {k}");
    }
    assert!(
        u.get("claude_cache_creation_5m_tokens").is_none(),
        "字段名是 5_m 不是 5m —— 写错会静默少算缓存写入费用"
    );
}

/// TC-CTR-RLY-004-POS:**分发阶段的错误一律 OpenAI 格式**,不按入口格式分派。★
///
/// 录制实测校正了一个重要误解。首版按"错误体形状按入口格式分派"写,
/// 但实测三个入口(`/v1/chat/completions`、`/v1/messages`、`/mj/submit/imagine`)
/// 在**选路失败**时返回的都是 OpenAI 形状 `{"error":{"code","message","type"}}`:
///
/// | 入口 | 期望(首版) | 实测 |
/// |---|---|---|
/// | `/v1/chat/completions` | OpenAI | OpenAI ✔ |
/// | `/v1/messages` | `{"type":"error",...}` | **OpenAI** ✘ |
/// | `/mj/submit/imagine` | `{"code","description","result"}` | **OpenAI** ✘ |
///
/// 原因:选路失败发生在 **Distribute 中间件**,此时还没进入各格式的 relay handler,
/// 出口统一走 `abortWithOpenAiMessage`。格式分派只在 handler 内部的业务错误上生效。
///
/// 溯源:`baseline/relay_error_{openai,claude,mj}.json`
#[rstest::rstest]
#[case("relay_error_openai")]
#[case("relay_error_claude")]
#[case("relay_error_mj")]
fn tc_ctr_rly_004_pos_distribute_errors_are_always_openai_shape(#[case] fixture: &str) {
    let f = load_baseline(fixture);
    let b = &f["response"]["body"];
    let e = b
        .get("error")
        .unwrap_or_else(|| panic!("{fixture}:分发阶段错误应为 OpenAI 形状的顶层 error"));
    for k in ["message", "type", "code"] {
        assert!(e.get(k).is_some(), "{fixture}:OpenAI 错误缺 {k}");
    }
    assert_eq!(e["type"], "new_api_error", "{fixture}:type 恒为 new_api_error");
    assert_eq!(f["response"]["status"], 503, "{fixture}:无可用渠道是 503");
}

/// TC-CTR-RLY-005-POS:handler 内部业务错误才按入口格式分派。
///
/// 尚未录制 —— 需要一个能真正到达 MJ / Claude handler 的可用渠道。
/// 待补录 fixture:`relay_error_mj_business`(MJ 格式 `{"code","description","result"}`,
/// `code == 30` 对应 HTTP 429)、`relay_error_claude_business`(`{"type":"error",...}`)。
#[test]
#[ignore = "待录制:需要可用的 MJ / Claude 渠道才能触达 handler 内部错误"]
fn tc_ctr_rly_005_pos_handler_errors_use_entry_format() {
    unimplemented!("补录 relay_error_mj_business / relay_error_claude_business 后启用")
}

/// TC-CTR-RLY-006-POS:`/v1/models` 返回令牌可用模型。
#[test]
fn tc_ctr_rly_006_pos_models_list_shape() {
    let f = load_baseline("relay_models_list");
    let b = &f["response"]["body"];
    assert_eq!(b["object"], "list");
    let data = b["data"].as_array().expect("data 应为数组");
    if let Some(first) = data.first() {
        for k in ["id", "object", "owned_by"] {
            assert!(first.get(k).is_some(), "模型条目缺 {k}");
        }
    }
}

/// TC-CTR-RLY-007-POS:dashboard 计费兼容端点。
#[test]
fn tc_ctr_rly_007_pos_dashboard_billing() {
    let f = load_baseline("dashboard_billing_subscription");
    let b = &f["response"]["body"];
    // OpenAI dashboard 兼容:客户端(如某些成本监控工具)按这个形状解析
    for k in ["object", "hard_limit_usd", "soft_limit_usd", "system_hard_limit_usd"] {
        assert!(b.get(k).is_some(), "billing/subscription 缺字段 {k}");
    }
}

/// TC-CTR-RLY-010-POS:SSE 流的事件序列。★
///
/// 逐 chunk 比对录制的流。流式契约错误的表现是"客户端卡住"而非报错,
/// 必须用录制回放守住。
#[test]
fn tc_ctr_rly_010_pos_sse_event_sequence() {
    let f = load_baseline("relay_openai_chat_stream");
    let chunks = f["response"]["chunks"].as_array().expect("流式基线应含 chunks 数组");

    assert!(!chunks.is_empty(), "录制的流不应为空");
    let last = chunks.last().unwrap().as_str().unwrap_or_default();
    assert!(last.contains("[DONE]"), "OpenAI 流必须以 data: [DONE] 结尾");

    // 首个 chunk 应含 role
    let first = chunks.first().unwrap().as_str().unwrap_or_default();
    assert!(first.starts_with("data: "), "每个事件以 'data: ' 开头");

    // TODO(实现): 用同样输入调 sea-weir,逐 chunk 比对(允许 id/created 差异)
}

/// TC-CTR-RLY-011-POS:占位端点(POST)返回 501 + `api_not_implemented`。
#[test]
fn tc_ctr_rly_011_pos_not_implemented_post() {
    let f = load_baseline("relay_not_implemented");
    assert_eq!(f["response"]["status"], 501, "POST 类占位端点是 501");
    let e = f["response"]["body"]["error"].clone();
    assert_eq!(e["code"], "api_not_implemented");
    assert_eq!(e["type"], "new_api_error");
    assert!(e.get("param").is_some(), "OpenAI 错误体含 param 字段");
    // TODO(实现): 断言调用前后用户额度不变
}

/// TC-CTR-RLY-012-POS:占位端点(GET)在 Distribute 解析 body 时就 400。★
///
/// 不是设计如此,而是 new-api 的中间件顺序导致 —— GET 无 body,
/// Distribute 解析失败直接 400,`RelayNotImplemented` handler 根本执行不到。
/// 契约兼容要求 sea-weir 复刻这个行为(客户端可能已经依赖 400 做分支)。
#[test]
fn tc_ctr_rly_012_pos_not_implemented_get_is_400() {
    let f = load_baseline("relay_not_implemented_get");
    assert_eq!(f["response"]["status"], 400, "GET 类占位端点被 Distribute 拦成 400");
}
