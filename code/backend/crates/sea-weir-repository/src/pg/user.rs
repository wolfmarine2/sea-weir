//! `UserRepository` 的 openGauss 实现。额度扣减必须带 `AND quota >= $1` 守卫。

use async_trait::async_trait;
use sea_weir_types::domain::User;
use sea_weir_types::{AppError, AppResult};
use std::sync::Arc;

pub struct PgUserRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgUserRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }

    fn pool(&self) -> &sqlx::PgPool {
        &self.ctx.pools.main
    }

    /// 失效用户缓存(状态/角色/密码变更后调用)。
    async fn invalidate_user(&self, id: i64) {
        if let Some(cache) = self.ctx.cache.as_ref() {
            let _ = cache.del(&format!("user:{id}")).await;
        }
    }
}

/// 与 `users` 表逐列对应的行结构。`"group"` 是保留字,SQL 里双引号引用。
#[derive(sqlx::FromRow)]
struct UserRow {
    id: i64,
    username: String,
    password: String,
    display_name: String,
    role: i64,
    status: i64,
    email: String,
    github_id: String,
    discord_id: String,
    oidc_id: String,
    wechat_id: String,
    telegram_id: String,
    linux_do_id: String,
    access_token: Option<String>,
    quota: i64,
    used_quota: i64,
    request_count: i64,
    group: String,
    aff_code: Option<String>,
    aff_count: i64,
    aff_quota: i64,
    aff_history_quota: i64,
    inviter_id: Option<i64>,
    stripe_customer: String,
    setting: Option<serde_json::Value>,
    remark: String,
    created_at: i64,
    last_login_at: i64,
}

const USER_COLUMNS: &str = r#"id, username, password, display_name, role, status, email,
    github_id, discord_id, oidc_id, wechat_id, telegram_id, linux_do_id,
    access_token, quota, used_quota, request_count, "group", aff_code,
    aff_count, aff_quota, aff_history_quota, inviter_id, stripe_customer,
    setting, remark, created_at, last_login_at"#;

impl UserRow {
    fn into_domain(self) -> User {
        User {
            id: self.id,
            username: self.username,
            password: self.password,
            display_name: self.display_name,
            role: self.role as i32,
            status: self.status as i32,
            email: self.email,
            github_id: self.github_id,
            discord_id: self.discord_id,
            oidc_id: self.oidc_id,
            wechat_id: self.wechat_id,
            telegram_id: self.telegram_id,
            linux_do_id: self.linux_do_id,
            access_token: self.access_token,
            quota: self.quota,
            used_quota: self.used_quota,
            request_count: self.request_count,
            group: self.group,
            aff_code: self.aff_code.unwrap_or_default(),
            aff_count: self.aff_count,
            aff_quota: self.aff_quota,
            aff_history_quota: self.aff_history_quota,
            inviter_id: self.inviter_id,
            stripe_customer: self.stripe_customer,
            setting: self.setting.unwrap_or_else(|| serde_json::json!({})),
            remark: (!self.remark.is_empty()).then_some(self.remark),
            created_at: self.created_at,
            last_login_at: self.last_login_at,
        }
    }
}

fn db_err(e: sqlx::Error) -> AppError {
    AppError::Database(e.to_string())
}

#[async_trait]
impl crate::traits::UserRepository for PgUserRepository {
    async fn find_by_id(&self, id: i64) -> AppResult<Option<User>> {
        let cache_key = format!("user:{id}");
        // 读穿:先查 Valkey,未命中回源 DB 并回填(ADR-007,TTL 60s)。
        if let Some(cache) = self.ctx.cache.as_ref() {
            match cache.get(&cache_key).await {
                Ok(Some(json)) => {
                    if let Ok(user) = serde_json::from_str::<User>(&json) {
                        return Ok(Some(user));
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "读用户缓存失败,回源 DB"),
            }
        }

        let sql = format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1 AND deleted_at IS NULL");
        let row: Option<UserRow> = sqlx::query_as(&sql)
            .bind(id)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        let user = row.map(UserRow::into_domain);

        if let (Some(cache), Some(user)) = (self.ctx.cache.as_ref(), user.as_ref()) {
            // 序列化会跳过 password(明文不落缓存)。
            if let Ok(json) = serde_json::to_string(user) {
                if let Err(e) = cache
                    .set_ex(&cache_key, &json, crate::pool::CACHE_TTL_SECS)
                    .await
                {
                    tracing::warn!(error = %e, "写用户缓存失败");
                }
            }
        }
        Ok(user)
    }

    async fn find_by_username(&self, username: &str) -> AppResult<Option<User>> {
        let sql =
            format!("SELECT {USER_COLUMNS} FROM users WHERE username = $1 AND deleted_at IS NULL");
        let row: Option<UserRow> = sqlx::query_as(&sql)
            .bind(username)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        Ok(row.map(UserRow::into_domain))
    }

    async fn find_by_access_token(&self, token: &str) -> AppResult<Option<User>> {
        let sql = format!(
            "SELECT {USER_COLUMNS} FROM users WHERE access_token = $1 AND deleted_at IS NULL"
        );
        let row: Option<UserRow> = sqlx::query_as(&sql)
            .bind(token)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        Ok(row.map(UserRow::into_domain))
    }

    async fn try_decrease_quota(&self, id: i64, amount: i64) -> AppResult<bool> {
        // 条件原子扣减:0 行 = 余额不足。绝不依赖读-改-写。
        let result = sqlx::query(
            "UPDATE users SET quota = quota - $1 WHERE id = $2 AND quota >= $1 \
             AND deleted_at IS NULL",
        )
        .bind(amount)
        .bind(id)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    async fn increase_quota(&self, id: i64, amount: i64) -> AppResult<()> {
        sqlx::query("UPDATE users SET quota = quota + $1 WHERE id = $2 AND deleted_at IS NULL")
            .bind(amount)
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn accumulate_usage(
        &self,
        id: i64,
        used_quota: i64,
        request_count: i64,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE users SET used_quota = used_quota + $1, request_count = request_count + $2 \
             WHERE id = $3 AND deleted_at IS NULL",
        )
        .bind(used_quota)
        .bind(request_count)
        .bind(id)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_status(&self, id: i64, status: i32) -> AppResult<()> {
        sqlx::query("UPDATE users SET status = $1 WHERE id = $2 AND deleted_at IS NULL")
            .bind(status as i64)
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        self.invalidate_user(id).await;
        Ok(())
    }

    async fn soft_delete(&self, id: i64) -> AppResult<()> {
        sqlx::query("UPDATE users SET deleted_at = now() WHERE id = $1")
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        self.invalidate_user(id).await;
        Ok(())
    }

    async fn list_paged(&self, offset: i64, limit: i64) -> AppResult<Vec<User>> {
        let sql = format!(
            "SELECT {USER_COLUMNS} FROM users WHERE deleted_at IS NULL \
             ORDER BY id DESC OFFSET $1 LIMIT $2"
        );
        let rows: Vec<UserRow> = sqlx::query_as(&sql)
            .bind(offset)
            .bind(limit)
            .fetch_all(self.pool())
            .await
            .map_err(db_err)?;
        Ok(rows.into_iter().map(UserRow::into_domain).collect())
    }

    async fn count(&self) -> AppResult<i64> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL")
                .fetch_one(self.pool())
                .await
                .map_err(db_err)?;
        Ok(count)
    }

    async fn exists_username(&self, username: &str) -> AppResult<bool> {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM users WHERE username = $1 AND deleted_at IS NULL)",
        )
        .bind(username)
        .fetch_one(self.pool())
        .await
        .map_err(db_err)?;
        Ok(exists)
    }

    async fn update_admin_fields(
        &self,
        id: i64,
        role: i32,
        status: i32,
        display_name: &str,
        group: &str,
    ) -> AppResult<bool> {
        let result = sqlx::query(
            r#"UPDATE users SET role = $1, status = $2, display_name = $3, "group" = $4
               WHERE id = $5 AND deleted_at IS NULL"#,
        )
        .bind(role as i64)
        .bind(status as i64)
        .bind(display_name)
        .bind(group)
        .bind(id)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        if result.rows_affected() > 0 {
            self.invalidate_user(id).await;
        }
        Ok(result.rows_affected() > 0)
    }

    async fn set_password(&self, id: i64, password_hash: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET password = $1 WHERE id = $2 AND deleted_at IS NULL")
            .bind(password_hash)
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        self.invalidate_user(id).await;
        Ok(())
    }

    async fn create(
        &self,
        username: &str,
        password_hash: &str,
        role: i32,
        aff_code: &str,
    ) -> AppResult<i64> {
        let now = chrono::Utc::now().timestamp();
        // 显式列出全部 NOT NULL 列,不依赖列默认值 —— 兼容由早期 DDL 建出的表
        // (那些表可能没有 DEFAULT,省略列会触发 "null value ... violates not-null")。
        let (id,): (i64,) = sqlx::query_as(
            r#"INSERT INTO users
                 (username, password, display_name, role, status, email,
                  github_id, discord_id, oidc_id, wechat_id, telegram_id, linux_do_id,
                  aff_code, aff_count, aff_quota, aff_history_quota, inviter_id,
                  "group", quota, used_quota, request_count, stripe_customer, remark,
                  created_at, last_login_at)
               VALUES ($1, $2, $1, $3, 1, '',
                       '', '', '', '', '', '',
                       $4, 0, 0, 0, 0,
                       'default', 0, 0, 0, '', '',
                       $5, 0)
               RETURNING id"#,
        )
        .bind(username)
        .bind(password_hash)
        .bind(role as i64)
        .bind(aff_code)
        .bind(now)
        .fetch_one(self.pool())
        .await
        .map_err(db_err)?;
        Ok(id)
    }
}
