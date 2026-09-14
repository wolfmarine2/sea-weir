//! # sea-weir-server(库目标)
//!
//! 二进制入口在 `main.rs`;本 lib 目标导出 router / middleware / response 等模块,
//! 供 `test/` crate 做路由自省与中间件单测(见 test/cases/11-route-completeness.md)。
//!
//! 装配逻辑仍在 `main.rs`,本文件只负责暴露模块。

#![forbid(unsafe_code)]

pub mod app_state;
pub mod background;
pub mod cache_listener;
pub mod config;
pub mod handlers;
pub mod middleware;
pub mod pricing;
pub mod response;
pub mod router;
pub mod session;
