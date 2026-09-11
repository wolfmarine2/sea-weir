//! 四个路由面。对应 C4 组件 `router`。
//!
//! | 面 | 前缀 | 中间件链 |
//! |---|---|---|
//! | 管理面 | `/api` | CORS → gzip → BodyCleanup → GlobalAPIRateLimit → auth → 角色闸门 |
//! | 中继面 | `/v1`、`/v1beta`、`/mj`、`/suno`、`/kling`、`/jimeng`、`/pg` | CORS → 解压 → 统计 → 性能护栏 → TokenAuth → ModelRequestRateLimit → Distribute |
//! | 兼容面 | `/dashboard` | TokenAuth |
//! | 静态面 | `/` | SPA fallback(仅 embed_frontend 形态) |
//!
//! 完整 305 条端点清单见 doc/architecture/CONTRACTS.md 附录 A。

pub mod api;
pub mod dashboard;
pub mod relay;
pub mod web;

use axum::Router;
use std::sync::Arc;

pub mod manifest;

pub use manifest::{registered_route_count, route_manifest, RouteEntry, RouteManifest};

pub fn build(state: Arc<crate::app_state::AppState>) -> Router {
    Router::new()
        .merge(api::routes(state.clone()))
        .merge(relay::routes(state.clone()))
        .merge(dashboard::routes(state.clone()))
        .merge(web::routes(state))
}
