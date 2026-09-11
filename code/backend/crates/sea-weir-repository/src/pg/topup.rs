//! `TopUpRepository` 的 openGauss 实现。trade_no 唯一 + 状态 CAS。

use std::sync::Arc;

pub struct PgTopUpRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgTopUpRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }
}

// TODO(TDD): impl crate::traits::TopUpRepository for PgTopUpRepository
//   —— 先写测试,再补 sqlx::query! 实现。
