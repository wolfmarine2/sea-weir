//! `ChannelRepository` 的 openGauss 实现。abilities 同步走事务先删后插。

use async_trait::async_trait;
use sea_weir_types::domain::Channel;
use sea_weir_types::{AppError, AppResult};
use std::sync::Arc;

pub struct PgChannelRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgChannelRepository {
    pub fn new(ctx: Arc<crate::RepositoryContext>) -> Self {
        Self { ctx }
    }

    fn pool(&self) -> &sqlx::PgPool {
        &self.ctx.pools.main
    }
}

#[derive(sqlx::FromRow)]
struct ChannelRow {
    id: i64,
    #[sqlx(rename = "type")]
    r#type: i64,
    key: String,
    openai_organization: String,
    test_model: String,
    status: i64,
    name: String,
    weight: i64,
    created_time: i64,
    test_time: i64,
    response_time: i64,
    base_url: String,
    balance: f64,
    balance_updated_time: i64,
    models: String,
    group: String,
    used_quota: i64,
    model_mapping: Option<serde_json::Value>,
    status_code_mapping: String,
    priority: i64,
    auto_ban: i64,
    other_info: String,
    tag: Option<String>,
    setting: Option<serde_json::Value>,
    param_override: Option<serde_json::Value>,
    header_override: Option<serde_json::Value>,
    channel_info: Option<serde_json::Value>,
    settings: Option<serde_json::Value>,
}

const CHANNEL_COLUMNS: &str = r#"id, type, key, openai_organization, test_model, status, name,
    weight, created_time, test_time, response_time, base_url, balance, balance_updated_time,
    models, "group", used_quota, model_mapping, status_code_mapping, priority, auto_ban,
    other_info, tag, setting, param_override, header_override, channel_info, settings"#;

impl ChannelRow {
    fn into_domain(self) -> Channel {
        Channel {
            id: self.id,
            r#type: self.r#type as i32,
            key: self.key,
            status: self.status as i32,
            name: self.name,
            weight: self.weight,
            priority: self.priority,
            group: self.group,
            models: self.models,
            model_mapping: self.model_mapping,
            param_override: self.param_override,
            header_override: self.header_override,
            base_url: (!self.base_url.is_empty()).then_some(self.base_url),
            openai_organization: (!self.openai_organization.is_empty())
                .then_some(self.openai_organization),
            test_model: (!self.test_model.is_empty()).then_some(self.test_model),
            balance: self.balance,
            balance_updated_time: self.balance_updated_time,
            used_quota: self.used_quota,
            auto_ban: self.auto_ban as i32,
            tag: self.tag,
            setting: self.setting,
            other_settings: self.settings,
            channel_info: self.channel_info,
            status_code_mapping: (!self.status_code_mapping.is_empty())
                .then_some(self.status_code_mapping),
            other_info: (!self.other_info.is_empty()).then_some(self.other_info),
            created_time: self.created_time,
            test_time: self.test_time,
            response_time: self.response_time,
        }
    }
}

fn db_err(e: sqlx::Error) -> AppError {
    AppError::Database(e.to_string())
}

fn split_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

#[async_trait]
impl crate::traits::ChannelRepository for PgChannelRepository {
    async fn list_enabled(&self) -> AppResult<Vec<Channel>> {
        let sql = format!(
            "SELECT {CHANNEL_COLUMNS} FROM channels WHERE status = 1 ORDER BY priority DESC, id"
        );
        let rows: Vec<ChannelRow> = sqlx::query_as(&sql)
            .fetch_all(self.pool())
            .await
            .map_err(db_err)?;
        Ok(rows.into_iter().map(ChannelRow::into_domain).collect())
    }

    async fn find_by_id(&self, id: i64) -> AppResult<Option<Channel>> {
        let sql = format!("SELECT {CHANNEL_COLUMNS} FROM channels WHERE id = $1");
        let row: Option<ChannelRow> = sqlx::query_as(&sql)
            .bind(id)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        Ok(row.map(ChannelRow::into_domain))
    }

    async fn list_paged(&self, offset: i64, limit: i64) -> AppResult<Vec<Channel>> {
        let sql = format!(
            "SELECT {CHANNEL_COLUMNS} FROM channels ORDER BY id DESC OFFSET $1 LIMIT $2"
        );
        let rows: Vec<ChannelRow> = sqlx::query_as(&sql)
            .bind(offset)
            .bind(limit)
            .fetch_all(self.pool())
            .await
            .map_err(db_err)?;
        Ok(rows.into_iter().map(ChannelRow::into_domain).collect())
    }

    async fn count(&self) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM channels")
            .fetch_one(self.pool())
            .await
            .map_err(db_err)?;
        Ok(count)
    }

    async fn exists_name(&self, name: &str) -> AppResult<bool> {
        let (exists,): (bool,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM channels WHERE name = $1)")
                .bind(name)
                .fetch_one(self.pool())
                .await
                .map_err(db_err)?;
        Ok(exists)
    }

    async fn create(&self, channel: &Channel) -> AppResult<i64> {
        let now = chrono::Utc::now().timestamp();
        let (id,): (i64,) = sqlx::query_as(
            r#"INSERT INTO channels
                 (type, key, openai_organization, test_model, status, name, weight, created_time,
                  test_time, response_time, base_url, balance, balance_updated_time, models,
                  "group", used_quota, model_mapping, status_code_mapping, priority, auto_ban,
                  other_info, tag, setting, param_override, header_override, channel_info,
                  settings, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,
                       $21,$22,$23,$24,$25,$26,$27,$28,$28)
               RETURNING id"#,
        )
        .bind(channel.r#type as i64)
        .bind(&channel.key)
        .bind(channel.openai_organization.clone().unwrap_or_default())
        .bind(channel.test_model.clone().unwrap_or_default())
        .bind(channel.status as i64)
        .bind(&channel.name)
        .bind(channel.weight)
        .bind(now)
        .bind(channel.test_time)
        .bind(channel.response_time)
        .bind(channel.base_url.clone().unwrap_or_default())
        .bind(channel.balance)
        .bind(channel.balance_updated_time)
        .bind(&channel.models)
        .bind(&channel.group)
        .bind(channel.used_quota)
        .bind(&channel.model_mapping)
        .bind(channel.status_code_mapping.clone().unwrap_or_default())
        .bind(channel.priority)
        .bind(channel.auto_ban as i64)
        .bind(channel.other_info.clone().unwrap_or_default())
        .bind(&channel.tag)
        .bind(&channel.setting)
        .bind(&channel.param_override)
        .bind(&channel.header_override)
        .bind(&channel.channel_info)
        .bind(&channel.other_settings)
        .bind(now)
        .fetch_one(self.pool())
        .await
        .map_err(db_err)?;

        let mut created = channel.clone();
        created.id = id;
        self.sync_abilities(&created).await?;
        Ok(id)
    }

    async fn update(&self, channel: &Channel) -> AppResult<()> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"UPDATE channels SET type = $1, key = $2, openai_organization = $3, test_model = $4,
                 status = $5, name = $6, weight = $7, base_url = $8, models = $9, "group" = $10,
                 model_mapping = $11, status_code_mapping = $12, priority = $13, auto_ban = $14,
                 other_info = $15, tag = $16, setting = $17, param_override = $18,
                 header_override = $19, channel_info = $20, settings = $21, updated_at = $22
               WHERE id = $23"#,
        )
        .bind(channel.r#type as i64)
        .bind(&channel.key)
        .bind(channel.openai_organization.clone().unwrap_or_default())
        .bind(channel.test_model.clone().unwrap_or_default())
        .bind(channel.status as i64)
        .bind(&channel.name)
        .bind(channel.weight)
        .bind(channel.base_url.clone().unwrap_or_default())
        .bind(&channel.models)
        .bind(&channel.group)
        .bind(&channel.model_mapping)
        .bind(channel.status_code_mapping.clone().unwrap_or_default())
        .bind(channel.priority)
        .bind(channel.auto_ban as i64)
        .bind(channel.other_info.clone().unwrap_or_default())
        .bind(&channel.tag)
        .bind(&channel.setting)
        .bind(&channel.param_override)
        .bind(&channel.header_override)
        .bind(&channel.channel_info)
        .bind(&channel.other_settings)
        .bind(now)
        .bind(channel.id)
        .execute(self.pool())
        .await
        .map_err(db_err)?;

        self.sync_abilities(channel).await
    }

    async fn delete(&self, id: i64) -> AppResult<()> {
        let mut tx = self.pool().begin().await.map_err(db_err)?;
        sqlx::query("DELETE FROM abilities WHERE channel_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM channels WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    async fn update_status(&self, id: i64, new_status: i32, reason: &str) -> AppResult<()> {
        let now = chrono::Utc::now().timestamp();
        let info = serde_json::json!({ "reason": reason, "ts": now }).to_string();
        sqlx::query("UPDATE channels SET status = $1, other_info = $2, updated_at = $3 WHERE id = $4")
            .bind(new_status as i64)
            .bind(info)
            .bind(now)
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        // 状态变更后重建 abilities,保证索引与状态一致。
        if let Some(channel) = self.find_by_id(id).await? {
            self.sync_abilities(&channel).await?;
        }
        Ok(())
    }

    async fn disable_key(&self, id: i64, key_index: usize, reason: &str) -> AppResult<()> {
        let now = chrono::Utc::now().timestamp();
        let mut info = self
            .find_by_id(id)
            .await?
            .and_then(|c| c.channel_info)
            .unwrap_or_else(|| serde_json::json!({}));
        if !info.is_object() {
            info = serde_json::json!({});
        }
        let obj = info.as_object_mut().expect("上面已保证是 object");
        let disabled = obj
            .entry("key_disabled")
            .or_insert_with(|| serde_json::json!({}));
        if let Some(map) = disabled.as_object_mut() {
            map.insert(key_index.to_string(), serde_json::json!({ "reason": reason, "ts": now }));
        }
        sqlx::query("UPDATE channels SET channel_info = $1, updated_at = $2 WHERE id = $3")
            .bind(&info)
            .bind(now)
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn sync_abilities(&self, channel: &Channel) -> AppResult<()> {
        let mut tx = self.pool().begin().await.map_err(db_err)?;
        sqlx::query("DELETE FROM abilities WHERE channel_id = $1")
            .bind(channel.id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;

        if channel.status == sea_weir_types::constants::status::ENABLED {
            let models = split_csv(&channel.models);
            let groups = split_csv(&channel.group);
            for group in &groups {
                for model in &models {
                    sqlx::query(
                        r#"INSERT INTO abilities
                             ("group", model, channel_id, enabled, priority, weight, tag, channel_type)
                           VALUES ($1, $2, $3, TRUE, $4, $5, $6, $7)"#,
                    )
                    .bind(group)
                    .bind(model)
                    .bind(channel.id)
                    .bind(channel.priority)
                    .bind(channel.weight)
                    .bind(&channel.tag)
                    .bind(channel.r#type as i64)
                    .execute(&mut *tx)
                    .await
                    .map_err(db_err)?;
                }
            }
        }

        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    async fn update_balance(&self, id: i64, balance: f64, ts: i64) -> AppResult<()> {
        sqlx::query("UPDATE channels SET balance = $1, balance_updated_time = $2 WHERE id = $3")
            .bind(balance)
            .bind(ts)
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn record_test_result(&self, id: i64, response_time_ms: i64, ts: i64) -> AppResult<()> {
        sqlx::query("UPDATE channels SET response_time = $1, test_time = $2 WHERE id = $3")
            .bind(response_time_ms)
            .bind(ts)
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(())
    }
}
