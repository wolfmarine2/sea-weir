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
    pub user_id: i64,
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

/// 防串号:`New-Api-User` 必须存在且等于会话/令牌归属用户 id。
fn require_new_api_user(
    headers: &http::HeaderMap,
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

/// 从请求中按优先级提取 sk-token。
pub fn extract_sk_token(_headers: &http::HeaderMap, _query: &str) -> Option<String> {
    todo!("按 5 个来源的优先级顺序提取")
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
}
