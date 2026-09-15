//! 预填分组:模型广场按 model / tag / endpoint 预置一批条目,便于快速筛选。

use serde::{Deserialize, Serialize};

/// 预填分组类型。
pub const PREFILL_TYPES: &[&str] = &["model", "tag", "endpoint"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefillGroup {
    pub id: i64,
    pub name: String,
    /// model / tag / endpoint。
    pub r#type: String,
    /// 条目数组(JSONB);语义随 type 而定。
    pub items: Option<serde_json::Value>,
    pub description: String,
    pub created_time: i64,
    pub updated_time: i64,
}
