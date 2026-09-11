//! `OptionRepository` 的 openGauss 实现。写后广播失效。

use std::sync::Arc;

pub struct PgOptionRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgOptionRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }
}

// TODO(TDD): impl crate::traits::OptionRepository for PgOptionRepository
//   —— 先写测试,再补 sqlx::query! 实现。
