//! `OptionRepository` 的 openGauss 实现。写后广播失效。

use async_trait::async_trait;
use sea_weir_types::{AppError, AppResult};
use std::sync::Arc;

pub struct PgOptionRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgOptionRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }

    fn pool(&self) -> &sqlx::PgPool {
        &self.ctx.pools.main
    }
}

fn db_err(e: sqlx::Error) -> AppError {
    AppError::Database(e.to_string())
}

#[async_trait]
impl crate::traits::OptionRepository for PgOptionRepository {
    async fn load_all(&self) -> AppResult<Vec<(String, String)>> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM options")
            .fetch_all(self.pool())
            .await
            .map_err(db_err)?;
        Ok(rows)
    }

    async fn upsert(&self, key: &str, value: &str) -> AppResult<()> {
        // NOTE(TDD): 多节点失效广播(pub/sub)待 Valkey 客户端接入后补;
        // 当前仅落库,本进程内的配置缓存也尚未引入。
        sqlx::query(
            "INSERT INTO options (key, value) VALUES ($1, $2) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(key)
        .bind(value)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn get(&self, key: &str) -> AppResult<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM options WHERE key = $1")
            .bind(key)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        Ok(row.map(|(value,)| value))
    }
}
