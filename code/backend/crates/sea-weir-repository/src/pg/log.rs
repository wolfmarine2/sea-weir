//! `LogRepository` 的 openGauss 实现。写入走日志库 pool。

use async_trait::async_trait;
use sea_weir_types::domain::Log;
use sea_weir_types::{AppError, AppResult};
use std::sync::Arc;

use crate::traits::log::{LogFilter, LogStat};

pub struct PgLogRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgLogRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }

    /// 日志库连接池(未独立配置时与主库同池)。
    fn pool(&self) -> &sqlx::PgPool {
        &self.ctx.pools.log
    }
}

#[derive(sqlx::FromRow)]
struct LogRow {
    id: i64,
    user_id: i64,
    created_at: i64,
    r#type: i64,
    content: String,
    username: String,
    token_name: String,
    model_name: String,
    quota: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    use_time: i64,
    is_stream: bool,
    channel_id: i64,
    token_id: i64,
    group: String,
    ip: String,
    request_id: String,
    other: Option<serde_json::Value>,
}

impl LogRow {
    fn into_domain(self) -> Log {
        Log {
            id: self.id,
            user_id: self.user_id,
            created_at: self.created_at,
            r#type: self.r#type as i32,
            content: self.content,
            username: self.username,
            token_name: self.token_name,
            model_name: self.model_name,
            quota: self.quota,
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            use_time: self.use_time as i32,
            is_stream: self.is_stream,
            channel_id: (self.channel_id != 0).then_some(self.channel_id),
            token_id: (self.token_id != 0).then_some(self.token_id),
            group: (!self.group.is_empty()).then_some(self.group),
            ip: (!self.ip.is_empty()).then_some(self.ip),
            request_id: (!self.request_id.is_empty()).then_some(self.request_id),
            other: self.other,
        }
    }
}

const LOG_COLUMNS: &str = r#"id, user_id, created_at, type, content, username, token_name,
    model_name, quota, prompt_tokens, completion_tokens, use_time, is_stream, channel_id,
    token_id, "group", ip, request_id, other"#;

fn db_err(e: sqlx::Error) -> AppError {
    AppError::Database(e.to_string())
}

enum FilterValue {
    I64(i64),
    Str(String),
}

/// 组装 WHERE 片段与绑定值(顺序与占位符一致)。
fn build_where(filter: &LogFilter) -> (String, Vec<FilterValue>) {
    let mut parts: Vec<String> = Vec::new();
    let mut binds: Vec<FilterValue> = Vec::new();

    macro_rules! cond {
        ($clause:expr, $v:expr) => {{
            binds.push($v);
            parts.push(format!("{} ${}", $clause, binds.len()));
        }};
    }

    if let Some(v) = filter.user_id {
        cond!("user_id =", FilterValue::I64(v));
    }
    if let Some(v) = filter.token_id {
        cond!("token_id =", FilterValue::I64(v));
    }
    if let Some(v) = filter.channel_id {
        cond!("channel_id =", FilterValue::I64(v));
    }
    if let Some(v) = filter.log_type {
        cond!("type =", FilterValue::I64(v as i64));
    }
    if let Some(v) = filter.model_name.as_deref().filter(|s| !s.is_empty()) {
        cond!("model_name LIKE", FilterValue::Str(format!("%{v}%")));
    }
    if let Some(v) = filter.start_ts {
        cond!("created_at >=", FilterValue::I64(v));
    }
    if let Some(v) = filter.end_ts {
        cond!("created_at <=", FilterValue::I64(v));
    }

    let where_sql = if parts.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", parts.join(" AND "))
    };
    (where_sql, binds)
}

/// 给 Query(SELECT 单值用)或 QueryAs(行用)按序绑定。
macro_rules! apply_binds {
    ($q:expr, $binds:expr) => {{
        let mut q = $q;
        for v in $binds {
            q = match v {
                FilterValue::I64(x) => q.bind(*x),
                FilterValue::Str(x) => q.bind(x.clone()),
            };
        }
        q
    }};
}

#[async_trait]
impl crate::traits::LogRepository for PgLogRepository {
    async fn record(&self, log: &Log) -> AppResult<()> {
        let now = chrono::Utc::now().timestamp();
        let created_at = if log.created_at > 0 { log.created_at } else { now };
        sqlx::query(
            r#"INSERT INTO logs
                 (user_id, created_at, type, content, username, token_name, model_name, quota,
                  prompt_tokens, completion_tokens, use_time, is_stream, channel_id, token_id,
                  "group", ip, request_id, other)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)"#,
        )
        .bind(log.user_id)
        .bind(created_at)
        .bind(log.r#type as i64)
        .bind(&log.content)
        .bind(&log.username)
        .bind(&log.token_name)
        .bind(&log.model_name)
        .bind(log.quota)
        .bind(log.prompt_tokens)
        .bind(log.completion_tokens)
        .bind(log.use_time as i64)
        .bind(log.is_stream)
        .bind(log.channel_id.unwrap_or(0))
        .bind(log.token_id.unwrap_or(0))
        .bind(log.group.clone().unwrap_or_default())
        .bind(log.ip.clone().unwrap_or_default())
        .bind(log.request_id.clone().unwrap_or_default())
        .bind(&log.other)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn search(
        &self,
        filter: &LogFilter,
        offset: i64,
        limit: i64,
    ) -> AppResult<(Vec<Log>, i64)> {
        let (where_sql, binds) = build_where(filter);

        let count_sql = format!("SELECT COUNT(*) FROM logs {where_sql}");
        let count_query = sqlx::query_as::<_, (i64,)>(&count_sql);
        let (total,): (i64,) = apply_binds!(count_query, &binds)
            .fetch_one(self.pool())
            .await
            .map_err(db_err)?;

        let rows_sql = format!(
            "SELECT {LOG_COLUMNS} FROM logs {where_sql} ORDER BY created_at DESC, id DESC \
             OFFSET ${} LIMIT ${}",
            binds.len() + 1,
            binds.len() + 2
        );
        let rows_query = sqlx::query_as::<_, LogRow>(&rows_sql);
        let rows: Vec<LogRow> = apply_binds!(rows_query, &binds)
            .bind(offset)
            .bind(limit)
            .fetch_all(self.pool())
            .await
            .map_err(db_err)?;

        Ok((rows.into_iter().map(LogRow::into_domain).collect(), total))
    }

    async fn stat(&self, filter: &LogFilter) -> AppResult<LogStat> {
        let (where_sql, binds) = build_where(filter);

        let quota_sql = format!("SELECT COALESCE(SUM(quota), 0) FROM logs {where_sql}");
        let quota_query = sqlx::query_as::<_, (i64,)>(&quota_sql);
        let (total_quota,): (i64,) = apply_binds!(quota_query, &binds)
            .fetch_one(self.pool())
            .await
            .map_err(db_err)?;

        // 最近 60s 的请求数 / token 数:在同一套过滤条件上把 start_ts 覆盖为时间窗。
        let since = chrono::Utc::now().timestamp() - 60;
        let mut recent = filter.clone();
        recent.start_ts = Some(since);
        let (recent_where, recent_binds) = build_where(&recent);
        let recent_sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(prompt_tokens + completion_tokens), 0) \
             FROM logs {recent_where}"
        );
        let recent_query = sqlx::query_as::<_, (i64, i64)>(&recent_sql);
        let (rpm, tpm): (i64, i64) = apply_binds!(recent_query, &recent_binds)
            .fetch_one(self.pool())
            .await
            .map_err(db_err)?;

        Ok(LogStat {
            total_quota,
            rpm,
            tpm,
        })
    }

    async fn delete_before(&self, ts: i64) -> AppResult<u64> {
        let result = sqlx::query("DELETE FROM logs WHERE created_at < $1")
            .bind(ts)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(result.rows_affected())
    }
}
