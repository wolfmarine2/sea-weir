//! `LogRepository` 的 openGauss 实现。写入走日志库 pool。

use std::sync::Arc;

pub struct PgLogRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgLogRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }
}

// TODO(TDD): impl crate::traits::LogRepository for PgLogRepository
//   —— 先写测试,再补 sqlx::query! 实现。
