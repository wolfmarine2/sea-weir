//! `prefill_group` 域 handler:预填分组 CRUD(AdminAuth)。
//!
//! 预填分组把一批条目(model / tag / endpoint)命名保存,模型广场据此快速筛选。
//! 表 `prefill_groups` 按名称在未删除记录内唯一,删除为软删除。

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;

use sea_weir_types::domain::{PrefillGroup, PREFILL_TYPES};
use sea_weir_types::AppError;

use crate::app_state::ServerState;
use crate::middleware::auth::AdminUser;
use crate::response;

#[derive(Debug, Deserialize)]
pub struct PrefillGroupRequest {
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    /// model / tag / endpoint。
    pub r#type: String,
    #[serde(default)]
    pub items: Option<serde_json::Value>,
    #[serde(default)]
    pub description: String,
}

/// `GET /api/prefill_group`:列表(AdminAuth)。
pub async fn list(State(state): State<Arc<ServerState>>, _auth: AdminUser) -> Response {
    let Some(repo) = state.prefill_groups.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    match repo.list().await {
        Ok(items) => response::ok(serde_json::json!({ "items": items, "total": items.len() })),
        Err(e) => response::err(e),
    }
}

/// `POST /api/prefill_group`:新增(AdminAuth)。
pub async fn create(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Json(req): Json<PrefillGroupRequest>,
) -> Response {
    let Some(repo) = state.prefill_groups.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    if let Err(e) = validate(&req) {
        return response::err(e);
    }
    let name = req.name.trim();
    match repo.exists_name(name).await {
        Ok(true) => return response::err(AppError::Biz(format!("预填分组 {name} 已存在"))),
        Ok(false) => {}
        Err(e) => return response::err(e),
    }

    let now = chrono::Utc::now().timestamp();
    let group = PrefillGroup {
        id: 0,
        name: name.to_string(),
        r#type: req.r#type.trim().to_string(),
        items: normalize_items(req.items),
        description: req.description.trim().to_string(),
        created_time: now,
        updated_time: now,
    };
    match repo.create(&group).await {
        Ok(id) => response::ok(serde_json::json!({ "id": id })),
        Err(e) => response::err(e),
    }
}

/// `PUT /api/prefill_group`:修改(id 在 body;AdminAuth)。
pub async fn update(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Json(req): Json<PrefillGroupRequest>,
) -> Response {
    let Some(repo) = state.prefill_groups.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    let Some(id) = req.id else {
        return response::err(AppError::BadRequest("缺少 id".into()));
    };
    if let Err(e) = validate(&req) {
        return response::err(e);
    }
    let existing = match repo.find_by_id(id).await {
        Ok(Some(g)) => g,
        Ok(None) => return response::err(AppError::NotFound("预填分组不存在".into())),
        Err(e) => return response::err(e),
    };
    let name = req.name.trim();
    if name != existing.name {
        match repo.exists_name(name).await {
            Ok(true) => return response::err(AppError::Biz(format!("预填分组 {name} 已存在"))),
            Ok(false) => {}
            Err(e) => return response::err(e),
        }
    }

    let group = PrefillGroup {
        id,
        name: name.to_string(),
        r#type: req.r#type.trim().to_string(),
        items: normalize_items(req.items),
        description: req.description.trim().to_string(),
        created_time: existing.created_time,
        updated_time: chrono::Utc::now().timestamp(),
    };
    match repo.update(&group).await {
        Ok(()) => response::ok(serde_json::json!({ "id": id })),
        Err(e) => response::err(e),
    }
}

/// `DELETE /api/prefill_group/:id`:软删除(AdminAuth)。
pub async fn delete(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Path(id): Path<i64>,
) -> Response {
    let Some(repo) = state.prefill_groups.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    match repo.find_by_id(id).await {
        Ok(Some(_)) => {}
        Ok(None) => return response::err(AppError::NotFound("预填分组不存在".into())),
        Err(e) => return response::err(e),
    }
    match repo.delete(id).await {
        Ok(()) => response::ok(serde_json::json!({ "id": id })),
        Err(e) => response::err(e),
    }
}

/// 校验:名称非空 ≤64;type ∈ {model, tag, endpoint};items 若给出必须是数组。
fn validate(req: &PrefillGroupRequest) -> Result<(), AppError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("名称不能为空".into()));
    }
    if name.chars().count() > 64 {
        return Err(AppError::BadRequest("名称长度不能超过 64".into()));
    }
    let ty = req.r#type.trim();
    if ty.is_empty() {
        return Err(AppError::BadRequest("类型不能为空".into()));
    }
    if !PREFILL_TYPES.contains(&ty) {
        return Err(AppError::BadRequest(format!(
            "类型必须是 {} 之一",
            PREFILL_TYPES.join(" / ")
        )));
    }
    if let Some(items) = req.items.as_ref() {
        if !items.is_null() && !items.is_array() {
            return Err(AppError::BadRequest("items 必须是数组".into()));
        }
    }
    Ok(())
}

/// `items` 归一化:null → `Some([])`(建表列可空,但统一存空数组便于前端直接渲染)。
fn normalize_items(items: Option<serde_json::Value>) -> Option<serde_json::Value> {
    match items {
        None | Some(serde_json::Value::Null) => Some(serde_json::json!([])),
        Some(v) => Some(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn req(name: &str, ty: &str) -> PrefillGroupRequest {
        PrefillGroupRequest {
            id: None,
            name: name.into(),
            r#type: ty.into(),
            items: None,
            description: String::new(),
        }
    }

    #[test]
    fn validate_accepts_known_types() {
        for ty in PREFILL_TYPES {
            assert!(validate(&req("g", ty)).is_ok(), "{ty} 应被接受");
        }
        assert!(validate(&req("  g  ", "model")).is_ok(), "应容忍首尾空白");
    }

    #[test]
    fn validate_rejects_unknown_type_and_empty_name() {
        assert!(validate(&req("", "model")).is_err());
        assert!(validate(&req("g", "")).is_err());
        assert!(validate(&req("g", "vendor")).is_err(), "未知类型应拒绝");
        assert!(validate(&req(&"x".repeat(65), "tag")).is_err());
    }

    #[test]
    fn validate_items_must_be_array() {
        let mut r = req("g", "model");
        r.items = Some(json!({"a": 1}));
        assert!(validate(&r).is_err(), "对象不是合法 items");

        r.items = Some(json!(["a", "b"]));
        assert!(validate(&r).is_ok());

        r.items = Some(serde_json::Value::Null);
        assert!(validate(&r).is_ok(), "null 视为未填");
    }

    #[test]
    fn normalize_items_defaults_to_empty_array() {
        assert_eq!(normalize_items(None), Some(json!([])));
        assert_eq!(normalize_items(Some(serde_json::Value::Null)), Some(json!([])));
        assert_eq!(normalize_items(Some(json!(["x"]))), Some(json!(["x"])));
    }
}
