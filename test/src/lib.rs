//! # sea-weir-tests
//!
//! 测试代码。分层与策略见 `doc/test-design.md`,黄金用例见
//! `doc/architecture/TEST-VECTORS.md`,用例背景见 `test/cases/`。
//!
//! ## 当前状态:TDD 红灯
//!
//! `code/backend` 骨架阶段实现全部为 `todo!()`,因此本目录测试**预期全部失败**
//! (`panic: not yet implemented`)。这是 TDD 的红灯阶段,不是缺陷。
//! 实现补齐一个模块,对应测试即转绿。进度见 `test/README.md`。
//!
//! ## 命名
//!
//! 测试函数名 = 用例编号小写下划线 + 语义后缀,例如
//! `tc_uni_bil_002_pos_claude_semantic_split_cache`
//! 对应 `cases/02-unit-billing.md` 的 `TC-UNI-BIL-002-POS`。

pub mod common;

// --- L1 单元层(零外部依赖)---
pub mod unit_constants;
pub mod unit_billing;
pub mod unit_select;
pub mod unit_retry;
pub mod unit_pagination;
pub mod unit_mask;
pub mod unit_error_exit;
pub mod unit_adaptor_common;
pub mod unit_convert;
pub mod unit_stream;
/// 需 `sea-weir-server`(经 webauthn-rs 依赖 openssl),用 feature 隔离。
#[cfg(feature = "server-tests")]
pub mod unit_route_completeness;

// --- L2 集成层(需 openGauss + Valkey)---
pub mod integration_repository;
pub mod integration_idempotency;
#[cfg(feature = "server-tests")]
pub mod integration_auth;
pub mod integration_cache;
pub mod integration_pipeline;
pub mod integration_task_polling;

// --- L3 契约层(录制 fixture)---
pub mod contract_admin_api;
pub mod contract_relay_api;

// --- L4 端到端(需已部署实例)---
pub mod live_smoke;

// --- 性能基线 ---
pub mod perf;
