//! Valkey 缓存:键约定、TTL、失效广播。
//!
//! 布局见 doc/architecture/adr/ADR-007-cache-and-rate-limit.md 技术架构表。

use sea_weir_types::AppResult;

/// 缓存键约定。集中定义,避免散落拼串。
pub mod keys {
    /// 用户缓存,TTL 60s。
    pub fn user(id: i64) -> String {
        format!("user:{id}")
    }
    /// 令牌缓存。**键用 HMAC(key),明文不落缓存**。
    pub fn token(key_hmac: &str) -> String {
        format!("token:{key_hmac}")
    }
    /// 会话吊销黑名单,TTL = 会话剩余有效期。
    pub fn session_revoked(jti: &str) -> String {
        format!("sess:revoke:{jti}")
    }
    /// 邮箱验证码,TTL 10 分钟,一次性消费。
    pub fn email_code(email: &str) -> String {
        format!("verify:{email}")
    }
    /// 敏感操作凭证,一次性消费。
    pub fn secure_credential(jti: &str) -> String {
        format!("secure:{jti}")
    }
    /// 限流窗口。
    pub fn rate_limit(dimension: &str, key: &str) -> String {
        format!("rateLimit:{dimension}:{key}")
    }
    /// 渠道亲和性。**保留 new-api 前缀以兼容存量缓存**。
    pub fn channel_affinity(suffix: &str) -> String {
        format!("new-api:channel_affinity:v1:{suffix}")
    }
}

/// 失效广播频道。多节点经 pub/sub 即时失效,60s 轮询兜底。
pub mod channels {
    pub const INVALIDATE_USER: &str = "cache:invalidate:user";
    pub const INVALIDATE_TOKEN: &str = "cache:invalidate:token";
    pub const INVALIDATE_CHANNEL: &str = "cache:invalidate:channel";
    pub const INVALIDATE_OPTION: &str = "cache:invalidate:option";
}

/// 令牌 key 的 HMAC-SHA256 摘要(小写十六进制)—— 保证明文 key 永不进入缓存。
pub fn hmac_token_key(key: &str, secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).expect("HMAC 接受任意长度密钥");
    mac.update(key.as_bytes());
    let bytes = mac.finalize().into_bytes();
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// 缓存不可用时的降级判定。
///
/// 契约(ADR-007):**鉴权类 fail-close**(拒绝请求),非关键路径 fail-open(回源 DB)。
pub fn should_fail_open(path_kind: CachePathKind) -> bool {
    match path_kind {
        // 令牌校验与会话吊销关系到安全边界,缓存不可用时必须拒绝而不是放行。
        CachePathKind::Auth => false,
        CachePathKind::Data | CachePathKind::RateLimit => true,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CachePathKind {
    /// 令牌校验、会话吊销 —— 不可 fail-open。
    Auth,
    /// 渠道索引、定价视图 —— 可回源。
    Data,
    /// 限流窗口 —— 可降级到内存。
    RateLimit,
}

/// 订阅失效广播并维护本地缓存。
pub async fn subscribe_invalidation() -> AppResult<()> {
    todo!("fred subscriber client 订阅 channels::* 并驱动本地失效")
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] keys::channel_affinity 保留 `new-api:` 前缀(存量兼容,不可改)
    // - [ ] hmac_token_key 对同一输入稳定、对不同 secret 不同
    // - [ ] should_fail_open(Auth) == false(安全语义,回归必查)
    // - [ ] 广播失效后本地 moka 索引被摘除
}
