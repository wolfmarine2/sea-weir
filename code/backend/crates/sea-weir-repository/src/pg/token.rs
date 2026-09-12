//! `TokenRepository` 的 openGauss 实现。读穿 Valkey;明文 key 不入缓存。

use async_trait::async_trait;
use sea_weir_types::domain::{NewToken, Token};
use sea_weir_types::{AppError, AppResult};
use std::sync::Arc;

pub struct PgTokenRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgTokenRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }

    fn pool(&self) -> &sqlx::PgPool {
        &self.ctx.pools.main
    }
}

#[derive(sqlx::FromRow)]
struct TokenRow {
    id: i64,
    user_id: i64,
    key: String,
    status: i64,
    name: String,
    created_time: i64,
    accessed_time: i64,
    expired_time: i64,
    remain_quota: i64,
    unlimited_quota: bool,
    model_limits_enabled: bool,
    model_limits: String,
    allow_ips: Option<String>,
    used_quota: i64,
    group: String,
    cross_group_retry: bool,
}

const TOKEN_COLUMNS: &str = r#"id, user_id, key, status, name, created_time, accessed_time,
    expired_time, remain_quota, unlimited_quota, model_limits_enabled, model_limits,
    allow_ips, used_quota, "group", cross_group_retry"#;

impl TokenRow {
    fn into_domain(self) -> Token {
        Token {
            id: self.id,
            user_id: self.user_id,
            key: self.key.trim_end().to_string(),
            status: self.status as i32,
            name: self.name,
            created_time: self.created_time,
            accessed_time: self.accessed_time,
            expired_time: self.expired_time,
            remain_quota: self.remain_quota,
            unlimited_quota: self.unlimited_quota,
            model_limits_enabled: self.model_limits_enabled,
            model_limits: self.model_limits,
            allow_ips: self.allow_ips,
            used_quota: self.used_quota,
            group: self.group,
            cross_group_retry: self.cross_group_retry,
        }
    }
}

fn db_err(e: sqlx::Error) -> AppError {
    AppError::Database(e.to_string())
}

#[async_trait]
impl crate::traits::TokenRepository for PgTokenRepository {
    async fn find_by_key(&self, key: &str) -> AppResult<Option<Token>> {
        // NOTE(TDD): 当前无缓存层,直接按明文 key 查库。接入 Valkey 后入参改为
        // HMAC 摘要、缓存未命中再以明文回源,meth签名需同步调整(见 cache.rs)。
        let sql = format!(
            "SELECT {TOKEN_COLUMNS} FROM tokens WHERE key = $1 AND deleted_at IS NULL"
        );
        let row: Option<TokenRow> = sqlx::query_as(&sql)
            .bind(key)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        Ok(row.map(TokenRow::into_domain))
    }

    async fn find_by_id(&self, id: i64) -> AppResult<Option<Token>> {
        let sql = format!("SELECT {TOKEN_COLUMNS} FROM tokens WHERE id = $1 AND deleted_at IS NULL");
        let row: Option<TokenRow> = sqlx::query_as(&sql)
            .bind(id)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        Ok(row.map(TokenRow::into_domain))
    }

    async fn list_by_user(&self, user_id: i64, offset: i64, limit: i64) -> AppResult<Vec<Token>> {
        let sql = format!(
            "SELECT {TOKEN_COLUMNS} FROM tokens WHERE user_id = $1 AND deleted_at IS NULL \
             ORDER BY id DESC OFFSET $2 LIMIT $3"
        );
        let rows: Vec<TokenRow> = sqlx::query_as(&sql)
            .bind(user_id)
            .bind(offset)
            .bind(limit)
            .fetch_all(self.pool())
            .await
            .map_err(db_err)?;
        Ok(rows.into_iter().map(TokenRow::into_domain).collect())
    }

    async fn count_by_user(&self, user_id: i64) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tokens WHERE user_id = $1 AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(self.pool())
        .await
        .map_err(db_err)?;
        Ok(count)
    }

    async fn exists_by_name(&self, user_id: i64, name: &str) -> AppResult<bool> {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM tokens WHERE user_id = $1 AND name = $2 \
             AND deleted_at IS NULL)",
        )
        .bind(user_id)
        .bind(name)
        .fetch_one(self.pool())
        .await
        .map_err(db_err)?;
        Ok(exists)
    }

    async fn create(&self, new: NewToken) -> AppResult<i64> {
        let now = chrono::Utc::now().timestamp();
        let (id,): (i64,) = sqlx::query_as(
            r#"INSERT INTO tokens
                 (user_id, key, status, name, created_time, accessed_time, expired_time,
                  remain_quota, unlimited_quota, model_limits_enabled, model_limits,
                  allow_ips, used_quota, "group", cross_group_retry)
               VALUES ($1, $2, 1, $3, $4, 0, $5, $6, $7, $8, $9, $10, 0, $11, $12)
               RETURNING id"#,
        )
        .bind(new.user_id)
        .bind(&new.key)
        .bind(&new.name)
        .bind(now)
        .bind(new.expired_time)
        .bind(new.remain_quota)
        .bind(new.unlimited_quota)
        .bind(new.model_limits_enabled)
        .bind(&new.model_limits)
        .bind(&new.allow_ips)
        .bind(&new.group)
        .bind(new.cross_group_retry)
        .fetch_one(self.pool())
        .await
        .map_err(db_err)?;
        Ok(id)
    }

    async fn update(&self, token: &Token) -> AppResult<bool> {
        let result = sqlx::query(
            r#"UPDATE tokens SET status = $1, name = $2, expired_time = $3, remain_quota = $4,
                 unlimited_quota = $5, model_limits_enabled = $6, model_limits = $7,
                 allow_ips = $8, "group" = $9, cross_group_retry = $10
               WHERE id = $11 AND user_id = $12 AND deleted_at IS NULL"#,
        )
        .bind(token.status as i64)
        .bind(&token.name)
        .bind(token.expired_time)
        .bind(token.remain_quota)
        .bind(token.unlimited_quota)
        .bind(token.model_limits_enabled)
        .bind(&token.model_limits)
        .bind(&token.allow_ips)
        .bind(&token.group)
        .bind(token.cross_group_retry)
        .bind(token.id)
        .bind(token.user_id)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    async fn soft_delete(&self, id: i64, user_id: i64) -> AppResult<bool> {
        let result = sqlx::query(
            "UPDATE tokens SET deleted_at = now() WHERE id = $1 AND user_id = $2 \
             AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(user_id)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    async fn try_decrease_quota(&self, id: i64, amount: i64) -> AppResult<bool> {
        // 无限额度令牌不写库,恒可扣。
        let row: Option<(bool,)> = sqlx::query_as(
            "SELECT unlimited_quota FROM tokens WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        match row {
            None => Ok(false),
            Some((true,)) => Ok(true),
            Some((false,)) => {
                let result = sqlx::query(
                    "UPDATE tokens SET remain_quota = remain_quota - $1 \
                     WHERE id = $2 AND remain_quota >= $1 AND deleted_at IS NULL",
                )
                .bind(amount)
                .bind(id)
                .execute(self.pool())
                .await
                .map_err(db_err)?;
                Ok(result.rows_affected() > 0)
            }
        }
    }

    async fn increase_quota(&self, id: i64, amount: i64) -> AppResult<()> {
        sqlx::query(
            "UPDATE tokens SET remain_quota = remain_quota + $1 WHERE id = $2 \
             AND deleted_at IS NULL",
        )
        .bind(amount)
        .bind(id)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_accessed_time(&self, id: i64, ts: i64) -> AppResult<()> {
        sqlx::query("UPDATE tokens SET accessed_time = $1 WHERE id = $2 AND deleted_at IS NULL")
            .bind(ts)
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn invalidate_cache_by_user(&self, user_id: i64) -> AppResult<()> {
        // NOTE(TDD): 缓存层(Valkey)未接入,当前无可失效对象;接入后在此广播 chan。
        tracing::debug!(user_id, "令牌缓存失效请求(Valkey 未接入,当前为空操作)");
        Ok(())
    }
}
