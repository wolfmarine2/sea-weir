//! HTTP 编排层。对应 C4 组件 `admin_handlers`(16 域)与 `relay_entry`。
//!
//! **职责边界**:参数校验 → 调用 core 层 → 统一响应包裹。
//! handler 里不写业务逻辑 —— 业务在 `sea_weir_core::admin`,
//! 这样业务逻辑的测试不需要构造 HTTP 请求。

pub mod channel;
pub mod dashboard;
pub mod group;
pub mod prefill_group;
pub mod log;
pub mod model;
pub mod oauth_provider;
pub mod option;
pub mod performance;
pub mod pricing;
pub mod relay;
pub mod security;
pub mod subscription;
pub mod system;
pub mod task;
pub mod token;
pub mod topup;
pub mod user;
