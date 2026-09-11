//! 渠道领域模型。对应 `channels` 表。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: i64,
    /// 渠道类型 → ApiType 映射。
    pub r#type: i32,
    /// 多 key 换行分隔。**加密存储**,仅 RootAuth + 敏感操作凭证可取明文。
    #[serde(skip_serializing)]
    pub key: String,
    /// 1 启用 / 2 手动禁用 / 3 自动禁用
    pub status: i32,
    pub name: String,
    pub weight: i64,
    pub priority: i64,
    /// 逗号分隔多分组。
    pub group: String,
    /// 逗号分隔模型列表。
    pub models: String,
    pub model_mapping: Option<serde_json::Value>,
    pub param_override: Option<serde_json::Value>,
    pub header_override: Option<serde_json::Value>,
    pub base_url: Option<String>,
    pub openai_organization: Option<String>,
    pub test_model: Option<String>,
    pub balance: f64,
    pub balance_updated_time: i64,
    pub used_quota: i64,
    pub auto_ban: i32,
    pub tag: Option<String>,
    pub setting: Option<serde_json::Value>,
    pub other_settings: Option<serde_json::Value>,
    /// 含 multi-key 逐 key 状态。
    pub channel_info: Option<serde_json::Value>,
    pub status_code_mapping: Option<String>,
    pub other_info: Option<String>,
    pub created_time: i64,
    pub test_time: i64,
    pub response_time: i64,
}

/// 多 key 选择模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiKeyMode {
    Random,
    Polling,
}

impl Channel {
    /// 拆分多 key(换行分隔)。
    pub fn keys(&self) -> Vec<&str> {
        self.key
            .split('\n')
            .map(|k| k.trim())
            .filter(|k| !k.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] keys() 对 "k1\nk2\n\n" 返回 ["k1","k2"]
    // - [ ] 序列化不含 key 明文
    // - [ ] status 三态语义:自动禁用可被渠道测试恢复,手动禁用不可
}
