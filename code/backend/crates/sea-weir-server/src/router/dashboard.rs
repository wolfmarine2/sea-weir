//! 兼容面路由(4 条)。OpenAI dashboard 计费兼容端点。
//!
//! `/dashboard/billing/{subscription,usage}` 及其 `/v1` 前缀变体,均走 TokenAuth。

use axum::Router;
use std::sync::Arc;

pub fn routes(_state: Arc<crate::app_state::AppState>) -> Router {
    todo!("注册 4 条;两种前缀指向同一 handler")
}
