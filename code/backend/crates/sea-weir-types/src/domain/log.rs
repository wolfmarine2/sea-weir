//! 日志领域模型。对应 `logs` 表(物理上可独立日志库)。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    pub id: i64,
    pub user_id: i64,
    pub created_at: i64,
    /// 见 constants::LogType(0..=6)。
    pub r#type: i32,
    pub content: String,
    pub username: String,
    pub token_name: String,
    pub model_name: String,
    pub quota: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub use_time: i32,
    pub is_stream: bool,
    pub channel_id: Option<i64>,
    pub token_id: Option<i64>,
    pub group: Option<String>,
    pub ip: Option<String>,
    /// 贯穿中继链路与计费日志的请求 id。
    pub request_id: Option<String>,
    /// JSONB:倍率明细等可审计字段。
    pub other: Option<serde_json::Value>,
}
