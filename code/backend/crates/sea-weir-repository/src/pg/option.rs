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

/// 唯一键冲突(PG/openGauss SQLSTATE `23505`)。
fn is_unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(|d| d.code())
        .map(|c| c == "23505")
        .unwrap_or(false)
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
        //
        // openGauss 基于 PG 9.2,不支持 `INSERT ... ON CONFLICT`(9.5+ 语法),
        // 故用 UPDATE → 未命中再 INSERT 的两步写法;并发下 INSERT 撞唯一键时
        // 说明别的请求刚插入,回退成 UPDATE 即可。
        let updated = sqlx::query("UPDATE options SET value = $1 WHERE key = $2")
            .bind(value)
            .bind(key)
            .execute(self.pool())
            .await
            .map_err(db_err)?
            .rows_affected();
        if updated > 0 {
            return Ok(());
        }

        match sqlx::query("INSERT INTO options (key, value) VALUES ($1, $2)")
            .bind(key)
            .bind(value)
            .execute(self.pool())
            .await
        {
            Ok(_) => Ok(()),
            Err(e) if is_unique_violation(&e) => {
                sqlx::query("UPDATE options SET value = $1 WHERE key = $2")
                    .bind(value)
                    .bind(key)
                    .execute(self.pool())
                    .await
                    .map_err(db_err)?;
                Ok(())
            }
            Err(e) => Err(db_err(e)),
        }
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
