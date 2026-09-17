//! 输入 token 估算。供 `POST /v1/messages/count_tokens` 使用。
//!
//! 说明:各家分词器不同(Claude 的分词器未公开),网关侧不做真实分词,只给出**近似估计**,
//! 供客户端做上下文预算/触发压缩。量级正确即可,不参与计费(计费以 usage 为准)。

use serde_json::Value;

/// 估算一段文本的 token 数(近似)。
///
/// 规则:ASCII 约 4 字符 1 token(向上取整);非 ASCII(中日韩等宽字符)约 1 字符 1 token。
/// 这与主流 BPE 分词器对英文/中文的量级基本吻合。
pub fn estimate_tokens(text: &str) -> i64 {
    let mut ascii = 0i64;
    let mut wide = 0i64;
    for c in text.chars() {
        if c.is_ascii() {
            ascii += 1;
        } else {
            wide += 1;
        }
    }
    (ascii + 3) / 4 + wide
}

/// 估算 Claude Messages 请求的输入 token 数。
///
/// 计入:`system`、`messages[].content`(文本/工具调用/工具结果/图片)、`tools` 定义,
/// 以及每条消息与每个工具的结构开销(近似)。空请求返回 1(token 数不应为 0)。
pub fn estimate_claude_input_tokens(body: &Value) -> i64 {
    let mut total = estimate_tokens_of(body.get("system"));

    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        for message in messages {
            // 每条消息的 role/分隔符等结构开销(近似 Anthropic 的每消息固定开销)。
            total += 4;
            total += estimate_tokens_of(message.get("content"));
        }
    }

    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        for tool in tools {
            total += 4;
            // 工具名/描述/JSON Schema 一并计入。
            total += estimate_tokens(&tool.to_string());
        }
    }

    total.max(1)
}

/// 估算 `system` / `content` 这类字段:字符串或内容块数组。
fn estimate_tokens_of(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::String(text)) => estimate_tokens(text),
        Some(Value::Array(blocks)) => blocks.iter().map(estimate_tokens_of_block).sum(),
        _ => 0,
    }
}

/// 单个内容块。未识别的块按其 JSON 文本估算,避免整块漏计。
fn estimate_tokens_of_block(block: &Value) -> i64 {
    match block.get("type").and_then(|t| t.as_str()) {
        Some("text") => block
            .get("text")
            .and_then(|t| t.as_str())
            .map(estimate_tokens)
            .unwrap_or(0),
        Some("tool_use") => {
            let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let input = block.get("input").cloned().unwrap_or(Value::Null);
            4 + estimate_tokens(name) + estimate_tokens(&input.to_string())
        }
        Some("tool_result") => estimate_tokens_of(block.get("content")),
        // 图片按固定量估算(真实开销取决于像素,客户端做预算时此量级足够)。
        Some("image") => 1600,
        _ => estimate_tokens(&block.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn estimates_ascii_and_wide_text() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1, "4 个 ASCII 字符约 1 token");
        assert_eq!(estimate_tokens("abcde"), 2, "向上取整");
        assert_eq!(estimate_tokens("你好"), 2, "中文约 1 字符 1 token");
        assert_eq!(estimate_tokens("你好world"), 2 + 2);
    }

    #[test]
    fn counts_system_messages_and_tools() {
        let body = json!({
            "model": "claude-x",
            "system": "你是助手",
            "messages": [
                {"role": "user", "content": "你好"},
                {"role": "assistant", "content": [
                    {"type": "text", "text": "查一下"},
                    {"type": "tool_use", "id": "t1", "name": "get_weather", "input": {"city": "上海"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "t1", "content": "晴"}
                ]}
            ],
            "tools": [
                {"name": "get_weather", "description": "查天气", "input_schema": {"type": "object"}}
            ]
        });
        let total = estimate_claude_input_tokens(&body);
        // system 4 + 每条消息 4*3 + 文本/工具内容 + 工具定义,应显著大于各部分之和的下限。
        assert!(total > 20, "应计入 system/消息/工具:{total}");
        assert!(estimate_claude_input_tokens(&body) > estimate_claude_input_tokens(&json!({"messages": []})));
    }

    #[test]
    fn empty_request_still_counts_one() {
        assert_eq!(estimate_claude_input_tokens(&json!({})), 1);
    }

    #[test]
    fn string_and_block_content_are_equivalent() {
        let as_string = json!({"messages": [{"role": "user", "content": "hello"}]});
        let as_blocks = json!({"messages": [{"role": "user", "content": [{"type": "text", "text": "hello"}]}]});
        assert_eq!(
            estimate_claude_input_tokens(&as_string),
            estimate_claude_input_tokens(&as_blocks)
        );
    }
}
