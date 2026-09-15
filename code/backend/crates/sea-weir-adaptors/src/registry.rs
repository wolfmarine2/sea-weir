//! 适配器注册表。按 `ApiType` / `TaskPlatform` 索引到具体实现。

use std::collections::HashMap;
use std::sync::Arc;

use crate::sync::generic::{CompatAdaptor, Protocol};
use crate::task::generic::CompatTaskAdaptor;
use crate::{Adaptor, ApiType, TaskAdaptor, TaskPlatform};

/// 进程启动时构建一次,之后只读共享。
pub struct AdaptorRegistry {
    sync: HashMap<ApiType, Arc<dyn Adaptor>>,
    task: HashMap<TaskPlatform, Arc<dyn TaskAdaptor>>,
}

impl AdaptorRegistry {
    /// 构建全量注册表。
    ///
    /// 不变量:`ApiType::ALL` 中的每一项都必须有实现 —— 缺失应在启动时 panic,
    /// 而不是等到线上某个渠道被选中才失败。
    pub fn build() -> Self {
        let mut sync: HashMap<ApiType, Arc<dyn Adaptor>> = HashMap::new();
        for api_type in ApiType::ALL {
            let (name, default_base, protocol) = sync_spec(*api_type);
            let adaptor = CompatAdaptor {
                api_type: *api_type,
                name,
                default_base,
                protocol,
                models: Vec::new(),
            };
            sync.insert(*api_type, Arc::new(adaptor));
        }
        assert_eq!(
            sync.len(),
            ApiType::ALL.len(),
            "同步适配器注册数与 ApiType::ALL 不一致"
        );

        let mut task: HashMap<TaskPlatform, Arc<dyn TaskAdaptor>> = HashMap::new();
        for platform in TaskPlatform::ALL {
            let (name, submit_path) = task_spec(*platform);
            task.insert(
                *platform,
                Arc::new(CompatTaskAdaptor {
                    platform: *platform,
                    name,
                    submit_path,
                }),
            );
        }
        assert_eq!(
            task.len(),
            TaskPlatform::ALL.len(),
            "任务适配器注册数与 TaskPlatform::ALL 不一致"
        );

        Self { sync, task }
    }

    pub fn get(&self, api_type: ApiType) -> Option<Arc<dyn Adaptor>> {
        self.sync.get(&api_type).cloned()
    }

    pub fn get_task(&self, platform: TaskPlatform) -> Option<Arc<dyn TaskAdaptor>> {
        self.task.get(&platform).cloned()
    }
}

/// 渠道类型目录(管理面「类型」下拉用):`(ChannelType, 展示名, 默认 base_url)`。
///
/// 由 [`ApiType::CHANNEL_TYPE_MAP`] 与 [`sync_spec`] 派生,保证与解析/适配器元数据同源;
/// 按 ChannelType 升序返回,便于表单稳定排序。
pub fn channel_type_catalog() -> Vec<(i32, &'static str, &'static str)> {
    let mut items: Vec<(i32, &'static str, &'static str)> = ApiType::ALL
        .iter()
        .filter_map(|api_type| {
            let channel_type = api_type.channel_type()?;
            let (name, default_base, _) = sync_spec(*api_type);
            Some((channel_type, name, default_base))
        })
        .collect();
    items.sort_by_key(|(channel_type, _, _)| *channel_type);
    items
}

/// 同步渠道元数据:`(展示名, 默认 base_url, 协议族)`。
///
/// 展示名进日志与前端,必须全表唯一(`TC-UNI-ADP-010`)。
fn sync_spec(api_type: ApiType) -> (&'static str, &'static str, Protocol) {
    use ApiType::*;
    match api_type {
        OpenAi => ("OpenAI", "https://api.openai.com", Protocol::OpenAi),
        Anthropic => ("Anthropic", "https://api.anthropic.com", Protocol::Claude),
        PaLM => (
            "PaLM",
            "https://generativelanguage.googleapis.com",
            Protocol::Gemini,
        ),
        Baidu => ("Baidu", "https://aip.baidubce.com", Protocol::Custom),
        Zhipu => ("Zhipu", "https://open.bigmodel.cn", Protocol::OpenAi),
        Ali => ("Ali", "https://dashscope.aliyuncs.com", Protocol::OpenAi),
        Xunfei => (
            "Xunfei",
            "https://spark-api-open.xf-yun.com",
            Protocol::OpenAi,
        ),
        AiProxyLibrary => ("AiProxyLibrary", "https://api.aiproxy.io", Protocol::OpenAi),
        Tencent => (
            "Tencent",
            "https://hunyuan.tencentcloudapi.com",
            Protocol::Custom,
        ),
        Gemini => (
            "Gemini",
            "https://generativelanguage.googleapis.com",
            Protocol::Gemini,
        ),
        ZhipuV4 => ("ZhipuV4", "https://open.bigmodel.cn", Protocol::OpenAi),
        Ollama => ("Ollama", "http://localhost:11434", Protocol::Custom),
        Perplexity => ("Perplexity", "https://api.perplexity.ai", Protocol::OpenAi),
        Aws => ("Aws", "", Protocol::Custom),
        Cohere => ("Cohere", "https://api.cohere.ai", Protocol::Custom),
        Dify => ("Dify", "", Protocol::Custom),
        Jina => ("Jina", "https://api.jina.ai", Protocol::OpenAi),
        Cloudflare => ("Cloudflare", "", Protocol::Custom),
        SiliconFlow => (
            "SiliconFlow",
            "https://api.siliconflow.cn",
            Protocol::OpenAi,
        ),
        VertexAi => ("VertexAi", "", Protocol::Gemini),
        Mistral => ("Mistral", "https://api.mistral.ai", Protocol::OpenAi),
        DeepSeek => ("DeepSeek", "https://api.deepseek.com", Protocol::OpenAi),
        MokaAi => ("MokaAi", "", Protocol::OpenAi),
        VolcEngine => (
            "VolcEngine",
            "https://ark.cn-beijing.volces.com",
            Protocol::OpenAi,
        ),
        BaiduV2 => ("BaiduV2", "https://qianfan.baidubce.com", Protocol::OpenAi),
        OpenRouter => ("OpenRouter", "https://openrouter.ai/api", Protocol::OpenAi),
        Xinference => ("Xinference", "", Protocol::OpenAi),
        Xai => ("Xai", "https://api.x.ai", Protocol::OpenAi),
        Coze => ("Coze", "https://api.coze.cn", Protocol::Custom),
        Jimeng => ("Jimeng", "", Protocol::Custom),
        Moonshot => ("Moonshot", "https://api.moonshot.cn", Protocol::Claude),
        Submodel => ("Submodel", "", Protocol::OpenAi),
        MiniMax => ("MiniMax", "https://api.minimax.chat", Protocol::OpenAi),
        Replicate => ("Replicate", "https://api.replicate.com", Protocol::Custom),
        Codex => ("Codex", "", Protocol::OpenAi),
    }
}

/// 任务平台元数据:`(展示名, 提交路径)`。
fn task_spec(platform: TaskPlatform) -> (&'static str, &'static str) {
    use TaskPlatform::*;
    match platform {
        Ali => ("ali-task", "/v1/tasks"),
        Doubao => ("doubao-task", "/v1/tasks"),
        Gemini => ("gemini-task", "/v1/tasks"),
        Hailuo => ("hailuo-task", "/v1/tasks"),
        Jimeng => ("jimeng-task", "/v1/tasks"),
        Kling => ("kling-task", "/v1/videos"),
        Sora => ("sora-task", "/v1/videos"),
        Suno => ("suno-task", "/api/v1/generate"),
        Vertex => ("vertex-task", "/v1/tasks"),
        Vidu => ("vidu-task", "/v1/tasks"),
    }
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] build() 后 ApiType::ALL 中每一项都能 get 到(全覆盖断言)
    // - [ ] TaskPlatform::ALL 同上
    // - [ ] channel_name() 在全表内唯一
}
