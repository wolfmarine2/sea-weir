//! 系统选项 KV。对应 `options` 表。
//!
//! 带 `.` 的键(如 `xxx_setting.yyy`)走注册式分层配置组,与 new-api 语义一致。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Option {
    pub key: String,
    pub value: String,
}
