//! `ChannelRepository` 的 openGauss 实现。abilities 同步走事务先删后插。

use std::sync::Arc;

pub struct PgChannelRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgChannelRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }
}

// TODO(TDD): impl crate::traits::ChannelRepository for PgChannelRepository
//   —— 先写测试,再补 sqlx::query! 实现。
