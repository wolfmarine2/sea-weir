//! # sea-weir-core
//!
//! 业务逻辑层:`admin`(16 个管理域)+ `relay`(中继引擎)。
//!
//! **约束**:依赖 repository / adaptors / types,不依赖 server。
//! 业务逻辑不接触 HTTP 类型 —— handler 负责解析与包裹,core 只吃领域对象。
//! 这条边界是 core 层可脱离 axum 单测的前提。

#![forbid(unsafe_code)]

pub mod admin;
pub mod relay;
