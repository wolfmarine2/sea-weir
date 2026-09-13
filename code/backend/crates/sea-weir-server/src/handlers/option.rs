//! `option` 域 handler:系统选项/倍率配置(RootAuth,CONTRACTS §7)。
//!
//! 已落地:`GET /api/option/`(全量 KV 快照)、`PUT /api/option/`(批量 upsert)。
//! PUT 后失效定价视图缓存,使倍率变更即时生效(多节点广播待 Valkey 接入)。
//! 待补:注册式分层配置组(`xxx_setting.yyy`)、单键删除、倍率同步。

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::Json;

use sea_weir_types::AppError;

use crate::app_state::ServerState;
use crate::middleware::auth::RootUser;
use crate::response;

/// `GET /api/option/`(RootAuth):返回全部选项的 `key → value` 映射。
pub async fn list(State(state): State<Arc<ServerState>>, _auth: RootUser) -> Response {
    let Some(options) = state.options.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    match options.load_all().await {
        Ok(pairs) => {
            let map: serde_json::Map<String, serde_json::Value> = pairs
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect();
            response::ok(serde_json::Value::Object(map))
        }
        Err(e) => response::err(e),
    }
}

/// `PUT /api/option/`(RootAuth):批量更新选项。
///
/// 请求体为 `{key: value}`;`value` 为字符串时原样存储,其他 JSON 值会被序列化后存储。
/// 更新后失效定价视图缓存。
pub async fn update(
    State(state): State<Arc<ServerState>>,
    _auth: RootUser,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let Some(options) = state.options.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    let Some(map) = body.as_object() else {
        return response::err(AppError::BadRequest("请求体应为 {key: value} 对象".into()));
    };
    if map.is_empty() {
        return response::err(AppError::BadRequest("没有需要更新的选项".into()));
    }

    for (key, value) in map {
        let stored = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if let Err(e) = options.upsert(key, &stored).await {
            return response::err(e);
        }
    }

    // 倍率等配置变更后立即使定价缓存失效(多节点广播待 Valkey)。
    state.pricing.invalidate().await;
    tracing::info!(count = map.len(), "选项已更新,定价缓存已失效");

    response::ok(serde_json::json!({ "updated": map.len() }))
}
