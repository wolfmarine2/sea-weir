//! 同步渠道适配器实现(35 个,对应 `ApiType`)。
//!
//! 骨架阶段每个模块仅有结构体与 TODO。TDD 落地顺序建议按渠道使用频度:
//! openai → anthropic → gemini → deepseek → volcengine → ali → 其余。
//!
//! 共性逻辑(URL 拼接、header_override、param_override、错误归一化)应先沉到
//! `common` 模块,避免 35 份复制粘贴。

pub mod aiproxy_library;
pub mod ali;
pub mod anthropic;
pub mod aws;
pub mod baidu;
pub mod baidu_v2;
pub mod cloudflare;
pub mod codex;
pub mod cohere;
pub mod common;
pub mod coze;
pub mod deepseek;
pub mod dify;
pub mod gemini;
pub mod generic;
pub mod jimeng;
pub mod jina;
pub mod minimax;
pub mod mistral;
pub mod moka_ai;
pub mod moonshot;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod palm;
pub mod perplexity;
pub mod replicate;
pub mod siliconflow;
pub mod submodel;
pub mod tencent;
pub mod vertex_ai;
pub mod volcengine;
pub mod xai;
pub mod xinference;
pub mod xunfei;
pub mod zhipu;
pub mod zhipu_v4;
