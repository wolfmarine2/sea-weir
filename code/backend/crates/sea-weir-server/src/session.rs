//! 会话签发与校验:签名 Cookie(自承载会话,ADR-004)。
//!
//! 现状:JWT 签名 + 过期校验已实现。jti 吊销列表依赖 Valkey(客户端未接入),
//! 因此登出仅清 Cookie,跨节点即时吊销待 cache 层实现后补(见 cache.rs TODO)。

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sea_weir_types::config::SessionConfig;
use sea_weir_types::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// 会话 Cookie 名。
pub const SESSION_COOKIE: &str = "sea_weir_session";

/// 自承载会话声明。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionClaims {
    /// 用户 id
    pub sub: i64,
    pub username: String,
    /// 0 guest / 1 common / 10 admin / 100 root
    pub role: i32,
    /// 会话唯一 id(接入 Valkey 后作为吊销黑名单键)
    pub jti: String,
    /// 过期时间(unix 秒)
    pub exp: i64,
}

pub struct SessionSigner {
    ttl_days: i64,
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl SessionSigner {
    pub fn new(cfg: &SessionConfig) -> Self {
        let secret = cfg.secret.as_bytes();
        Self {
            ttl_days: cfg.ttl_days.max(1),
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }

    pub fn ttl_secs(&self) -> i64 {
        self.ttl_days * 86_400
    }

    pub fn issue(&self, user_id: i64, username: &str, role: i32) -> AppResult<String> {
        let claims = SessionClaims {
            sub: user_id,
            username: username.to_string(),
            role,
            jti: uuid::Uuid::new_v4().to_string(),
            exp: chrono::Utc::now().timestamp() + self.ttl_secs(),
        };
        encode(&Header::default(), &claims, &self.encoding)
            .map_err(|e| AppError::Internal(format!("会话签发失败: {e}")))
    }

    pub fn verify(&self, token: &str) -> AppResult<SessionClaims> {
        decode::<SessionClaims>(token, &self.decoding, &Validation::default())
            .map(|data| data.claims)
            .map_err(|_| AppError::Unauthorized("会话无效或已过期".into()))
    }
}

/// 会话吊销黑名单键(ADR-004:登出/改密/禁用 → 写 jti,TTL=剩余有效期)。
pub fn revoke_key(jti: &str) -> String {
    format!("sess:revoke:{jti}")
}

/// 构造 Set-Cookie 值。HttpOnly + SameSite=Strict(契约固定)。
pub fn session_cookie(token: &str, max_age_secs: i64) -> String {
    format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age_secs}")
}

/// 清会话 Cookie。
pub fn clear_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0")
}

/// 从 `Cookie` 头解析指定名的值(不引入 cookie 库)。
pub fn parse_cookie(headers: &http::HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(http::header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|part| {
        let (k, v) = part.split_once('=')?;
        (k.trim() == name).then(|| v.trim().to_string())
    })
}
