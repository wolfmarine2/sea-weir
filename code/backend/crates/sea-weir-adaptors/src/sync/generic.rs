//! 通用渠道适配器。
//!
//! 35 个同步渠道在协议层分为四族(`Protocol`),绝大多数差异只在
//! base_url、鉴权头与模型列表。这里实现协议族共性,作为注册表的默认实现;
//! 有专有协议差异的渠道(如 Anthropic / Gemini 的原生事件流)可在
//! 同名模块中替换为专有 `Adaptor` 实现,注册表逐项覆盖即可。
//!
//! 这样做的收益:新增渠道的成本降为「登记一条元数据」,而共性逻辑
//! (URL 拼接 / header_override / param_override / 错误归一化)只有一份。

use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use sea_weir_types::{
    dto::{relay::UpstreamResponse, RelayInfo, RelayRequest, Usage},
    AppError, NewApiError,
};

use super::common;
use crate::{Adaptor, ApiType};

/// 渠道协议族。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// OpenAI 兼容:`Authorization: Bearer <key>`,默认 `/v1/chat/completions`。
    OpenAi,
    /// Anthropic 原生:`x-api-key`,默认 `/v1/messages`。
    Claude,
    /// Google Gemini 原生:`x-goog-api-key`,默认 `/v1beta/models`。
    Gemini,
    /// 厂商自定义路径与鉴权,由 `base_url` 与 `header_override` 决定。
    Custom,
}

/// 通用同步渠道适配器。
pub struct CompatAdaptor {
    pub api_type: ApiType,
    pub name: &'static str,
    pub default_base: &'static str,
    pub protocol: Protocol,
    /// 该渠道支持的模型列表(骨架阶段为空,由管理面同步填充)。
    pub models: Vec<String>,
}

impl CompatAdaptor {
    fn default_path(&self) -> &'static str {
        match self.protocol {
            Protocol::Claude => "/v1/messages",
            Protocol::Gemini => "/v1beta/models",
            Protocol::OpenAi | Protocol::Custom => "/v1/chat/completions",
        }
    }
}

#[async_trait]
impl Adaptor for CompatAdaptor {
    fn init(&self, _info: &RelayInfo) -> Result<(), AppError> {
        Ok(())
    }

    fn get_request_url(&self, info: &RelayInfo) -> Result<String, AppError> {
        Ok(common::join_url(
            info.base_url.as_deref(),
            self.default_base,
            self.default_path(),
        ))
    }

    fn setup_request_header(
        &self,
        headers: &mut HeaderMap,
        info: &RelayInfo,
    ) -> Result<(), AppError> {
        let value = match self.protocol {
            Protocol::Claude => Some(("x-api-key", info.key.clone())),
            Protocol::Gemini => Some(("x-goog-api-key", info.key.clone())),
            Protocol::OpenAi | Protocol::Custom => {
                Some(("authorization", format!("Bearer {}", info.key)))
            }
        };
        if let Some((name, value)) = value {
            if let (Ok(name), Ok(value)) = (
                http::HeaderName::from_bytes(name.as_bytes()),
                http::HeaderValue::from_str(&value),
            ) {
                headers.insert(name, value);
            }
        }
        // 渠道级 header_override 最后应用,可覆盖上面的鉴权头。
        common::apply_header_override(headers, info);
        Ok(())
    }

    fn convert_request(&self, req: RelayRequest, info: &RelayInfo) -> Result<Bytes, AppError> {
        // 协议族内的字段改写由 pipeline 的 convert 链完成;此处应用渠道参数覆盖。
        let mut body = req.raw;
        common::apply_param_override(&mut body, info);
        serde_json::to_vec(&body)
            .map(Bytes::from)
            .map_err(|e| AppError::BadRequest(format!("请求体序列化失败: {e}")))
    }

    async fn do_request(
        &self,
        info: &RelayInfo,
        body: Bytes,
    ) -> Result<UpstreamResponse, AppError> {
        let url = self.get_request_url(info)?;
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
        self.setup_request_header(&mut headers, info)?;

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .headers(headers)
            .body(body.to_vec())
            .send()
            .await
            .map_err(|e| AppError::Upstream(format!("上游请求失败: {e}")))?;

        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::Upstream(format!("上游响应读取失败: {e}")))?;
        Ok(UpstreamResponse {
            status,
            headers,
            body: Some(bytes),
        })
    }

    async fn do_response(
        &self,
        resp: UpstreamResponse,
        _info: &mut RelayInfo,
    ) -> Result<Usage, NewApiError> {
        let Some(body) = resp.body else {
            // 流式响应由 stream_pipe 驱动并累计 usage。
            return Ok(Usage::default());
        };
        let value: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|e| NewApiError::from(AppError::Upstream(format!("上游响应解析失败: {e}"))))?;
        Ok(parse_usage(&value, self.protocol))
    }

    fn get_model_list(&self) -> &[String] {
        &self.models
    }

    fn channel_name(&self) -> &'static str {
        self.name
    }
}

/// 从上游响应中提取 usage。三种协议族的字段结构不同。
pub fn parse_usage(value: &serde_json::Value, protocol: Protocol) -> Usage {
    match protocol {
        Protocol::Claude => {
            let prompt = value
                .pointer("/usage/input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let completion = value
                .pointer("/usage/output_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            Usage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: prompt + completion,
                cached_tokens: value
                    .pointer("/usage/cache_read_input_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                cache_creation_tokens: value
                    .pointer("/usage/cache_creation_input_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                usage_semantic: Some(sea_weir_types::dto::relay::UsageSemantic::Anthropic),
                ..Default::default()
            }
        }
        Protocol::Gemini => {
            let prompt = value
                .pointer("/usageMetadata/promptTokenCount")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let completion = value
                .pointer("/usageMetadata/candidatesTokenCount")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            Usage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: value
                    .pointer("/usageMetadata/totalTokenCount")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(prompt + completion),
                ..Default::default()
            }
        }
        Protocol::OpenAi | Protocol::Custom => {
            let mut usage: Usage =
                serde_json::from_value(value.get("usage").cloned().unwrap_or_default())
                    .unwrap_or_default();
            if usage.total_tokens == 0 {
                usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;
            }
            usage
        }
    }
}
