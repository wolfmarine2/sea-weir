//! `log` 域 handler:消费日志查询(CONTRACTS §5)。
//!
//! 已落地:本人日志(`/api/log/self`)、全量日志(AdminAuth,`/api/log/`)。
//! 待补:统计端点 `/api/log/stat`、令牌日志 `/api/log/token`、管理操作日志。

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;

use sea_weir_repository::traits::log::LogFilter;
use sea_weir_types::AppError;

use crate::app_state::ServerState;
use crate::middleware::auth::{AdminUser, AuthUser};
use crate::response;

#[derive(Debug, Deserialize)]
pub struct LogQuery {
    #[serde(default)]
    pub p: Option<i64>,
    #[serde(default, alias = "ps", alias = "size")]
    pub page_size: Option<i64>,
    #[serde(rename = "type", default)]
    pub log_type: Option<i32>,
    #[serde(default)]
    pub model_name: Option<String>,
    #[serde(default)]
    pub start_timestamp: Option<i64>,
    #[serde(default)]
    pub end_timestamp: Option<i64>,
}

impl LogQuery {
    fn paging(&self) -> (i64, i64, i64) {
        let page = match self.p.unwrap_or(1) {
            v if v < 1 => 1,
            v => v,
        };
        let page_size = match self.page_size.unwrap_or(10) {
            v if v <= 0 => 10,
            v => v.min(100),
        };
        (page, page_size, (page - 1) * page_size)
    }

    fn filter(&self, user_id: Option<i64>) -> LogFilter {
        LogFilter {
            user_id,
            log_type: self.log_type,
            model_name: self.model_name.clone(),
            start_ts: self.start_timestamp,
            end_ts: self.end_timestamp,
            ..Default::default()
        }
    }
}

async fn query_logs(state: &ServerState, filter: LogFilter, q: &LogQuery) -> Response {
    let Some(logs) = state.logs.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    let (page, page_size, offset) = q.paging();
    match logs.search(&filter, offset, page_size).await {
        Ok((items, total)) => response::ok(serde_json::json!({
            "items": items,
            "total": total,
            "page": page,
            "page_size": page_size,
        })),
        Err(e) => response::err(e),
    }
}

/// `GET /api/log/self`(UserAuth):本人消费日志分页。
pub async fn self_logs(
    State(state): State<Arc<ServerState>>,
    auth: AuthUser,
    Query(q): Query<LogQuery>,
) -> Response {
    let filter = q.filter(Some(auth.user_id));
    query_logs(&state, filter, &q).await
}

/// `GET /api/log/`、`GET /api/log/search`(AdminAuth):全量日志分页。
pub async fn all_logs(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Query(q): Query<LogQuery>,
) -> Response {
    let filter = q.filter(None);
    query_logs(&state, filter, &q).await
}
