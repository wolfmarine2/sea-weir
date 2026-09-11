use async_trait::async_trait;
use sea_weir_types::{
    domain::{task::TaskStatus, Task},
    AppResult,
};

#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(&self, task: &Task) -> AppResult<i64>;
    async fn find_by_task_id(&self, task_id: &str) -> AppResult<Option<Task>>;
    /// 轮询取待处理任务(非终态),按平台分组。
    async fn list_pending(&self, limit: i64) -> AppResult<Vec<Task>>;

    /// 终态迁移 CAS。
    ///
    /// `UPDATE tasks SET status=$1 WHERE id=$2 AND status=$3`
    /// 返回 false 表示已被其他节点抢先迁移 —— **调用方据此跳过重复结算/退款**。
    async fn cas_status(&self, id: i64, from: TaskStatus, to: TaskStatus) -> AppResult<bool>;

    async fn update_progress(&self, id: i64, progress: &str) -> AppResult<()>;
}

#[cfg(test)]
mod tests {
    // TDD 入口(多节点轮询的关键):
    // - [ ] 两个节点同时 cas_status(InProgress→Success),恰好一个返回 true
    // - [ ] from 状态不匹配时返回 false 且不改库
    // - [ ] 终态任务不会被 list_pending 捞出
}
