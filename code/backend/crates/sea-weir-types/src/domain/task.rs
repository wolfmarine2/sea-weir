//! 异步任务领域模型。对应 `tasks` 表(视频/音乐等)与 `midjourneys` 表。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    NotStart,
    Submitted,
    Queued,
    InProgress,
    Failure,
    Success,
    Unknown,
}

impl TaskStatus {
    /// 终态不可再迁移(CAS 的判定依据)。
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Failure | Self::Success)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub user_id: i64,
    pub channel_id: i64,
    /// 对外暴露的任务 id。
    pub task_id: String,
    pub platform: String,
    pub action: String,
    pub status: TaskStatus,
    pub fail_reason: Option<String>,
    pub submit_time: i64,
    pub start_time: i64,
    pub finish_time: i64,
    pub progress: Option<String>,
    pub properties: Option<serde_json::Value>,
    /// 计费快照(预扣额度、倍率),用于完成时补差/退款。
    pub private_data: Option<serde_json::Value>,
    pub quota: i64,
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] is_terminal() 仅对 Success/Failure 为 true
    // - [ ] TaskStatus 序列化文本与 new-api 一致(客户端会直接读这个字段)
}
