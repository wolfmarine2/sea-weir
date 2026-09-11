//! 统一错误类型与四级分类。
//!
//! 设计见 doc/architecture/adr/ADR-008-error-classification-and-exit.md、
//! doc/system-design.md §10。

use std::fmt;

pub type AppResult<T> = Result<T, AppError>;

/// 错误的恢复语义分类,决定调用方「能否重试 / 是否告警 / 是否自动恢复」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// 瞬时故障,原地重试可能成功(数据库连接抖动、上游 5xx)。
    Transient,
    /// 确定性失败,重试无意义(参数非法、鉴权失败)。
    Permanent,
    /// 用户可自行恢复(额度不足→充值、限流→退避)。
    Recoverable,
    /// 不可恢复,需人工介入(数据不一致、配置缺失)。
    Unrecoverable,
}

/// 全局错误枚举。管理面与中继面共用;中继面出口前转 [`crate::NewApiError`]。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(String),

    #[error("缓存错误: {0}")]
    Cache(String),

    #[error("{0}")]
    Biz(String),

    #[error("未认证: {0}")]
    Unauthorized(String),

    #[error("无权限: {0}")]
    Forbidden(String),

    #[error("请求过于频繁")]
    RateLimited { retry_after_secs: u64 },

    #[error("额度不足")]
    QuotaExceeded,

    #[error("参数错误: {0}")]
    BadRequest(String),

    #[error("资源不存在: {0}")]
    NotFound(String),

    #[error("上游错误: {0}")]
    Upstream(String),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("内部错误: {0}")]
    Internal(String),
}

impl AppError {
    /// 恢复语义分类。
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::Database(_) | Self::Cache(_) | Self::Upstream(_) => ErrorClass::Transient,
            Self::Biz(_)
            | Self::Unauthorized(_)
            | Self::Forbidden(_)
            | Self::BadRequest(_)
            | Self::NotFound(_) => ErrorClass::Permanent,
            Self::RateLimited { .. } | Self::QuotaExceeded => ErrorClass::Recoverable,
            Self::Config(_) | Self::Internal(_) => ErrorClass::Unrecoverable,
        }
    }

    /// 管理面 HTTP 状态码。
    ///
    /// 注意契约:管理面**业务错误一律 HTTP 200** + `{success:false}`,
    /// 只有鉴权/限流类走真实状态码(CONTRACTS.md §统一响应包裹)。
    pub fn admin_status(&self) -> http::StatusCode {
        match self {
            // 未认证与限流使用真实状态码(ADR-008 出口表)。
            Self::Unauthorized(_) => http::StatusCode::UNAUTHORIZED,
            Self::RateLimited { .. } => http::StatusCode::TOO_MANY_REQUESTS,
            // 基础设施类不是业务错误,按 5xx 上报。
            Self::Database(_) | Self::Cache(_) | Self::Config(_) | Self::Internal(_) => {
                http::StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::Upstream(_) => http::StatusCode::BAD_GATEWAY,
            // 其余业务错误一律 HTTP 200 + success:false(含 Forbidden)。
            Self::Biz(_)
            | Self::Forbidden(_)
            | Self::QuotaExceeded
            | Self::BadRequest(_)
            | Self::NotFound(_) => http::StatusCode::OK,
        }
    }
}

/// 出口前敏感信息打码。
///
/// 覆盖三类最常见的泄漏源(ADR-008「出口前 MaskSensitiveInfo」):
/// 1. `sk-` 开头的渠道/用户密钥 → `sk-****`
/// 2. `Bearer <token>` → `Bearer ****`
/// 3. 邮箱 → 保留域名,本地部分打码 `***@domain`
///
/// 纯扫描实现,不引入正则依赖。
pub fn mask_sensitive(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0usize;
    while i < raw.len() {
        // 1. sk- 密钥
        if raw[i..].starts_with("sk-") {
            let mut j = i + 3;
            while j < raw.len() {
                let c = bytes[j] as char;
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    j += 1;
                } else {
                    break;
                }
            }
            out.push_str("sk-****");
            i = j;
            continue;
        }
        // 2. Bearer <token>
        if raw[i..].starts_with("Bearer ") || raw[i..].starts_with("bearer ") {
            let prefix = &raw[i..i + 7];
            let mut j = i + 7;
            while j < raw.len() {
                let c = bytes[j] as char;
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    j += 1;
                } else {
                    break;
                }
            }
            if j > i + 7 {
                out.push_str(prefix);
                out.push_str("****");
                i = j;
                continue;
            }
        }
        // 3. 邮箱:向后看 @,向前回收本地部分
        if bytes[i] == b'@' {
            let local_start = out
                .char_indices()
                .rev()
                .take_while(|(_, c)| {
                    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-')
                })
                .last()
                .map(|(idx, _)| idx);
            if let Some(start) = local_start {
                out.truncate(start);
                out.push_str("***");
                out.push('@');
                i += 1;
                continue;
            }
        }
        // 原样输出当前字符(按 UTF-8 边界推进)
        let ch = raw[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

impl fmt::Display for ErrorClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Transient => "transient",
            Self::Permanent => "permanent",
            Self::Recoverable => "recoverable",
            Self::Unrecoverable => "unrecoverable",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 每个 AppError 变体的 class() 落在预期分类
    // - [ ] admin_status():Biz → 200、Unauthorized → 401、Forbidden → 403、RateLimited → 429
    // - [ ] mask_sensitive() 对 sk-xxx / 邮箱 / 长密钥的打码形状
    // - [ ] Display 文案不泄漏原始密钥
}
