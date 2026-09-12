//! `user` 域 handler。契约见 CONTRACTS.md §2;端点清单见附录 A。
//!
//! 现状:登录 / 登出 / 本人信息三条已落地,其余(注册、改密、2FA、passkey、
//! 签到、邀请等)待 TDD 补齐。

use std::sync::Arc;

use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::HeaderValue;
use axum::response::Response;
use axum::Json;
use serde::Deserialize;

use sea_weir_types::AppError;

use crate::app_state::ServerState;
use crate::middleware::auth::AuthUser;
use crate::response;
use crate::session;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

fn set_cookie(response: &mut Response, value: String) {
    match HeaderValue::from_str(&value) {
        Ok(v) => {
            response.headers_mut().insert(SET_COOKIE, v);
        }
        Err(e) => tracing::warn!(error = %e, "构造 Set-Cookie 失败"),
    }
}

/// `POST /api/user/login`(公开):账密校验 → 写签名会话 Cookie → 返回用户对象。
///
/// 密码错误按契约返回 HTTP 200 + `success:false`(业务错误),不泄漏用户是否存在。
pub async fn login(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let users = match state.users.as_ref() {
        Some(users) => users,
        None => return response::err(AppError::Database("数据库未连接".into())),
    };

    let user = match users.find_by_username(req.username.trim()).await {
        Ok(Some(user)) => user,
        Ok(None) => return response::err(AppError::Biz("用户名或密码错误".into())),
        Err(e) => return response::err(e),
    };

    if !bcrypt::verify(&req.password, &user.password).unwrap_or(false) {
        return response::err(AppError::Biz("用户名或密码错误".into()));
    }
    if !user.is_enabled() {
        return response::err(AppError::Unauthorized("用户已被封禁".into()));
    }

    let token = match state.sessions.issue(user.id, &user.username, user.role) {
        Ok(token) => token,
        Err(e) => return response::err(e),
    };

    let mut response = response::ok(user);
    set_cookie(
        &mut response,
        session::session_cookie(&token, state.sessions.ttl_secs()),
    );
    response
}

/// `GET /api/user/logout`:清会话 Cookie。幂等,未登录也成功。
///
/// NOTE(TDD): jti 写入 Valkey 吊销列表(ADR-004)待 cache 层接入;
/// 当前仅清 Cookie,服务端不保留吊销状态。
pub async fn logout() -> Response {
    let mut response = response::ok(serde_json::json!({}));
    set_cookie(&mut response, session::clear_cookie());
    response
}

/// `GET /api/user/self`(UserAuth):返回本人信息(password 不参与序列化)。
pub async fn self_info(
    State(state): State<Arc<ServerState>>,
    auth: AuthUser,
) -> Response {
    let users = match state.users.as_ref() {
        Some(users) => users,
        None => return response::err(AppError::Database("数据库未连接".into())),
    };

    match users.find_by_id(auth.user_id).await {
        Ok(Some(user)) => response::ok(user),
        Ok(None) => response::err(AppError::Unauthorized("用户不存在".into())),
        Err(e) => response::err(e),
    }
}
