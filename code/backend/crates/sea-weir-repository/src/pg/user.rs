//! `UserRepository` 的 openGauss 实现。额度扣减必须带 `AND quota >= $1` 守卫。

use std::sync::Arc;

pub struct PgUserRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgUserRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }
}

// TODO(TDD): impl crate::traits::UserRepository for PgUserRepository
//   —— 先写测试,再补 sqlx::query! 实现。
