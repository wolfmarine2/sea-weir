//! 中继面错误类型与出口格式。
//!
//! 见 doc/architecture/CONTRACTS.md §13 错误码表、ADR-008。

use crate::AppError;

/// 中继入口协议格式。决定错误出口形状与流式改写规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayFormat {
    /// `{"error":{"message","type","code"}}`
    OpenAi,
    /// `{"type":"error","error":{...}}`
    Claude,
    /// Gemini 原生错误体
    Gemini,
    /// `{"code","description","result"}`(code=30 → 429)
    Midjourney,
    /// 异步任务平台(Suno / 视频等)
    Task,
}

/// 中继面错误。承载重试与自动禁用判定所需的全部信息。
#[derive(Debug, Clone)]
pub struct NewApiError {
    pub status_code: u16,
    /// 契约错误码,见 CONTRACTS.md §13(如 `insufficient_quota`、`channel:invalid_key`)。
    pub error_code: String,
    /// OpenAI 错误 `type` 字段(如 `invalid_request_error`)。
    pub error_type: String,
    pub message: String,
    /// 适配器内部错误标记。为 true 时**不触发**渠道禁用与重试。
    pub local_error: bool,
    /// 命中「永不重试」规则(400/408/504/524、IsAlwaysSkipRetryCode 等)。
    pub skip_retry: bool,
    /// 是否写错误日志表(LogType::Error)。
    pub record_error_log: bool,
}

impl NewApiError {
    /// 是否属于渠道类错误(`channel:*`)—— 触发重试与自动禁用判定。
    pub fn is_channel_error(&self) -> bool {
        self.error_code.starts_with("channel:")
    }

    /// 按入口格式序列化为出口错误体。
    pub fn to_body(&self, format: RelayFormat) -> serde_json::Value {
        match format {
            RelayFormat::OpenAi => serde_json::json!({
                "error": {
                    "message": self.message,
                    "type": self.error_type,
                    "code": self.error_code,
                }
            }),
            RelayFormat::Claude => serde_json::json!({
                "type": "error",
                "error": {
                    "type": self.error_type,
                    "message": self.message,
                    "code": self.error_code,
                }
            }),
            RelayFormat::Gemini => serde_json::json!({
                "error": {
                    "code": self.status_code,
                    "message": self.message,
                    "status": self.error_code,
                }
            }),
            RelayFormat::Midjourney => {
                // MJ 契约:code 为整数,30 表示队列满(HTTP 429)。
                let code: i64 = self.error_code.parse().unwrap_or(0);
                serde_json::json!({
                    "code": code,
                    "description": self.message,
                    "result": serde_json::Value::Null,
                })
            }
            RelayFormat::Task => serde_json::json!({
                "code": self.error_code,
                "message": self.message,
            }),
        }
    }
}

impl From<AppError> for NewApiError {
    fn from(err: AppError) -> Self {
        let message = err.to_string();
        // 溯源:TEST-VECTORS §6 F1/F2、baseline/relay_error_*.json。
        // error.type 恒为 `new_api_error`;error.code 仅业务可识别错误填值。
        let (status_code, error_code): (u16, &str) = match &err {
            AppError::QuotaExceeded => (403, "insufficient_quota"),
            AppError::Unauthorized(_) => (401, ""),
            AppError::Forbidden(_) => (403, ""),
            AppError::BadRequest(_) | AppError::Biz(_) => (400, ""),
            AppError::NotFound(_) => (503, "model_not_found"),
            AppError::RateLimited { .. } => (429, "rate_limit_exceeded"),
            AppError::Upstream(_) => (502, ""),
            AppError::Database(_)
            | AppError::Cache(_)
            | AppError::Config(_)
            | AppError::Internal(_) => (500, ""),
        };
        let client_error = (400..500).contains(&status_code);
        NewApiError {
            status_code,
            error_code: error_code.to_string(),
            error_type: "new_api_error".to_string(),
            message,
            local_error: false,
            // 4xx 是确定性失败,重试无意义;5xx/网络类留给重试与禁用判定。
            skip_retry: client_error,
            record_error_log: !client_error,
        }
    }
}

/// panic 兜底错误体:`{"error":{"type":"new_api_panic","message":"…(request id: xxx)"}}`。
pub fn panic_body(request_id: &str) -> serde_json::Value {
    serde_json::json!({
        "error": {
            "type": "new_api_panic",
            "message": format!("服务器内部错误 (request id: {request_id})"),
        }
    })
}

#[cfg(test)]
mod tests {
    // TDD 入口(错误出口是契约兼容的重灾区,优先覆盖):
    // - [ ] to_body() 在 5 种 RelayFormat 下的字段形状逐一比对 new-api 实际响应
    // - [ ] is_channel_error() 对 `channel:invalid_key` 为 true、`insufficient_quota` 为 false
    // - [ ] AppError::QuotaExceeded → status 403 + error_code "insufficient_quota"
    // - [ ] local_error=true 时不参与重试与禁用判定
    // - [ ] MJ 格式 code=30 映射 HTTP 429
}
