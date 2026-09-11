//! `AbilityRepository` 的 openGauss 实现。选路热查询,注意 priority 索引。

use std::sync::Arc;

pub struct PgAbilityRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgAbilityRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }
}

// TODO(TDD): impl crate::traits::AbilityRepository for PgAbilityRepository
//   —— 先写测试,再补 sqlx::query! 实现。
