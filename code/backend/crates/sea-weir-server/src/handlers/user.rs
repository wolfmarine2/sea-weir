//! `user` 域 handler。契约见 CONTRACTS.md §2;端点清单见附录 A。
//!
//! 现状:登录 / 登出 / 本人信息三条已落地,其余(注册、改密、2FA、passkey、
//! 签到、邀请等)待 TDD 补齐。

use std::sync::Arc;

use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, HeaderValue};
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

/// `GET /api/user/logout`:清会话 Cookie,并把当前会话 jti 写入吊销黑名单
/// (TTL=剩余有效期)。幂等,未登录也成功。
pub async fn logout(State(state): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    // 尽力吊销:能解析出会话则写黑名单,失败不影响登出。
    if let Some(token) = session::parse_cookie(&headers, session::SESSION_COOKIE) {
        if let Ok(claims) = state.sessions.verify(&token) {
            if let Some(cache) = state.cache.as_ref() {
                let remaining = claims.exp - chrono::Utc::now().timestamp();
                if remaining > 0 {
                    if let Err(e) = cache
                        .set_ex(&session::revoke_key(&claims.jti), "1", remaining)
                        .await
                    {
                        tracing::warn!(error = %e, "写会话吊销黑名单失败");
                    }
                }
            }
        }
    }

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

// ───────────────────────── 管理面:用户访问控制(AdminAuth)─────────────────────────
//
// 仅做访问控制(角色/状态/分组/密码),不含额度充值等支付相关操作。

use axum::extract::{Path, Query};
use sea_weir_types::constants::role;
use sea_weir_types::dto::common::PageQuery;

use crate::middleware::auth::AdminUser;

fn user_repo<'a>(
    state: &'a ServerState,
) -> Result<&'a Arc<dyn sea_weir_repository::UserRepository>, Response> {
    state
        .users
        .as_ref()
        .ok_or_else(|| response::err(AppError::Database("数据库未连接".into())))
}

/// 角色不足时不允许操作 root 账号(仅 root 可管理 root)。
fn guard_root(actor_role: i32, target_role: i32) -> Result<(), Response> {
    if target_role >= role::ROOT && actor_role < role::ROOT {
        return Err(response::err(AppError::Forbidden(
            "仅 root 可管理 root 账号".into(),
        )));
    }
    Ok(())
}

/// `GET /api/user/`(AdminAuth):用户分页列表(password 不参与序列化)。
pub async fn admin_list(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Query(q): Query<PageQuery>,
) -> Response {
    let repo = match user_repo(&state) {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let page = q.normalized_page();
    let page_size = if q.page_size <= 0 { 10 } else { q.page_size.min(100) };

    let users = match repo.list_paged((page - 1) * page_size, page_size).await {
        Ok(u) => u,
        Err(e) => return response::err(e),
    };
    let total = match repo.count().await {
        Ok(t) => t,
        Err(e) => return response::err(e),
    };
    let items: Vec<serde_json::Value> = users
        .into_iter()
        .map(|u| serde_json::to_value(&u).unwrap_or(serde_json::Value::Null))
        .collect();
    response::ok(serde_json::json!({
        "items": items, "total": total, "page": page, "page_size": page_size
    }))
}

/// `GET /api/user/:id`(AdminAuth)。
pub async fn admin_get(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Path(id): Path<i64>,
) -> Response {
    let repo = match user_repo(&state) {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    match repo.find_by_id(id).await {
        Ok(Some(user)) => response::ok(user),
        Ok(None) => response::err(AppError::NotFound("用户不存在".into())),
        Err(e) => response::err(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub role: Option<i32>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    /// 允许管理员自行指定初始邀请码(缺省随机生成)。
    #[serde(default)]
    pub aff_code: Option<String>,
}

/// `POST /api/user/`(AdminAuth):创建用户(额度为 0;本系统不涉及充值)。
pub async fn admin_create(
    State(state): State<Arc<ServerState>>,
    AdminUser(actor): AdminUser,
    Json(req): Json<CreateUserRequest>,
) -> Response {
    let repo = match user_repo(&state) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    let username = req.username.trim();
    if username.is_empty() {
        return response::err(AppError::BadRequest("用户名不能为空".into()));
    }
    if req.password.len() < 6 {
        return response::err(AppError::BadRequest("口令至少 6 位".into()));
    }
    let new_role = req.role.unwrap_or(role::COMMON);
    if !role::is_valid(new_role) {
        return response::err(AppError::BadRequest("角色取值非法".into()));
    }
    if let Err(resp) = guard_root(actor.role, new_role) {
        return resp;
    }

    match repo.exists_username(username).await {
        Ok(true) => return response::err(AppError::Biz("用户名已存在".into())),
        Ok(false) => {}
        Err(e) => return response::err(e),
    }

    let hash = match bcrypt::hash(&req.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => return response::err(AppError::Internal(format!("口令哈希失败: {e}"))),
    };
    let aff = req
        .aff_code
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string()[..16].to_string());

    let id = match repo.create(username, &hash, new_role, &aff).await {
        Ok(id) => id,
        Err(e) => return response::err(e),
    };

    // create() 默认 display_name=username、group=default;请求里带了的再覆盖一次。
    let display_name = req.display_name.filter(|s| !s.trim().is_empty());
    let group = req.group.filter(|s| !s.trim().is_empty());
    if display_name.is_some() || group.is_some() {
        let _ = repo
            .update_admin_fields(
                id,
                new_role,
                1,
                display_name.as_deref().unwrap_or(username),
                group.as_deref().unwrap_or("default"),
            )
            .await;
    }

    match repo.find_by_id(id).await {
        Ok(Some(user)) => response::ok(user),
        Ok(None) => response::ok(serde_json::json!({ "id": id })),
        Err(e) => response::err(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub id: i64,
    #[serde(default)]
    pub role: Option<i32>,
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    /// 非空则重置密码。
    #[serde(default)]
    pub password: Option<String>,
}

/// `PUT /api/user/`(AdminAuth):更新角色/状态/显示名/分组/密码(id 在 body)。
pub async fn admin_update(
    State(state): State<Arc<ServerState>>,
    AdminUser(actor): AdminUser,
    Json(req): Json<UpdateUserRequest>,
) -> Response {
    let repo = match user_repo(&state) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    let target = match repo.find_by_id(req.id).await {
        Ok(Some(u)) => u,
        Ok(None) => return response::err(AppError::NotFound("用户不存在".into())),
        Err(e) => return response::err(e),
    };
    if let Err(resp) = guard_root(actor.role, target.role) {
        return resp;
    }

    let new_role = req.role.unwrap_or(target.role);
    if !role::is_valid(new_role) {
        return response::err(AppError::BadRequest("角色取值非法".into()));
    }
    // 提权到 root 也要求操作者是 root。
    if let Err(resp) = guard_root(actor.role, new_role) {
        return resp;
    }
    let new_status = req.status.unwrap_or(target.status);
    let display_name = req
        .display_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(target.display_name);
    let group = req.group.filter(|s| !s.trim().is_empty()).unwrap_or(target.group);

    let password_hash = match req.password.filter(|p| !p.is_empty()) {
        Some(p) => {
            if p.len() < 6 {
                return response::err(AppError::BadRequest("口令至少 6 位".into()));
            }
            match bcrypt::hash(&p, bcrypt::DEFAULT_COST) {
                Ok(h) => Some(h),
                Err(e) => {
                    return response::err(AppError::Internal(format!("口令哈希失败: {e}")))
                }
            }
        }
        None => None,
    };

    match repo
        .update_admin_fields(req.id, new_role, new_status, &display_name, &group)
        .await
    {
        Ok(true) => {
            if let Some(hash) = password_hash.as_deref() {
                if let Err(e) = repo.set_password(req.id, hash).await {
                    return response::err(e);
                }
            }
            match repo.find_by_id(req.id).await {
                Ok(Some(user)) => response::ok(user),
                Ok(None) => response::err(AppError::NotFound("用户不存在".into())),
                Err(e) => response::err(e),
            }
        }
        Ok(false) => response::err(AppError::NotFound("用户不存在".into())),
        Err(e) => response::err(e),
    }
}

/// `DELETE /api/user/:id`(AdminAuth):软删除用户。
pub async fn admin_delete(
    State(state): State<Arc<ServerState>>,
    AdminUser(actor): AdminUser,
    Path(id): Path<i64>,
) -> Response {
    let repo = match user_repo(&state) {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let target = match repo.find_by_id(id).await {
        Ok(Some(u)) => u,
        Ok(None) => return response::err(AppError::NotFound("用户不存在".into())),
        Err(e) => return response::err(e),
    };
    if let Err(resp) = guard_root(actor.role, target.role) {
        return resp;
    }
    if target.id == actor.user_id {
        return response::err(AppError::BadRequest("不能删除自己".into()));
    }
    match repo.soft_delete(id).await {
        Ok(()) => response::ok(serde_json::json!({})),
        Err(e) => response::err(e),
    }
}
