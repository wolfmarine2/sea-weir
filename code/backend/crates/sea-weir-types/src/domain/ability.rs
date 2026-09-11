//! 渠道-模型-分组能力三元组。对应 `abilities` 表,是选路的核心索引。
//!
//! 复合主键:(group, model, channel_id)。渠道写入时在同一事务内「先删后插」重建。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ability {
    pub group: String,
    pub model: String,
    pub channel_id: i64,
    pub enabled: bool,
    pub priority: i64,
    pub weight: i64,
    pub tag: Option<String>,
}
