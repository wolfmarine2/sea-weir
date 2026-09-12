//! `token` 域 handler:令牌管理(UserAuth,CONTRACTS §3)。
//!
//! 已落地:列表(分页)/ 创建 / 更新 / 软删除 / 明文 key 揭示。
//! 响应中 key 一律走 `mask_token_key` 脱敏,仅 `/:id/key` 返回明文。

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::Json;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};

use sea_weir_repository::TokenRepository;
use sea_weir_types::constants::TOKEN_KEY_LEN;
use sea_weir_types::domain::{mask_token_key, NewToken, Token};
use sea_weir_types::dto::common::PageQuery;
use sea_weir_types::AppError;

use crate::app_state::ServerState;
use crate::middleware::auth::AuthUser;
use crate::response;

/// 列表项:字段与 `Token` 一致,`key` 换成脱敏值。
#[derive(Debug, Serialize)]
struct TokenItem {
    id: i64,
    user_id: i64,
    key: String,
    status: i32,
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

impl From<Token> for TokenItem {
    fn from(t: Token) -> Self {
        Self {
            id: t.id,
            user_id: t.user_id,
            key: mask_token_key(&t.key),
            status: t.status,
            name: t.name,
            created_time: t.created_time,
            accessed_time: t.accessed_time,
            expired_time: t.expired_time,
            remain_quota: t.remain_quota,
            unlimited_quota: t.unlimited_quota,
            model_limits_enabled: t.model_limits_enabled,
            model_limits: t.model_limits,
            allow_ips: t.allow_ips,
            used_quota: t.used_quota,
            group: t.group,
            cross_group_retry: t.cross_group_retry,
        }
    }
}

fn token_repo(state: &ServerState) -> Result<Arc<dyn TokenRepository>, Response> {
    state
        .tokens
        .as_ref()
        .cloned()
        .ok_or_else(|| response::err(AppError::Database("数据库未连接".into())))
}

/// 48 位随机 key(`sk-` 前缀不入库,由调用方拼接)。
fn generate_key() -> String {
    let mut rng = rand::thread_rng();
    (0..TOKEN_KEY_LEN)
        .map(|_| rng.sample(Alphanumeric) as char)
        .collect()
}

/// `GET /api/token/`、`GET /api/token/search`:本人令牌分页(key 脱敏)。
pub async fn list(
    State(state): State<Arc<ServerState>>,
    auth: AuthUser,
    Query(q): Query<PageQuery>,
) -> Response {
    let repo = match token_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };

    let page = q.normalized_page();
    let page_size = if q.page_size <= 0 {
        10
    } else {
        q.page_size.min(100)
    };
    let offset = (page - 1) * page_size;

    let rows = match repo.list_by_user(auth.user_id, offset, page_size).await {
        Ok(rows) => rows,
        Err(e) => return response::err(e),
    };
    let total = match repo.count_by_user(auth.user_id).await {
        Ok(total) => total,
        Err(e) => return response::err(e),
    };
    let items: Vec<TokenItem> = rows.into_iter().map(TokenItem::from).collect();

    response::ok(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "page_size": page_size,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    pub name: String,
    #[serde(default)]
    pub remain_quota: Option<i64>,
    #[serde(default)]
    pub unlimited_quota: Option<bool>,
    #[serde(default)]
    pub expired_time: Option<i64>,
    #[serde(default)]
    pub model_limits_enabled: Option<bool>,
    #[serde(default)]
    pub model_limits: Option<String>,
    #[serde(default)]
    pub allow_ips: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub cross_group_retry: Option<bool>,
}

/// `POST /api/token/`:创建本人令牌。名称在本人的未删除令牌中唯一。
pub async fn create(
    State(state): State<Arc<ServerState>>,
    auth: AuthUser,
    Json(req): Json<CreateTokenRequest>,
) -> Response {
    let repo = match token_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };

    let name = req.name.trim();
    if name.is_empty() {
        return response::err(AppError::BadRequest("令牌名称不能为空".into()));
    }
    match repo.exists_by_name(auth.user_id, name).await {
        Ok(true) => return response::err(AppError::Biz("令牌名称已存在".into())),
        Ok(false) => {}
        Err(e) => return response::err(e),
    }

    let new = NewToken {
        user_id: auth.user_id,
        key: generate_key(),
        name: name.to_string(),
        expired_time: req.expired_time.unwrap_or(-1),
        remain_quota: req.remain_quota.unwrap_or(0),
        unlimited_quota: req.unlimited_quota.unwrap_or(false),
        model_limits_enabled: req.model_limits_enabled.unwrap_or(false),
        model_limits: req.model_limits.unwrap_or_default(),
        allow_ips: req.allow_ips.filter(|s| !s.trim().is_empty()),
        group: req.group.unwrap_or_default(),
        cross_group_retry: req.cross_group_retry.unwrap_or(false),
    };

    let id = match repo.create(new).await {
        Ok(id) => id,
        Err(e) => return response::err(e),
    };
    match repo.find_by_id(id).await {
        Ok(Some(token)) => response::ok(TokenItem::from(token)),
        Ok(None) => response::err(AppError::Internal("创建后查询令牌失败".into())),
        Err(e) => response::err(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateTokenRequest {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub expired_time: Option<i64>,
    #[serde(default)]
    pub remain_quota: Option<i64>,
    #[serde(default)]
    pub unlimited_quota: Option<bool>,
    #[serde(default)]
    pub model_limits_enabled: Option<bool>,
    #[serde(default)]
    pub model_limits: Option<String>,
    #[serde(default)]
    pub allow_ips: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub cross_group_retry: Option<bool>,
}

/// `PUT /api/token/`:全量更新本人令牌(仅提供的字段被覆盖)。
pub async fn update(
    State(state): State<Arc<ServerState>>,
    auth: AuthUser,
    Json(req): Json<UpdateTokenRequest>,
) -> Response {
    let repo = match token_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };

    let mut token = match repo.find_by_id(req.id).await {
        Ok(Some(t)) if t.user_id == auth.user_id => t,
        Ok(_) => return response::err(AppError::NotFound("令牌不存在".into())),
        Err(e) => return response::err(e),
    };

    if let Some(name) = req.name {
        if name.trim().is_empty() {
            return response::err(AppError::BadRequest("令牌名称不能为空".into()));
        }
        token.name = name;
    }
    if let Some(status) = req.status {
        token.status = status;
    }
    if let Some(expired) = req.expired_time {
        token.expired_time = expired;
    }
    if let Some(quota) = req.remain_quota {
        token.remain_quota = quota;
    }
    if let Some(unlimited) = req.unlimited_quota {
        token.unlimited_quota = unlimited;
    }
    if let Some(enabled) = req.model_limits_enabled {
        token.model_limits_enabled = enabled;
    }
    if let Some(limits) = req.model_limits {
        token.model_limits = limits;
    }
    if let Some(ips) = req.allow_ips {
        token.allow_ips = (!ips.trim().is_empty()).then_some(ips);
    }
    if let Some(group) = req.group {
        token.group = group;
    }
    if let Some(cross) = req.cross_group_retry {
        token.cross_group_retry = cross;
    }

    match repo.update(&token).await {
        Ok(true) => response::ok(TokenItem::from(token)),
        Ok(false) => response::err(AppError::NotFound("令牌不存在".into())),
        Err(e) => response::err(e),
    }
}

/// `DELETE /api/token/:id`:软删除本人令牌。
pub async fn delete(
    State(state): State<Arc<ServerState>>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Response {
    let repo = match token_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };
    match repo.soft_delete(id, auth.user_id).await {
        Ok(true) => response::ok(serde_json::json!({})),
        Ok(false) => response::err(AppError::NotFound("令牌不存在".into())),
        Err(e) => response::err(e),
    }
}

/// `POST /api/token/:id/key`:返回完整 key(**唯一**明文出口)。
pub async fn reveal_key(
    State(state): State<Arc<ServerState>>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Response {
    let repo = match token_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };
    match repo.find_by_id(id).await {
        Ok(Some(t)) if t.user_id == auth.user_id => {
            response::ok(serde_json::json!({ "key": t.key }))
        }
        Ok(_) => response::err(AppError::NotFound("令牌不存在".into())),
        Err(e) => response::err(e),
    }
}
