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

use sea_weir_types::AppResult;

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

/// 从请求中按优先级提取 sk-token。
pub fn extract_sk_token(_headers: &http::HeaderMap, _query: &str) -> Option<String> {
    todo!("按 5 个来源的优先级顺序提取")
}

/// 角色闸门。
pub fn require_role(_min_role: i32) -> impl Clone {
    todo!("axum layer:role < min_role → 403")
}

/// 敏感操作凭证校验(取渠道密钥等)。**一次性消费**。
pub async fn require_secure_credential(_jti: &str) -> AppResult<()> {
    todo!("Valkey 一次性消费;失败 401")
}

#[cfg(test)]
mod tests {
    // TDD 入口(安全边界,优先级仅次于账务):
    // - [ ] extract_sk_token 五个来源的优先级顺序
    // - [ ] `New-Api-User` 与会话 user 不一致 → 401(防串号)
    // - [ ] 缺 `New-Api-User` 头 → 401
    // - [ ] jti 在吊销列表 → 401
    // - [ ] 用户 status != 1 → 401「用户已被封禁」
    // - [ ] role 不足 → 403
    // - [ ] `sk-xxx-123` 由普通用户使用 → 403「普通用户不支持指定渠道」
    // - [ ] 令牌过期(expired_time 已过且 != -1)→ 401
    // - [ ] allow_ips 非空且客户端 IP 未命中 CIDR → 403
    // - [ ] 敏感凭证第二次使用 → 401(一次性)
    // - [ ] 缓存不可用时鉴权 fail-close(不放行)
}
