//! 渠道类型枚举。
//!
//! 与 new-api `constant/api_type.go` 的 35 个常量一一对应(不含仅作计数位的 `APITypeDummy`),
//! 另含 2 个 sea-weir 扩展类型(`OpenCode` / `CommandCode`,编号 100/101 避开 new-api 空间)。
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
    /// sea-weir 扩展:OpenCode(OpenAI 兼容)。
    OpenCode,
    /// sea-weir 扩展:CommandCode(OpenAI 兼容)。
    CommandCode,
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
        Self::OpenCode,
        Self::CommandCode,
    ];

    /// ChannelType ↔ ApiType 登记表(new-api `common/api_type.go` `ChannelType2APIType` 的镜像)。
    /// 两个方向都由它派生,避免下发列表与解析逻辑漂移。
    pub const CHANNEL_TYPE_MAP: &'static [(i32, ApiType)] = &[
        (1, ApiType::OpenAi),
        (4, ApiType::Ollama),
        (11, ApiType::PaLM),
        (14, ApiType::Anthropic),
        (15, ApiType::Baidu),
        (16, ApiType::Zhipu),
        (17, ApiType::Ali),
        (18, ApiType::Xunfei),
        (20, ApiType::OpenRouter),
        (21, ApiType::AiProxyLibrary),
        (23, ApiType::Tencent),
        (24, ApiType::Gemini),
        (25, ApiType::Moonshot),
        (26, ApiType::ZhipuV4),
        (27, ApiType::Perplexity),
        (33, ApiType::Aws),
        (34, ApiType::Cohere),
        (35, ApiType::MiniMax),
        (37, ApiType::Dify),
        (38, ApiType::Jina),
        (39, ApiType::Cloudflare),
        (40, ApiType::SiliconFlow),
        (41, ApiType::VertexAi),
        (42, ApiType::Mistral),
        (43, ApiType::DeepSeek),
        (44, ApiType::MokaAi),
        (45, ApiType::VolcEngine),
        (46, ApiType::BaiduV2),
        (47, ApiType::Xinference),
        (48, ApiType::Xai),
        (49, ApiType::Coze),
        (51, ApiType::Jimeng),
        (53, ApiType::Submodel),
        (56, ApiType::Replicate),
        (57, ApiType::Codex),
        // sea-weir 扩展:编号避开 new-api 已用空间(其常量最大 57)。
        (100, ApiType::OpenCode),
        (101, ApiType::CommandCode),
    ];

    /// 渠道表 `type` 列(ChannelType)→ ApiType。
    ///
    /// 未登记的渠道类型返回 `None`(调用方回落 OpenAI 并标记未识别)。
    pub fn from_channel_type(channel_type: i32) -> Option<Self> {
        Self::CHANNEL_TYPE_MAP
            .iter()
            .find(|(c, _)| *c == channel_type)
            .map(|(_, api_type)| *api_type)
    }

    /// ApiType → 渠道表 `type` 列(反向映射)。
    pub fn channel_type(self) -> Option<i32> {
        Self::CHANNEL_TYPE_MAP
            .iter()
            .find(|(_, api_type)| *api_type == self)
            .map(|(c, _)| *c)
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
    use super::*;

    #[test]
    fn all_types_are_registered_and_round_trip() {
        assert_eq!(ApiType::ALL.len(), 37, "35 个 new-api 类型 + 2 个扩展;数量锁死防止漏实现");
        assert_eq!(TaskPlatform::ALL.len(), 10);
        for api_type in ApiType::ALL {
            let ct = api_type
                .channel_type()
                .unwrap_or_else(|| panic!("{api_type:?} 缺少 ChannelType 登记"));
            assert_eq!(
                ApiType::from_channel_type(ct),
                Some(*api_type),
                "{api_type:?} 双向映射不一致"
            );
        }
    }

    #[test]
    fn channel_type_map_has_no_duplicates() {
        let mut types: Vec<i32> = ApiType::CHANNEL_TYPE_MAP.iter().map(|(c, _)| *c).collect();
        types.sort_unstable();
        types.dedup();
        assert_eq!(types.len(), ApiType::CHANNEL_TYPE_MAP.len(), "ChannelType 有重复");
        assert_eq!(
            ApiType::CHANNEL_TYPE_MAP.len(),
            ApiType::ALL.len(),
            "每个 ApiType 应恰好登记一次"
        );
    }

    #[test]
    fn unknown_channel_type_returns_none() {
        assert_eq!(ApiType::from_channel_type(9999), None);
        assert_eq!(ApiType::from_channel_type(2), None, "2 未登记(历史上是 API2D)");
        assert_eq!(ApiType::from_channel_type(1), Some(ApiType::OpenAi));
        assert_eq!(ApiType::from_channel_type(43), Some(ApiType::DeepSeek));
    }
}
