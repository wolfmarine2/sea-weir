//! 认证与角色闸门。见 CONTRACTS.md §1、ADR-004。
//!
//! ## 管理面(AUTH-MW)
//! 前置:签名会话 Cookie 有效且未过期、jti 不在 Valkey 吊销列表、
//! `New-Api-User` 头与会话 user id 一致(防串号)、用户 `status == 1`。
//! 无会话时回落 `Authorization: Bearer <access_token>`。
//! 后置:请求扩展注入 `{user_id, username, role, group}`。
//!
//! ## 中继面(TOKEN-MW)
//! sk-token 提取顺序:
//! `Authorization: Bearer sk-…` → `x-api-key`(Claude)→ `?key=` / `x-goog-api-key`(Gemini)
//! → `Sec-WebSocket-Protocol`(realtime)→ `mj-api-secret`(MJ)。
//! `sk-xxx-{channelId}` 后缀指定渠道**仅 admin/root 可用**。

use axum::extract::FromRequestParts;
use sea_weir_types::constants::role;
use sea_weir_types::{AppError, AppResult};

use crate::app_state::ServerState;
use crate::response;

/// 已认证的调用者身份。注入请求扩展。
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub username: String,
    pub role: i32,
    pub group: String,
}

/// 中继面令牌上下文。
#[derive(Debug, Clone)]
pub struct AuthToken {
    pub token_id: i64,
    pub token_name: String,
    pub user_id: i64,
    pub username: String,
    pub group: String,
    pub model_limits: Option<Vec<String>>,
    pub remain_quota: i64,
    pub unlimited_quota: bool,
    pub specific_channel_id: Option<i64>,
}

/// 管理面会话鉴权提取器。
///
/// 认证顺序:会话 Cookie → `Authorization: Bearer <access_token>`;
/// 两条路径都要求 `New-Api-User` 与会话/令牌归属用户一致(防串号)。
impl axum::extract::FromRequestParts<std::sync::Arc<ServerState>> for AuthUser {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &std::sync::Arc<ServerState>,
    ) -> Result<Self, Self::Rejection> {
        let users = state
            .users
            .as_ref()
            .ok_or_else(|| response::err(AppError::Database("数据库未连接,鉴权不可用".into())))?;

        let user = if let Some(token) =
            crate::session::parse_cookie(&parts.headers, crate::session::SESSION_COOKIE)
        {
            let claims = state.sessions.verify(&token).map_err(response::err)?;
            require_new_api_user(&parts.headers, claims.sub)?;
            users
                .find_by_id(claims.sub)
                .await
                .map_err(response::err)?
                .ok_or_else(|| response::err(AppError::Unauthorized("用户不存在".into())))?
        } else if let Some(token) = bearer_token(&parts.headers) {
            let user = users
                .find_by_access_token(&token)
                .await
                .map_err(response::err)?
                .ok_or_else(|| response::err(AppError::Unauthorized("访问令牌无效".into())))?;
            require_new_api_user(&parts.headers, user.id)?;
            user
        } else {
            return Err(response::err(AppError::Unauthorized("未登录".into())));
        };

        if !user.is_enabled() {
            return Err(response::err(AppError::Unauthorized("用户已被封禁".into())));
        }

        Ok(AuthUser {
            user_id: user.id,
            username: user.username,
            role: user.role,
            group: user.group,
        })
    }
}

/// 角色闸门失败文案(与 new-api 逐字一致)。
const INSUFFICIENT_PRIVILEGES: &str = "Unauthorized, insufficient privileges";

/// 角色校验:不足时返回 HTTP 200 + `success:false`(`AppError::Forbidden` 的映射)。
fn ensure_role(user: &AuthUser, min_role: i32) -> Result<(), axum::response::Response> {
    if user.role < min_role {
        return Err(response::err(AppError::Forbidden(
            INSUFFICIENT_PRIVILEGES.into(),
        )));
    }
    Ok(())
}

/// AdminAuth(role ≥ 10)。
///
/// 契约:管理面角色不足返回 **HTTP 200 + `success:false`**(`AppError::Forbidden`
/// 的 `admin_status()` 映射为 200),不是 403。
pub struct AdminUser(pub AuthUser);

impl FromRequestParts<std::sync::Arc<ServerState>> for AdminUser {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &std::sync::Arc<ServerState>,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        ensure_role(&user, role::ADMIN)?;
        Ok(AdminUser(user))
    }
}

/// RootAuth(role ≥ 100)。失败同 [`AdminUser`],返回 200 + `success:false`。
pub struct RootUser(pub AuthUser);

impl FromRequestParts<std::sync::Arc<ServerState>> for RootUser {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &std::sync::Arc<ServerState>,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        ensure_role(&user, role::ROOT)?;
        Ok(RootUser(user))
    }
}

/// 防串号:`New-Api-User` 必须存在且等于会话/令牌归属用户 id。
fn require_new_api_user(    headers: &http::HeaderMap,
    expected: i64,
) -> Result<(), axum::response::Response> {
    let got = headers
        .get("New-Api-User")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<i64>().ok());
    if got != Some(expected) {
        return Err(response::err(AppError::Unauthorized(
            "缺少或错误的 New-Api-User 头".into(),
        )));
    }
    Ok(())
}

fn bearer_token(headers: &http::HeaderMap) -> Option<String> {
    headers
        .get(http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 从请求中按优先级提取 sk-token(CONTRACTS §1 TOKEN-MW)。
///
/// 顺序:`Authorization: Bearer sk-…` → `x-api-key`(Claude)→
/// `x-goog-api-key` / `?key=`(Gemini)→ `Sec-WebSocket-Protocol`(realtime)→
/// `mj-api-secret`(MJ)。
pub fn extract_sk_token(headers: &http::HeaderMap, query: &str) -> Option<String> {
    // 1. Authorization: Bearer sk-...
    if let Some(value) = headers
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = value.strip_prefix("Bearer ") {
            let token = token.trim();
            if token.starts_with("sk-") {
                return Some(token.to_string());
            }
        }
    }
    // 2. x-api-key(Claude 路径)
    if let Some(token) = header_token(headers, "x-api-key") {
        return Some(token);
    }
    // 3. x-goog-api-key(Gemini 路径)
    if let Some(token) = header_token(headers, "x-goog-api-key") {
        return Some(token);
    }
    // 3'. ?key=(Gemini 路径)
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("key=") {
            if !value.trim().is_empty() {
                return Some(value.trim().to_string());
            }
        }
    }
    // 4. Sec-WebSocket-Protocol(realtime):形如 `realtime, openai-insecure-api-key.sk-xxx`
    if let Some(value) = headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
    {
        for part in value.split(',') {
            let part = part.trim();
            if let Some(token) = part.strip_prefix("openai-insecure-api-key.") {
                return Some(token.to_string());
            }
        }
    }
    // 5. mj-api-secret(MJ 路径)
    if let Some(value) = header_token(headers, "mj-api-secret") {
        return Some(value.strip_prefix("Bearer ").unwrap_or(&value).trim().to_string());
    }
    None
}

fn header_token(headers: &http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// 中继面令牌鉴权提取器:sk-token → [`AuthToken`] 上下文。
///
/// 校验:令牌存在且 `status=1`、未过期(`-1` 或 > now)、非无限额度时额度 > 0、
/// 归属用户未禁用。`sk-xxx-{channelId}` 后缀指定渠道仅 admin/root 可用。
pub struct TokenAuth(pub AuthToken);

impl FromRequestParts<std::sync::Arc<ServerState>> for TokenAuth {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &std::sync::Arc<ServerState>,
    ) -> Result<Self, Self::Rejection> {
        use sea_weir_types::constants::{status, token_status};

        let reject = |msg: &str| {
            Err(response::relay_err(
                AppError::Unauthorized(msg.to_string()).into(),
                sea_weir_types::RelayFormat::OpenAi,
            ))
        };

        let query = parts.uri.query().unwrap_or("");
        let Some(raw) = extract_sk_token(&parts.headers, query) else {
            return reject("缺少 API 密钥");
        };
        // 客户端携带的 `sk-` 前缀不入库,查库前剥离。
        let raw = raw.strip_prefix("sk-").unwrap_or(&raw);
        // `-{channelId}` 后缀:指定渠道。
        let (key, specific_channel_id) = split_specific_channel(raw);

        let tokens = state
            .tokens
            .as_ref()
            .ok_or_else(|| response::err(AppError::Database("数据库未连接".into())))?;
        let token = tokens
            .find_by_key(key)
            .await
            .map_err(response::err)?
            .ok_or_else(|| AppError::Unauthorized("API 密钥无效".into()))
            .map_err(|e| {
                response::relay_err(e.into(), sea_weir_types::RelayFormat::OpenAi)
            })?;

        if token.status != token_status::ENABLED {
            return reject("API 密钥已禁用");
        }
        let now = chrono::Utc::now().timestamp();
        if token.expired_time != -1 && token.expired_time <= now {
            return reject("API 密钥已过期");
        }
        if !token.unlimited_quota && token.remain_quota <= 0 {
            return reject("API 密钥额度已用尽");
        }

        let users = state
            .users
            .as_ref()
            .ok_or_else(|| response::err(AppError::Database("数据库未连接".into())))?;
        let user = users
            .find_by_id(token.user_id)
            .await
            .map_err(response::err)?
            .ok_or_else(|| AppError::Unauthorized("用户不存在".into()))
            .map_err(|e| {
                response::relay_err(e.into(), sea_weir_types::RelayFormat::OpenAi)
            })?;
        if !user.is_enabled() {
            return reject("用户已被封禁");
        }
        if specific_channel_id.is_some() && user.role < role::ADMIN {
            return Err(response::relay_err(
                AppError::Forbidden("普通用户不支持指定渠道".into()).into(),
                sea_weir_types::RelayFormat::OpenAi,
            ));
        }

        let group = if token.group.trim().is_empty() {
            user.group.clone()
        } else {
            token.group.clone()
        };
        let _ = status::ENABLED;

        Ok(TokenAuth(AuthToken {
            token_id: token.id,
            token_name: token.name.clone(),
            user_id: token.user_id,
            username: user.username.clone(),
            group,
            model_limits: None,
            remain_quota: token.remain_quota,
            unlimited_quota: token.unlimited_quota,
            specific_channel_id,
        }))
    }
}

/// 解析 `sk-xxx-{channelId}` 后缀 → (key, Option<channelId>)。
fn split_specific_channel(raw: &str) -> (&str, Option<i64>) {
    if let Some((head, tail)) = raw.rsplit_once('-') {
        if !head.is_empty() {
            if let Ok(id) = tail.parse::<i64>() {
                return (head, Some(id));
            }
        }
    }
    (raw, None)
}

/// 角色闸门。
///
/// 契约:管理面角色不足返回 **HTTP 200 + `success:false`**(不是 403)。
pub fn require_role(_min_role: i32) -> impl Clone {
    todo!("axum layer:role < min_role → HTTP 200 + success:false")
}

/// 敏感操作凭证校验(取渠道密钥等)。**一次性消费**。
pub async fn require_secure_credential(_jti: &str) -> AppResult<()> {
    todo!("Valkey 一次性消费;失败 401")
}

#[cfg(test)]
mod tests {
    // TDD 入口(安全边界,优先级仅次于账务):
    // - [x] 缺 `New-Api-User` 头 → 401(require_new_api_user)
    // - [x] `New-Api-User` 与会话 user 不一致 → 401(防串号)
    // - [ ] extract_sk_token 五个来源的优先级顺序
    // - [ ] jti 在吊销列表 → 401(待 Valkey)
    // - [ ] 用户 status != 1 → 401「用户已被封禁」
    // - [ ] role 不足 → HTTP 200 + success:false
    // - [ ] `sk-xxx-123` 由普通用户使用 → 403「普通用户不支持指定渠道」
    // - [ ] 令牌过期(expired_time 已过且 != -1)→ 401
    // - [ ] allow_ips 非空且客户端 IP 未命中 CIDR → 403
    // - [ ] 敏感凭证第二次使用 → 401(一次性)
    // - [ ] 缓存不可用时鉴权 fail-close(不放行)
    use super::*;

    fn headers_with(value: Option<&str>) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        if let Some(v) = value {
            headers.insert("New-Api-User", v.parse().expect("合法头值"));
        }
        headers
    }

    #[test]
    fn missing_new_api_user_is_rejected() {
        assert!(require_new_api_user(&headers_with(None), 1).is_err());
    }

    #[test]
    fn mismatched_new_api_user_is_rejected() {
        assert!(require_new_api_user(&headers_with(Some("2")), 1).is_err());
    }

    #[test]
    fn matching_new_api_user_passes() {
        assert!(require_new_api_user(&headers_with(Some("1")), 1).is_ok());
    }

    fn auth_user(role_value: i32) -> AuthUser {
        AuthUser {
            user_id: 1,
            username: "u".into(),
            role: role_value,
            group: "default".into(),
        }
    }

    #[test]
    fn role_gate_rejects_insufficient_role() {
        // common(1) 不能过 admin(10) 闸门
        assert!(ensure_role(&auth_user(role::COMMON), role::ADMIN).is_err());
        // admin(10) 不能过 root(100) 闸门
        assert!(ensure_role(&auth_user(role::ADMIN), role::ROOT).is_err());
    }

    #[test]
    fn role_gate_returns_http_200_not_403() {
        // 契约:管理面角色不足是 HTTP 200 + success:false,不是 403。
        let response = ensure_role(&auth_user(role::COMMON), role::ADMIN).expect_err("应被拒绝");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    #[test]
    fn role_gate_allows_sufficient_role() {
        assert!(ensure_role(&auth_user(role::ADMIN), role::ADMIN).is_ok());
        assert!(ensure_role(&auth_user(role::ROOT), role::ROOT).is_ok());
        assert!(ensure_role(&auth_user(role::COMMON), role::COMMON).is_ok());
    }

    #[test]
    fn sk_token_authorization_beats_other_sources() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::AUTHORIZATION,
            "Bearer sk-from-header".parse().unwrap(),
        );
        headers.insert("x-api-key", "from-x-api-key".parse().unwrap());
        assert_eq!(
            extract_sk_token(&headers, "key=from-query").as_deref(),
            Some("sk-from-header")
        );
    }

    #[test]
    fn sk_token_falls_back_to_query_key() {
        let headers = http::HeaderMap::new();
        assert_eq!(
            extract_sk_token(&headers, "stream=true&key=plain-key").as_deref(),
            Some("plain-key")
        );
    }

    #[test]
    fn sk_token_specific_channel_suffix() {
        assert_eq!(split_specific_channel("abc123-42"), ("abc123", Some(42)));
        assert_eq!(split_specific_channel("abc123"), ("abc123", None));
    }
}
