//! `RedemptionRepository` 的 openGauss 实现。事务 + FOR UPDATE 行锁。

use std::sync::Arc;

pub struct PgRedemptionRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgRedemptionRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }
}

// TODO(TDD): impl crate::traits::RedemptionRepository for PgRedemptionRepository
//   —— 先写测试,再补 sqlx::query! 实现。
