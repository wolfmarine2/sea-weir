//! `PrefillGroupRepository` 的 openGauss 实现。软删除,name 在未删除记录内唯一。

use async_trait::async_trait;
use sea_weir_types::{domain::PrefillGroup, AppError, AppResult};
use std::sync::Arc;

pub struct PgPrefillGroupRepository {
    pub(crate) ctx: Arc<crate::RepositoryContext>,
}

impl PgPrefillGroupRepository {
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

/// 列顺序与 [`PrefillGroup`] 字段一致。
const COLUMNS: &str = "id, name, type, items, description, created_time, updated_time";

type Row = (i64, String, String, Option<serde_json::Value>, String, i64, i64);

fn into_domain(r: Row) -> PrefillGroup {
    PrefillGroup {
        id: r.0,
        name: r.1,
        r#type: r.2,
        items: r.3,
        description: r.4,
        created_time: r.5,
        updated_time: r.6,
    }
}

#[async_trait]
impl crate::traits::PrefillGroupRepository for PgPrefillGroupRepository {
    async fn list(&self) -> AppResult<Vec<PrefillGroup>> {
        let rows: Vec<Row> = sqlx::query_as(&format!(
            "SELECT {COLUMNS} FROM prefill_groups WHERE deleted_at IS NULL ORDER BY id DESC"
        ))
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(into_domain).collect())
    }

    async fn find_by_id(&self, id: i64) -> AppResult<Option<PrefillGroup>> {
        let row: Option<Row> = sqlx::query_as(&format!(
            "SELECT {COLUMNS} FROM prefill_groups WHERE id = $1 AND deleted_at IS NULL"
        ))
        .bind(id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        Ok(row.map(into_domain))
    }

    async fn exists_name(&self, name: &str) -> AppResult<bool> {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM prefill_groups WHERE name = $1 AND deleted_at IS NULL)",
        )
        .bind(name)
        .fetch_one(self.pool())
        .await
        .map_err(db_err)?;
        Ok(exists)
    }

    async fn create(&self, group: &PrefillGroup) -> AppResult<i64> {
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO prefill_groups (name, type, items, description, created_time, updated_time) \
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind(&group.name)
        .bind(&group.r#type)
        .bind(&group.items)
        .bind(&group.description)
        .bind(group.created_time)
        .bind(group.updated_time)
        .fetch_one(self.pool())
        .await
        .map_err(db_err)?;
        Ok(id)
    }

    async fn update(&self, group: &PrefillGroup) -> AppResult<()> {
        sqlx::query(
            "UPDATE prefill_groups SET name = $1, type = $2, items = $3, description = $4, \
             updated_time = $5 WHERE id = $6 AND deleted_at IS NULL",
        )
        .bind(&group.name)
        .bind(&group.r#type)
        .bind(&group.items)
        .bind(&group.description)
        .bind(group.updated_time)
        .bind(group.id)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn delete(&self, id: i64) -> AppResult<()> {
        let now = chrono::Utc::now();
        sqlx::query("UPDATE prefill_groups SET deleted_at = $1 WHERE id = $2 AND deleted_at IS NULL")
            .bind(now)
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(())
    }
}
