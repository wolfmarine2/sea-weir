//! `system` 域 handler:全站状态与首装向导(system-design §9.2、CONTRACTS §附录 A)。
//!
//! 现状:`/api/status`、`/api/setup` 已落地;`/api/status/test`、公告、OAuth 等待补。

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::Json;
use serde::Deserialize;

use sea_weir_types::constants::role;
use sea_weir_types::AppError;

use crate::app_state::ServerState;
use crate::response;

/// `GET /api/status`(公开)。
///
/// 全站配置,前端 status store 的唯一来源。当前返回系统名/版本/首装状态等;
/// OAuth、支付开关、导航可见性等待 options 表落地后补全。
pub async fn status(State(state): State<Arc<ServerState>>) -> Response {
    let mut system_name = "sea-weir".to_string();
    let mut setup = false;

    if let Some(options) = state.options.as_ref() {
        if let Ok(Some(v)) = options.get("system_name").await {
            if !v.trim().is_empty() {
                system_name = v;
            }
        }
        if let Ok(Some(v)) = options.get("setup").await {
            setup = v == "true" || v == "1";
        }
    }
    // 首装与否以「是否已有账号」为准(首装账号用户名可自定义,不一定叫 root)。
    if let Some(users) = state.users.as_ref() {
        if let Ok(n) = users.count().await {
            setup = setup || n > 0;
        }
    }

    response::ok(serde_json::json!({
        "system_name": system_name,
        "logo": "",
        "version": env!("CARGO_PKG_VERSION"),
        "start_time": state.started,
        "setup": setup,
        "db_ready": state.db_ready(),
        // 数据库不可用时的原因(Oracle 兼容模式需重建 / 连接被拒等),便于线上直接定位。
        "db_error": state.db_error,
    }))
}

/// `GET /api/setup`(公开):是否已完成首装。
pub async fn get_setup(State(state): State<Arc<ServerState>>) -> Response {
    let root_init = match state.users.as_ref() {
        Some(users) => users.count().await.map(|n| n > 0).unwrap_or(false),
        None => false,
    };

    response::ok(serde_json::json!({
        "status": root_init,
        "root_init": root_init,
        "database_type": if state.db_ready() { "openGauss" } else { "" },
    }))
}

#[derive(Debug, Deserialize)]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
}

/// `POST /api/setup`(公开):未初始化时创建 root 并写初始化标记;已初始化拒绝。
pub async fn post_setup(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<SetupRequest>,
) -> Response {
    let users = match state.users.as_ref() {
        Some(users) => users,
        None => return response::err(AppError::Database("数据库未连接".into())),
    };

    match users.count().await {
        Ok(0) => {}
        Ok(_) => return response::err(AppError::Biz("系统已初始化".into())),
        Err(e) => return response::err(e),
    }

    let username = req.username.trim();
    if username.is_empty() {
        return response::err(AppError::BadRequest("用户名不能为空".into()));
    }
    if req.password.len() < 6 {
        return response::err(AppError::BadRequest("口令至少 6 位".into()));
    }

    let password_hash = match bcrypt::hash(&req.password, bcrypt::DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => return response::err(AppError::Internal(format!("口令哈希失败: {e}"))),
    };

    let aff = uuid::Uuid::new_v4().simple().to_string();
    let aff_code = &aff[..16];

    match users.create(username, &password_hash, role::ROOT, aff_code).await {
        Ok(id) => response::ok(serde_json::json!({ "id": id, "username": username })),
        Err(e) => response::err(e),
    }
}

// ───────────────────────── 运营文案(公开)─────────────────────────
//
// 与 new-api 一致:`data` 为字符串(可含 Markdown/HTML)。文案取自 options:
// Notice / About / UserAgreement / PrivacyPolicy / HomePageContent。

async fn text_option(state: &ServerState, key: &str) -> String {
    match state.options.as_ref() {
        Some(options) => options.get(key).await.ok().flatten().unwrap_or_default(),
        None => String::new(),
    }
}

/// `GET /api/notice`(公开):站内公告。
pub async fn notice(State(state): State<Arc<ServerState>>) -> Response {
    response::ok(text_option(&state, "Notice").await)
}

/// `GET /api/about`(公开):关于页文案。
pub async fn about(State(state): State<Arc<ServerState>>) -> Response {
    response::ok(text_option(&state, "About").await)
}

/// `GET /api/user-agreement`(公开):用户协议。
pub async fn user_agreement(State(state): State<Arc<ServerState>>) -> Response {
    response::ok(text_option(&state, "UserAgreement").await)
}

/// `GET /api/privacy-policy`(公开):隐私政策。
pub async fn privacy_policy(State(state): State<Arc<ServerState>>) -> Response {
    response::ok(text_option(&state, "PrivacyPolicy").await)
}

/// `GET /api/home_page_content`(公开):首页运营文案。
pub async fn home_page_content(State(state): State<Arc<ServerState>>) -> Response {
    response::ok(text_option(&state, "HomePageContent").await)
}
