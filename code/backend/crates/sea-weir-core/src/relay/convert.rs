//! 格式转换。对应 C4 组件 `format_convert`。
//!
//! OpenAI ⇄ Claude ⇄ Gemini 的请求/响应/流式改写集中于此。
//! `RequestConversionChain` 记录转换链路并决定计费语义(ADR-006)。

use sea_weir_types::{dto::RelayRequest, AppError, AppResult, RelayFormat};
use serde_json::{json, Value};

/// 转换链路记录。用于日志审计与计费语义判定。
#[derive(Debug, Clone, Default)]
pub struct RequestConversionChain {
    pub steps: Vec<(RelayFormat, RelayFormat)>,
}

/// 请求体转换。
pub fn convert_request(
    req: &RelayRequest,
    from: RelayFormat,
    to: RelayFormat,
    chain: &mut RequestConversionChain,
) -> AppResult<serde_json::Value> {
    // 同格式转换恒等,不计入转换链。
    if from == to {
        return Ok(req.raw.clone());
    }
    chain.steps.push((from, to));

    Ok(match (from, to) {
        (RelayFormat::OpenAi, RelayFormat::Claude) => openai_to_claude(&req.raw),
        (RelayFormat::Claude, RelayFormat::OpenAi) => claude_to_openai(&req.raw),
        (RelayFormat::OpenAi, RelayFormat::Gemini) => openai_to_gemini(&req.raw),
        // 无直连转换的组合经 OpenAI 中转,保持语义。
        (RelayFormat::Claude, RelayFormat::Gemini) => openai_to_gemini(&claude_to_openai(&req.raw)),
        (RelayFormat::Gemini, RelayFormat::OpenAi) => gemini_to_openai(&req.raw),
        (RelayFormat::Gemini, RelayFormat::Claude) => openai_to_claude(&gemini_to_openai(&req.raw)),
        _ => req.raw.clone(),
    })
}

/// 非流式响应体转换。
pub fn convert_response(
    body: &serde_json::Value,
    from: RelayFormat,
    to: RelayFormat,
) -> AppResult<serde_json::Value> {
    if from == to {
        return Ok(body.clone());
    }
    Ok(match (from, to) {
        (RelayFormat::Claude, RelayFormat::OpenAi) => claude_response_to_openai(body),
        (RelayFormat::OpenAi, RelayFormat::Claude) => openai_response_to_claude(body),
        _ => body.clone(),
    })
}

// ───────────────────────── 请求转换 ─────────────────────────

/// OpenAI → Claude:system 提取到顶层、补 max_tokens、多模态块转换。
pub(crate) fn openai_to_claude(raw: &Value) -> Value {
    let mut out = raw.clone();
    let obj = match out.as_object_mut() {
        Some(o) => o,
        None => return out,
    };

    let mut systems: Vec<String> = Vec::new();
    let mut messages: Vec<Value> = Vec::new();
    if let Some(list) = obj.get("messages").and_then(|m| m.as_array()) {
        for msg in list {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            if role == "system" {
                if let Some(text) = content_to_text(msg.get("content")) {
                    systems.push(text);
                }
                continue;
            }
            let mut m = msg.clone();
            if let Some(content) = m.get("content") {
                let converted = openai_content_to_claude(content);
                if let Some(o) = m.as_object_mut() {
                    o.insert("content".into(), converted);
                }
            }
            messages.push(m);
        }
    }
    obj.insert("messages".into(), Value::Array(messages));
    if !systems.is_empty() {
        obj.insert("system".into(), Value::String(systems.join("\n")));
    }
    // Claude 的 max_tokens 必填。
    if !obj.contains_key("max_tokens") {
        obj.insert("max_tokens".into(), json!(4096));
    }
    // Claude 不认 OpenAI 的 stream_options。
    obj.remove("stream_options");
    out
}

/// Claude → OpenAI:顶层 system 还原为 system 消息。
fn claude_to_openai(raw: &Value) -> Value {
    let mut out = raw.clone();
    let obj = match out.as_object_mut() {
        Some(o) => o,
        None => return out,
    };

    let mut messages: Vec<Value> = Vec::new();
    if let Some(system) = obj.remove("system") {
        if let Some(text) = content_to_text(Some(&system)) {
            messages.push(json!({"role": "system", "content": text}));
        }
    }
    if let Some(list) = obj.get("messages").and_then(|m| m.as_array()) {
        for msg in list {
            let mut m = msg.clone();
            if let Some(content) = m.get("content") {
                let converted = claude_content_to_openai(content);
                if let Some(o) = m.as_object_mut() {
                    o.insert("content".into(), converted);
                }
            }
            messages.push(m);
        }
    }
    obj.insert("messages".into(), Value::Array(messages));
    out
}

/// OpenAI → Gemini:`contents` + `parts`,assistant → model。
fn openai_to_gemini(raw: &Value) -> Value {
    let mut contents: Vec<Value> = Vec::new();
    if let Some(list) = raw.get("messages").and_then(|m| m.as_array()) {
        for msg in list {
            let role = match msg.get("role").and_then(|r| r.as_str()).unwrap_or("user") {
                "assistant" => "model",
                "system" => "user",
                other => other,
            };
            let parts = openai_content_to_gemini_parts(msg.get("content"));
            contents.push(json!({"role": role, "parts": parts}));
        }
    }
    let mut out = json!({"contents": contents});
    if let Some(model) = raw.get("model") {
        out["model"] = model.clone();
    }
    out
}

/// Gemini → OpenAI:`contents` → messages(用于 Gemini 入口 + OpenAI 上游的组合)。
fn gemini_to_openai(raw: &Value) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    if let Some(list) = raw.get("contents").and_then(|c| c.as_array()) {
        for item in list {
            let role = match item.get("role").and_then(|r| r.as_str()).unwrap_or("user") {
                "model" => "assistant",
                other => other,
            };
            let text = item
                .get("parts")
                .and_then(|p| p.as_array())
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            messages.push(json!({"role": role, "content": text}));
        }
    }
    let mut out = json!({"messages": messages});
    if let Some(model) = raw.get("model") {
        out["model"] = model.clone();
    }
    out
}

// ───────────────────────── 内容块转换 ─────────────────────────

fn content_to_text(content: Option<&Value>) -> Option<String> {
    match content? {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("");
            Some(text)
        }
        _ => None,
    }
}

/// OpenAI content → Claude content(字符串保持字符串,数组逐块映射)。
fn openai_content_to_claude(content: &Value) -> Value {
    match content {
        Value::Array(items) => Value::Array(items.iter().map(openai_block_to_claude).collect()),
        other => other.clone(),
    }
}

fn openai_block_to_claude(block: &Value) -> Value {
    match block.get("type").and_then(|t| t.as_str()) {
        Some("image_url") => {
            let url = block
                .pointer("/image_url/url")
                .and_then(|u| u.as_str())
                .unwrap_or("");
            let source = if let Some(rest) = url.strip_prefix("data:") {
                // data:<media_type>;base64,<data>
                let (meta, data) = rest.split_once(',').unwrap_or(("image/png;base64", ""));
                let media_type = meta.split(';').next().unwrap_or("image/png");
                json!({"type": "base64", "media_type": media_type, "data": data})
            } else {
                json!({"type": "url", "url": url})
            };
            json!({"type": "image", "source": source})
        }
        _ => block.clone(),
    }
}

/// Claude content → OpenAI content(纯文本块合并为字符串)。
fn claude_content_to_openai(content: &Value) -> Value {
    match content {
        Value::Array(items) => {
            if items
                .iter()
                .all(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            {
                let text = items
                    .iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("");
                Value::String(text)
            } else {
                content.clone()
            }
        }
        other => other.clone(),
    }
}

fn openai_content_to_gemini_parts(content: Option<&Value>) -> Vec<Value> {
    match content {
        Some(Value::String(s)) => vec![json!({"text": s})],
        Some(Value::Array(items)) => items
            .iter()
            .map(|b| match b.get("type").and_then(|t| t.as_str()) {
                Some("image_url") => {
                    let url = b
                        .pointer("/image_url/url")
                        .and_then(|u| u.as_str())
                        .unwrap_or("");
                    json!({"file_data": {"file_uri": url}})
                }
                _ => json!({"text": b.get("text").and_then(|t| t.as_str()).unwrap_or("")}),
            })
            .collect(),
        _ => vec![json!({"text": ""})],
    }
}

// ───────────────────────── 响应转换 ─────────────────────────

fn claude_response_to_openai(body: &Value) -> Value {
    let text = content_to_text(body.get("content")).unwrap_or_default();
    let input = body
        .pointer("/usage/input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output = body
        .pointer("/usage/output_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let finish = body.get("stop_reason").cloned().unwrap_or(Value::Null);
    json!({
        "id": body.get("id").cloned().unwrap_or(json!("chatcmpl-seaweir")),
        "object": "chat.completion",
        "model": body.get("model").cloned().unwrap_or(Value::Null),
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": text},
            "finish_reason": finish,
        }],
        "usage": {
            "prompt_tokens": input,
            "completion_tokens": output,
            "total_tokens": input + output,
        }
    })
}

fn openai_response_to_claude(body: &Value) -> Value {
    let text = body
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let input = body
        .pointer("/usage/prompt_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output = body
        .pointer("/usage/completion_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    json!({
        "id": body.get("id").cloned().unwrap_or(json!("msg_seaweir")),
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": text}],
        "model": body.get("model").cloned().unwrap_or(Value::Null),
        "stop_reason": body.pointer("/choices/0/finish_reason").cloned().unwrap_or(Value::Null),
        "usage": {"input_tokens": input, "output_tokens": output},
    })
}

#[allow(dead_code)]
fn err(msg: impl Into<String>) -> AppError {
    AppError::BadRequest(msg.into())
}

#[cfg(test)]
mod tests {
    // TDD 入口(纯函数,最适合表驱动测试):
    // - [ ] OpenAI → Claude 请求:system 消息提取、max_tokens 必填、tool 定义映射
    // - [ ] OpenAI → Gemini 请求:contents/parts 结构、role 映射(assistant→model)
    // - [ ] Claude → OpenAI 响应:content blocks 合并为 message.content
    // - [ ] 往返转换幂等性:OpenAI → Claude → OpenAI 语义不丢失
    // - [ ] 多模态(图片/音频)内容块的转换
    // - [ ] 转换链记录:两跳转换时 steps.len() == 2
}
