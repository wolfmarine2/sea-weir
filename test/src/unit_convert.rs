//! L1 单元 — 格式转换。用例文档:`cases/06-unit-convert-stream.md`
//!
//! 纯函数 + 表驱动。转换错误会直接表现为"上游拒绝请求"或"客户端解析失败",
//! 且往往只在特定字段组合下触发,必须表驱动全覆盖。

use sea_weir_core::relay::convert::{convert_request, convert_response, RequestConversionChain};
use sea_weir_types::{dto::RelayRequest, RelayFormat};

fn req(body: serde_json::Value) -> RelayRequest {
    RelayRequest {
        model: body["model"].as_str().unwrap_or("gpt-4").to_string(),
        stream: body["stream"].as_bool().unwrap_or(false),
        raw: body,
    }
}

/// TC-UNI-CVT-001-POS:OpenAI → Claude 请求,system 消息提取到顶层。
///
/// Claude 的 system 不在 messages 数组里,而是顶层 `system` 字段。
/// 漏了会导致 system prompt 静默丢失 —— 最难发现的一类 bug。
#[test]
fn tc_uni_cvt_001_pos_openai_to_claude_extracts_system() {
    let r = req(serde_json::json!({
        "model": "claude-3-5-sonnet",
        "messages": [
            {"role": "system", "content": "You are helpful."},
            {"role": "user", "content": "hi"}
        ]
    }));
    let mut chain = RequestConversionChain::default();
    let out = convert_request(&r, RelayFormat::OpenAi, RelayFormat::Claude, &mut chain)
        .expect("转换应成功");

    assert_eq!(out["system"], "You are helpful.", "system 必须提取到顶层");
    let msgs = out["messages"].as_array().expect("messages 应为数组");
    assert_eq!(msgs.len(), 1, "system 已提走,messages 只剩 user");
    assert_eq!(msgs[0]["role"], "user");
}

/// TC-UNI-CVT-002-POS:OpenAI → Claude 必须补 max_tokens。
///
/// Claude API 的 `max_tokens` 是**必填**,OpenAI 侧可缺省。不补会被上游 400。
#[test]
fn tc_uni_cvt_002_pos_openai_to_claude_fills_max_tokens() {
    let r = req(serde_json::json!({
        "model": "claude-3-5-sonnet",
        "messages": [{"role": "user", "content": "hi"}]
    }));
    let mut chain = RequestConversionChain::default();
    let out = convert_request(&r, RelayFormat::OpenAi, RelayFormat::Claude, &mut chain).unwrap();
    assert!(out.get("max_tokens").is_some(), "Claude 的 max_tokens 必填,须补默认值");
}

/// TC-UNI-CVT-003-POS:OpenAI → Gemini 的 role 映射(assistant → model)。
#[test]
fn tc_uni_cvt_003_pos_openai_to_gemini_role_mapping() {
    let r = req(serde_json::json!({
        "model": "gemini-1.5-pro",
        "messages": [
            {"role": "user", "content": "hi"},
            {"role": "assistant", "content": "hello"}
        ]
    }));
    let mut chain = RequestConversionChain::default();
    let out = convert_request(&r, RelayFormat::OpenAi, RelayFormat::Gemini, &mut chain).unwrap();

    let contents = out["contents"].as_array().expect("Gemini 用 contents 而非 messages");
    assert_eq!(contents[0]["role"], "user");
    assert_eq!(contents[1]["role"], "model", "assistant 必须映射为 model");
    assert!(contents[0]["parts"].is_array(), "Gemini 用 parts 承载内容");
}

/// TC-UNI-CVT-004-POS:Claude → OpenAI 响应,content blocks 合并为字符串。
#[test]
fn tc_uni_cvt_004_pos_claude_to_openai_merges_content_blocks() {
    let body = serde_json::json!({
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "text", "text": "Hello, "},
            {"type": "text", "text": "world!"}
        ],
        "usage": {"input_tokens": 10, "output_tokens": 5}
    });
    let out = convert_response(&body, RelayFormat::Claude, RelayFormat::OpenAi).unwrap();
    assert_eq!(out["choices"][0]["message"]["content"], "Hello, world!");
    assert_eq!(out["usage"]["prompt_tokens"], 10);
    assert_eq!(out["usage"]["completion_tokens"], 5);
}

/// TC-UNI-CVT-005-POS:往返转换语义不丢失。
///
/// OpenAI → Claude → OpenAI 后,messages 的角色与内容应保持一致。
#[test]
fn tc_uni_cvt_005_pos_roundtrip_preserves_semantics() {
    let original = serde_json::json!({
        "model": "claude-3-5-sonnet",
        "messages": [
            {"role": "user", "content": "question"},
            {"role": "assistant", "content": "answer"},
            {"role": "user", "content": "follow up"}
        ]
    });
    let mut chain = RequestConversionChain::default();
    let to_claude =
        convert_request(&req(original.clone()), RelayFormat::OpenAi, RelayFormat::Claude, &mut chain)
            .unwrap();
    let back =
        convert_request(&req(to_claude), RelayFormat::Claude, RelayFormat::OpenAi, &mut chain)
            .unwrap();

    let orig_msgs = original["messages"].as_array().unwrap();
    let back_msgs = back["messages"].as_array().unwrap();
    assert_eq!(back_msgs.len(), orig_msgs.len(), "往返后消息条数应一致");
}

/// TC-UNI-CVT-006-POS:转换链记录跳数。
///
/// 链路记录用于日志审计与计费语义判定(ADR-006),漏记会让计费走错分支。
#[test]
fn tc_uni_cvt_006_pos_conversion_chain_records_steps() {
    let mut chain = RequestConversionChain::default();
    let r = req(serde_json::json!({"model": "x", "messages": []}));
    let _ = convert_request(&r, RelayFormat::OpenAi, RelayFormat::Claude, &mut chain);
    assert_eq!(chain.steps.len(), 1);
    let _ = convert_request(&r, RelayFormat::Claude, RelayFormat::Gemini, &mut chain);
    assert_eq!(chain.steps.len(), 2, "两跳转换应记录两条");
}

/// TC-UNI-CVT-007-BND:同格式转换是恒等的,不应改写。
#[test]
fn tc_uni_cvt_007_bnd_same_format_is_identity() {
    let body = serde_json::json!({"model": "gpt-4", "messages": [{"role":"user","content":"hi"}]});
    let mut chain = RequestConversionChain::default();
    let out = convert_request(&req(body.clone()), RelayFormat::OpenAi, RelayFormat::OpenAi, &mut chain)
        .unwrap();
    assert_eq!(out, body, "OpenAI → OpenAI 应原样返回");
}

/// TC-UNI-CVT-008-POS:多模态内容块转换。
#[test]
fn tc_uni_cvt_008_pos_multimodal_content() {
    let r = req(serde_json::json!({
        "model": "claude-3-5-sonnet",
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "what is this"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBORw0KG"}}
            ]
        }]
    }));
    let mut chain = RequestConversionChain::default();
    let out = convert_request(&r, RelayFormat::OpenAi, RelayFormat::Claude, &mut chain).unwrap();
    let content = out["messages"][0]["content"].as_array().expect("多模态应保持数组");
    assert!(
        content.iter().any(|c| c["type"] == "image"),
        "OpenAI 的 image_url 应转为 Claude 的 image 块"
    );
}
