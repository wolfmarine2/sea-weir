//! 管理面路由 `/api`(236 条)。清单见 CONTRACTS.md 附录 A.1。
//!
//! 分域挂载,每域的角色闸门在 group 层统一 `layer`,避免逐条遗漏:
//! - UserAuth:`/api/user/self*`、`/api/token`、`/api/subscription`
//! - AdminAuth:`/api/channel`、`/api/redemption`、`/api/models`、`/api/vendors`、
//!   `/api/deployments`、`/api/group`、`/api/prefill_group`、`/api/subscription/admin`
//! - RootAuth:`/api/option`、`/api/custom-oauth-provider`、`/api/performance`、`/api/ratio_sync`
//! - 公开:`/api/setup`、`/api/status`、`/api/notice`、OAuth、支付 webhook

use axum::Router;
use std::sync::Arc;

pub fn routes(_state: Arc<crate::app_state::AppState>) -> Router {
    todo!("按附录 A.1 逐域注册;角色闸门在 group 层统一挂载")
}

#[cfg(test)]
mod tests {
    // TDD 入口(路由注册的完整性可以自动化验证):
    // - [ ] 注册的路由数 == 236(与附录 A 对表,防漏防重)
    // - [ ] 每条受保护路由都挂了对应的角色闸门(遍历断言,防止漏挂)
    // - [ ] 公开路由不挂 auth 层
    // - [ ] `/api/channel/:id/key` 同时挂了 RootAuth 与 SecureVerificationRequired
}
