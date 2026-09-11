//! # sea-weir-types
//!
//! 基础类型层:领域模型、REST/中继 DTO、`AppError`/`NewApiError`、常量、分层配置。
//!
//! **约束**(doc/architecture/module-dependency.puml):本 crate 不依赖任何内部 crate。
//!
//! ## 字段命名铁律
//! 管理面所有 DTO 一律 `snake_case`,与 new-api 的 Go struct tag 逐字段一致。
//! 见 doc/system-design.md §6.1、doc/architecture/CONTRACTS.md §通用约定。
//! 任何 camelCase 字段都会破坏「客户端与前端可灰度替换」的兼容目标。

#![forbid(unsafe_code)]

pub mod config;
pub mod constants;
pub mod domain;
pub mod dto;
pub mod error;
pub mod relay_error;

pub use error::{AppError, AppResult, ErrorClass};
pub use relay_error::{NewApiError, RelayFormat};
