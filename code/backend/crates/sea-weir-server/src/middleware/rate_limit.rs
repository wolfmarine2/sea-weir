//! 限流。见 ADR-007 限流维度表。
//!
//! 维度(与 new-api 对齐):
//! - IP 维度:GlobalWeb / GlobalAPI / Critical
//! - IP 或用户维度:Search / Download / Upload
//! - 用户维度:ModelRequestRateLimit(总次数令牌桶 + 成功次数滑动窗口,分组可覆盖)
//!
//! 现状:进程内滑动窗口实现(单副本足够);Valkey 滑动窗口 + 多副本一致待缓存层接入。
//! 客户端 IP 取自 `X-Real-IP` / `X-Forwarded-For`(nginx 已注入)。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;

use sea_weir_types::{NewApiError, RelayFormat};

use crate::app_state::ServerState;

#[derive(Debug, Clone, Copy)]
pub enum LimitDimension {
    GlobalWeb,
    GlobalApi,
    Critical,
    Search,
    Download,
    Upload,
    ModelRequest,
}

/// 进程内滑动窗口限流器(按 key 隔离)。
pub struct SlidingWindowLimiter {
    limit: usize,
    window: Duration,
    hits: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

impl SlidingWindowLimiter {
    pub fn new(limit: usize, window_secs: u64) -> Self {
        Self {
            limit,
            window: Duration::from_secs(window_secs.max(1)),
            hits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 记一次请求。超限返回 `Err(retry_after_secs)`。
    pub fn check(&self, key: &str) -> Result<(), u64> {
        let now = Instant::now();
        let mut map = match self.hits.lock() {
            Ok(m) => m,
            Err(poisoned) => poisoned.into_inner(),
        };
        let queue = map.entry(key.to_string()).or_default();
        // 滑出窗口。
        while let Some(front) = queue.front() {
            if now.duration_since(*front) >= self.window {
                queue.pop_front();
            } else {
                break;
            }
        }
        if queue.len() >= self.limit {
            let retry_after = queue
                .front()
                .map(|t| self.window.saturating_sub(now.duration_since(*t)).as_secs() + 1)
                .unwrap_or(1);
            return Err(retry_after.max(1));
        }
        queue.push_back(now);
        // 粗粒度清理,避免 key 无限增长。
        if map.len() > 10_000 {
            map.retain(|_, q| !q.is_empty());
        }
        Ok(())
    }
}

/// 客户端 IP:X-Real-IP → X-Forwarded-For 首跳 → "unknown"。
pub fn client_ip(req: &Request) -> String {
    let headers = req.headers();
    if let Some(ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let ip = ip.trim();
        if !ip.is_empty() {
            return ip.to_string();
        }
    }
    if let Some(xff) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(first) = xff.split(',').next().map(str::trim).filter(|s| !s.is_empty()) {
            return first.to_string();
        }
    }
    "unknown".to_string()
}

/// 中继面限流中间件:按客户端 IP 限流,超限返回 429 + Retry-After(OpenAI 错误体)。
pub async fn relay_limit(
    State(state): State<Arc<ServerState>>,
    req: Request,
    next: Next,
) -> Response {
    let ip = client_ip(&req);
    if let Err(retry_after) = state.relay_limiter.check(&ip) {
        let body = NewApiError {
            status_code: 429,
            error_code: "rate_limit_exceeded".into(),
            error_type: "new_api_error".into(),
            message: format!("请求过于频繁,请 {retry_after}s 后重试"),
            local_error: true,
            skip_retry: true,
            record_error_log: false,
        }
        .to_body(RelayFormat::OpenAi);
        let mut resp = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
        if let Ok(value) = HeaderValue::from_str(&retry_after.to_string()) {
            resp.headers_mut().insert(header::RETRY_AFTER, value);
        }
        return resp;
    }
    next.run(req).await
}

/// 保留给非中继维度(管理面/搜索等)的占位。
pub fn layer(_dimension: LimitDimension) -> impl Clone {
    todo!("按维度挂载:目前中继面用 relay_limit;管理面各维度待补")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_limit_and_recovery() {
        let limiter = SlidingWindowLimiter::new(3, 1);
        assert!(limiter.check("ip1").is_ok());
        assert!(limiter.check("ip1").is_ok());
        assert!(limiter.check("ip1").is_ok());
        // 第 4 次超限,返回 retry_after ≥ 1
        let err = limiter.check("ip1").unwrap_err();
        assert!(err >= 1);
    }

    #[test]
    fn keys_are_isolated() {
        let limiter = SlidingWindowLimiter::new(1, 60);
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("a").is_err());
        // 不同 key 互不影响
        assert!(limiter.check("b").is_ok());
    }

    #[test]
    fn client_ip_prefers_real_ip_then_xff() {
        let mut req = Request::new(axum::body::Body::empty());
        req.headers_mut()
            .insert("x-forwarded-for", "1.2.3.4, 5.6.7.8".parse().unwrap());
        assert_eq!(client_ip(&req), "1.2.3.4");
        req.headers_mut()
            .insert("x-real-ip", "9.9.9.9".parse().unwrap());
        assert_eq!(client_ip(&req), "9.9.9.9");
    }
}
