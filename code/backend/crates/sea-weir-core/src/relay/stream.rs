//! 流式转发。对应 C4 组件 `stream_pipe`。见 SEQ-004。
//!
//! ## SSE 管线
//! 行扫描(`data:` 行)→ **延迟一拍转发**(保留末行以判断 usage)
//! → 按入口格式改写(OpenAI 原样 / 转 Claude / 转 Gemini)
//! → 上游无 usage 而客户端要求 `include_usage` 时**注入估算 usage chunk**
//! → `[DONE]`。
//!
//! ## 看门狗
//! 空闲超时、ping 保活(默认 10s)、客户端断开检测。
//! 首字节时间与流状态记入日志。
//!
//! ## WebSocket(realtime)
//! 双向 pipe;`response.done` 事件触发增量扣费。

use sea_weir_types::{dto::Usage, AppError, AppResult, NewApiError, RelayFormat};

/// SSE 转发配置。
pub struct StreamConfig {
    pub ping_interval_secs: u64,
    pub idle_timeout_secs: u64,
    /// 客户端是否请求了 usage(OpenAI `stream_options.include_usage`)。
    pub include_usage: bool,
    pub format: RelayFormat,
}

/// 驱动一条 SSE 流,返回累计 usage。
pub async fn pipe_sse(_cfg: &StreamConfig) -> Result<Usage, NewApiError> {
    todo!("行扫描 + 延迟一拍 + 格式改写 + usage 注入 + ping 保活")
}

/// realtime WebSocket 双向转发。
pub async fn pipe_websocket() -> AppResult<()> {
    todo!("双向 pipe;response.done 事件增量扣费")
}

/// 单行 SSE 事件的格式改写。
///
/// - 空行 / `:` 注释行(ping 保活)→ `Ok(None)`,不进业务数据流
/// - `[DONE]` 标记原样保留
/// - 同格式(OpenAI → OpenAI)逐字节透传
/// - 跨格式(Claude / Gemini)重组事件结构;畸形 JSON 返回 `Err` 而非 panic
pub fn rewrite_chunk(line: &str, format: RelayFormat) -> AppResult<Option<String>> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() || trimmed.starts_with(':') {
        return Ok(None);
    }
    if !trimmed.starts_with("data:") {
        return Ok(None);
    }
    let payload = trimmed["data:".len()..].trim();
    if payload == "[DONE]" {
        return Ok(Some(trimmed.to_string()));
    }
    if format == RelayFormat::OpenAi {
        return Ok(Some(trimmed.to_string()));
    }

    let value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| AppError::BadRequest(format!("SSE chunk JSON 解析失败: {e}")))?;

    let rewritten = match format {
        RelayFormat::Claude => {
            let text = value
                .pointer("/choices/0/delta/content")
                .and_then(|v| v.as_str());
            let finished = value
                .pointer("/choices/0/finish_reason")
                .map(|v| !v.is_null())
                .unwrap_or(false);
            if finished {
                serde_json::json!({"type": "message_stop"})
            } else {
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {"type": "text_delta", "text": text.unwrap_or("")},
                })
            }
        }
        RelayFormat::Gemini => {
            let text = value
                .pointer("/choices/0/delta/content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({
                "candidates": [{"content": {"role": "model", "parts": [{"text": text}]}}]
            })
        }
        _ => return Ok(Some(trimmed.to_string())),
    };

    let event = rewritten
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("message");
    Ok(Some(format!("event: {event}\ndata: {rewritten}")))
}

#[cfg(test)]
mod tests {
    // TDD 入口(流式是最容易出兼容问题的地方,用录制的真实上游流做回放测试):
    // - [ ] 完整流:输入固定 SSE 序列 → 输出逐字节比对期望(黄金用例)
    // - [ ] 上游带 usage:原样透传,不重复注入
    // - [ ] 上游无 usage + include_usage=true:在 [DONE] 前注入一个 usage chunk
    // - [ ] 上游无 usage + include_usage=false:不注入
    // - [ ] 转 Claude:message_start/content_block_delta/message_delta 事件序列正确
    // - [ ] 空闲超时触发后连接关闭且记录流状态
    // - [ ] 客户端中途断开:停止拉取上游,已消费部分仍进入结算
    // - [ ] 上游流中途报错:错误按入口格式输出,且触发退款
    // - [ ] ping 事件不污染业务数据流
}
