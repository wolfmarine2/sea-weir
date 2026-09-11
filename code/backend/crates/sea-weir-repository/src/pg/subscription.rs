//! `SubscriptionRepository` 的 openGauss 实现。request_id 幂等 + end_time 升序 FOR UPDATE。

use std::sync::Arc;

pub struct PgSubscriptionRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgSubscriptionRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }
}

// TODO(TDD): impl crate::traits::SubscriptionRepository for PgSubscriptionRepository
//   —— 先写测试,再补 sqlx::query! 实现。
