//! `TokenRepository` 的 openGauss 实现。读穿 Valkey;明文 key 不入缓存。

use std::sync::Arc;

pub struct PgTokenRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgTokenRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }
}

// TODO(TDD): impl crate::traits::TokenRepository for PgTokenRepository
//   —— 先写测试,再补 sqlx::query! 实现。
