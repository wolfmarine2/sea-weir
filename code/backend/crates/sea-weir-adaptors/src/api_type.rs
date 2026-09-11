//! 渠道类型枚举。
//!
//! 与 new-api `constant/api_type.go` 的 35 个常量一一对应(不含仅作计数位的 `APITypeDummy`)。
//! 注意:`relay/channel/` 下有 36 个目录,部分目录共用 OpenAI 兼容 ApiType。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum ApiType {
    OpenAi = 0,
    Anthropic,
    PaLM,
    Baidu,
    Zhipu,
    Ali,
    Xunfei,
    AiProxyLibrary,
    Tencent,
    Gemini,
    ZhipuV4,
    Ollama,
    Perplexity,
    Aws,
    Cohere,
    Dify,
    Jina,
    Cloudflare,
    SiliconFlow,
    VertexAi,
    Mistral,
    DeepSeek,
    MokaAi,
    VolcEngine,
    BaiduV2,
    OpenRouter,
    Xinference,
    Xai,
    Coze,
    Jimeng,
    Moonshot,
    Submodel,
    MiniMax,
    Replicate,
    Codex,
}

impl ApiType {
    /// 全部 35 个类型,注册表构建用。
    pub const ALL: &'static [ApiType] = &[
        Self::OpenAi,
        Self::Anthropic,
        Self::PaLM,
        Self::Baidu,
        Self::Zhipu,
        Self::Ali,
        Self::Xunfei,
        Self::AiProxyLibrary,
        Self::Tencent,
        Self::Gemini,
        Self::ZhipuV4,
        Self::Ollama,
        Self::Perplexity,
        Self::Aws,
        Self::Cohere,
        Self::Dify,
        Self::Jina,
        Self::Cloudflare,
        Self::SiliconFlow,
        Self::VertexAi,
        Self::Mistral,
        Self::DeepSeek,
        Self::MokaAi,
        Self::VolcEngine,
        Self::BaiduV2,
        Self::OpenRouter,
        Self::Xinference,
        Self::Xai,
        Self::Coze,
        Self::Jimeng,
        Self::Moonshot,
        Self::Submodel,
        Self::MiniMax,
        Self::Replicate,
        Self::Codex,
    ];

    /// 渠道表 `type` 列(ChannelType)→ ApiType。
    ///
    /// 与 new-api `common/api_type.go` `ChannelType2APIType` 一一对应;
    /// 未登记的渠道类型返回 `None`(new-api 侧回落 OpenAI 并标记未识别)。
    pub fn from_channel_type(channel_type: i32) -> Option<Self> {
        let api_type = match channel_type {
            1 => Self::OpenAi,
            4 => Self::Ollama,
            11 => Self::PaLM,
            14 => Self::Anthropic,
            15 => Self::Baidu,
            16 => Self::Zhipu,
            17 => Self::Ali,
            18 => Self::Xunfei,
            20 => Self::OpenRouter,
            21 => Self::AiProxyLibrary,
            23 => Self::Tencent,
            24 => Self::Gemini,
            25 => Self::Moonshot,
            26 => Self::ZhipuV4,
            27 => Self::Perplexity,
            33 => Self::Aws,
            34 => Self::Cohere,
            35 => Self::MiniMax,
            37 => Self::Dify,
            38 => Self::Jina,
            39 => Self::Cloudflare,
            40 => Self::SiliconFlow,
            41 => Self::VertexAi,
            42 => Self::Mistral,
            43 => Self::DeepSeek,
            44 => Self::MokaAi,
            45 => Self::VolcEngine,
            46 => Self::BaiduV2,
            47 => Self::Xinference,
            48 => Self::Xai,
            49 => Self::Coze,
            51 => Self::Jimeng,
            53 => Self::Submodel,
            56 => Self::Replicate,
            57 => Self::Codex,
            _ => return None,
        };
        Some(api_type)
    }
}

/// 异步任务平台。对应 new-api `relay/channel/task/` 下 10 个平台目录。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPlatform {
    Ali,
    Doubao,
    Gemini,
    Hailuo,
    Jimeng,
    Kling,
    Sora,
    Suno,
    Vertex,
    Vidu,
}

impl TaskPlatform {
    pub const ALL: &'static [TaskPlatform] = &[
        Self::Ali,
        Self::Doubao,
        Self::Gemini,
        Self::Hailuo,
        Self::Jimeng,
        Self::Kling,
        Self::Sora,
        Self::Suno,
        Self::Vertex,
        Self::Vidu,
    ];
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] ApiType::ALL.len() == 35(数量锁死,防止漏实现)
    // - [ ] TaskPlatform::ALL.len() == 10
    // - [ ] from_channel_type 覆盖 new-api 全部渠道类型常量,未知值返回 None
    // - [ ] 判别值与 Go 侧 iota 顺序一致(渠道表存的是数字,不能错位)
}
