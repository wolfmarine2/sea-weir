//! 静态面。
//!
//! K8s 部署形态下前端由 nginx 托管,本面为空;
//! `embed_frontend = true` 的单容器形态才用 rust-embed 提供 SPA 资源 + fallback。

use axum::Router;
use std::sync::Arc;

pub fn routes(_state: Arc<crate::app_state::AppState>) -> Router {
    todo!("embed_frontend 时提供静态资源与 SPA fallback,否则返回空 Router")
}
