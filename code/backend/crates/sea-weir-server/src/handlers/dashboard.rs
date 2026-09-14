//! `dashboard`/`data` 域 handler:管理面概览(AdminAuth)。
//!
//! 已落地:`GET /api/data/` —— 用户/渠道/令牌数量 + 日志统计(总消费、最近 60s rpm·tpm)。
//! 待补:分时段趋势、按模型/渠道的用量分布。

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;

use sea_weir_repository::traits::log::LogFilter;
use sea_weir_types::AppError;

use crate::app_state::ServerState;
use crate::middleware::auth::AdminUser;
use crate::response;

/// `GET /api/data/`(AdminAuth):概览统计。
pub async fn overview(State(state): State<Arc<ServerState>>, _auth: AdminUser) -> Response {
    let user_count = match state.users.as_ref() {
        Some(repo) => repo.count().await.unwrap_or(0),
        None => 0,
    };
    let channel_count = match state.channels.as_ref() {
        Some(repo) => repo.count().await.unwrap_or(0),
        None => 0,
    };
    let token_count = match state.tokens.as_ref() {
        Some(repo) => repo.count().await.unwrap_or(0),
        None => 0,
    };

    let (total_quota, rpm, tpm) = match state.logs.as_ref() {
        Some(logs) => match logs.stat(&LogFilter::default()).await {
            Ok(s) => (s.total_quota, s.rpm, s.tpm),
            Err(e) => return response::err(e),
        },
        None => return response::err(AppError::Database("数据库未连接".into())),
    };

    response::ok(serde_json::json!({
        "user_count": user_count,
        "channel_count": channel_count,
        "token_count": token_count,
        "total_quota": total_quota,
        "rpm": rpm,
        "tpm": tpm,
    }))
}
