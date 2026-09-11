//! `TaskRepository` 的 openGauss 实现。终态迁移用 CAS。

use std::sync::Arc;

pub struct PgTaskRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgTaskRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }
}

// TODO(TDD): impl crate::traits::TaskRepository for PgTaskRepository
//   —— 先写测试,再补 sqlx::query! 实现。
