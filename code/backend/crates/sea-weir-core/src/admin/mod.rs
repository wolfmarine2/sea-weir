//! 管理面业务逻辑。16 个功能域,与 doc/system-design.md §1.3 一一对应。
//!
//! **边界**:本层不接触 HTTP 类型。handler 负责解析请求与包裹响应,
//! admin 层只吃领域对象、只返回 `AppResult<T>`。

pub mod channel;
pub mod dashboard;
pub mod group;
pub mod log;
pub mod model;
pub mod oauth_provider;
pub mod option;
pub mod performance;
pub mod pricing;
pub mod security;
pub mod subscription;
pub mod system;
pub mod task;
pub mod token;
pub mod topup;
pub mod user;
