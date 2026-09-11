//! 中继面路由(54 条)。清单见 CONTRACTS.md 附录 A.2 / A.3。
//!
//! 注意两处易漏的兼容点:
//! 1. **MJ 双前缀**:同一组 16 条须同时注册在 `/mj/**` 与 `/:mode/mj/**`;
//! 2. **11 条占位端点**(files / fine-tunes / images.variations / DELETE models)
//!    须保留路由并返回与 new-api `RelayNotImplemented` 同构的错误体。

use axum::Router;
use std::sync::Arc;

pub fn routes(_state: Arc<crate::app_state::AppState>) -> Router {
    todo!("按附录 A.2/A.3 注册;MJ 组用同一个函数注册两次")
}

/// 占位端点统一处理器。
pub async fn not_implemented() -> axum::response::Response {
    todo!("返回与 new-api RelayNotImplemented 同构的错误体")
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 注册的路由数 == 54
    // - [ ] `/mj/submit/imagine` 与 `/x/mj/submit/imagine` 都能路由到同一 handler
    // - [ ] 11 条占位端点返回 501/相同错误体,且**不消耗额度**
    // - [ ] `/pg/chat/completions` 走 UserAuth 而非 TokenAuth
    // - [ ] `/v1/models` 系列不经过 Distribute(不选渠道)
}
