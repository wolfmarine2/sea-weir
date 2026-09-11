//! 中继面 DTO。
//!
//! 注意:本模块字段遵循**各上游原生协议**,不套用管理面命名规则。

use serde::{Deserialize, Serialize};

/// 中继请求的统一内部表示。适配器从此结构转出上游各自的请求体。
#[derive(Debug, Clone)]
pub struct RelayRequest {
    pub model: String,
    pub stream: bool,
    /// 原始请求体。转换链在此之上做改写。
    pub raw: serde_json::Value,
}

/// 贯穿中继管线的上下文。由 Distribute 中间件注入渠道信息后进入 relay_engine。
#[derive(Debug, Clone)]
pub struct RelayInfo {
    pub request_id: String,
    pub user_id: i64,
    pub token_id: i64,
    pub group: String,
    /// 用户请求的原始模型名。
    pub origin_model: String,
    /// 经 model_mapping 映射后的上游模型名。
    pub upstream_model: String,
    pub format: crate::RelayFormat,
    pub is_stream: bool,

    // --- Distribute 后置注入(CONTRACTS.md §10 渠道上下文注入约定)---
    pub channel_id: i64,
    pub channel_type: i32,
    pub base_url: Option<String>,
    /// 含 multi-key 选中下标。
    pub key: String,
    pub key_index: Option<usize>,
    pub model_mapping: Option<serde_json::Value>,
    pub param_override: Option<serde_json::Value>,
    pub header_override: Option<serde_json::Value>,
    pub status_code_mapping: Option<String>,
    pub auto_ban: bool,
    pub setting: Option<serde_json::Value>,

    /// 第几次尝试(0 起)。决定选路取第几优先级档。
    pub retry_count: u32,
}

/// usage 语义。**决定计价时是否从 prompt 中扣减缓存 token**。
///
/// - `OpenAi`:`prompt_tokens` **已包含** cached / cache_creation,须先扣减
/// - `Anthropic`:Claude 分开上报,`prompt_tokens` 不含,**不扣减**
///
/// 判错会造成系统性偏账,见 test/cases/02-unit-billing.md。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UsageSemantic {
    #[default]
    OpenAi,
    Anthropic,
}

/// token 用量。结算的唯一输入。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    /// 缓存命中 token(按 CacheRatio 计价)。
    #[serde(default)]
    pub cached_tokens: i64,
    /// 缓存写入 token(按 CacheCreationRatio 计价)。
    #[serde(default)]
    pub cache_creation_tokens: i64,
    /// Anthropic 5 分钟 TTL 缓存写入,单价独立。
    ///
    /// **线上字段名是 `claude_cache_creation_5_m_tokens`**(5 与 m 之间有下划线)——
    /// Go 侧 struct tag 由驼峰自动转换产生的形态,录制实测确认,不可"修正"为 `5m`。
    #[serde(default, rename = "claude_cache_creation_5_m_tokens")]
    pub claude_cache_creation_5m_tokens: i64,
    /// Anthropic 1 小时 TTL 缓存写入,单价独立。同上,线上是 `1_h`。
    #[serde(default, rename = "claude_cache_creation_1_h_tokens")]
    pub claude_cache_creation_1h_tokens: i64,
    #[serde(default)]
    pub image_tokens: i64,
    #[serde(default)]
    pub audio_tokens: i64,
    /// 上游显式声明的语义。为 None 时由 `effective_semantic` 推断。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_semantic: Option<UsageSemantic>,
}

impl Usage {
    /// 判定实际生效的 usage 语义。
    ///
    /// 优先级:
    /// 1. 上游显式声明的 `usage_semantic`
    /// 2. 最终请求格式为 Claude → Anthropic
    /// 3. **遗留 Claude 派生**:以 OpenAI 格式上报但带了 claude 分档字段
    ///    且无语义标记 → 按 Anthropic 处理(见 TC-UNI-BIL-004)
    /// 4. 否则 OpenAI
    pub fn effective_semantic(&self, final_format: crate::RelayFormat) -> UsageSemantic {
        if let Some(s) = self.usage_semantic {
            return s;
        }
        if final_format == crate::RelayFormat::Claude {
            return UsageSemantic::Anthropic;
        }
        if self.is_legacy_claude_derived() {
            return UsageSemantic::Anthropic;
        }
        UsageSemantic::OpenAi
    }

    /// 是否属于「遗留 Claude 派生的 OpenAI usage」。
    pub fn is_legacy_claude_derived(&self) -> bool {
        self.usage_semantic.is_none()
            && (self.claude_cache_creation_5m_tokens > 0
                || self.claude_cache_creation_1h_tokens > 0)
    }
}

/// 上游响应句柄。非流式持有完整体,流式持有字节流。
pub struct UpstreamResponse {
    pub status: u16,
    pub headers: http::HeaderMap,
    /// 流式时为 None,由 stream_pipe 驱动读取。
    pub body: Option<bytes::Bytes>,
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] Usage 反序列化能吃下 OpenAI / Claude / Gemini 三种 usage 结构
    // - [ ] 缺省字段(cached_tokens 等)默认 0 而非报错
    // - [ ] RelayInfo 在 retry_count 递增时携带正确的档位语义
}
