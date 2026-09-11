//! 传输对象。
//!
//! 管理面(`common`/`admin`)一律 `snake_case`;中继面(`relay`)为各上游原生协议字段。

pub mod admin;
pub mod common;
pub mod relay;

pub use common::{ApiResponse, PageInfo, PageQuery};
pub use relay::{RelayInfo, RelayRequest, Usage};
