//! L2 集成 — 异步任务轮询与结算。用例文档:`cases/08-integration-pipeline.md`
//!
//! 溯源:SEQ-006

use crate::require_dep;
use pretty_assertions::assert_eq;

/// TC-INT-TSK-001-POS:任务提交全额预扣(无信任旁路)。
///
/// 与同步中继不同:任务类无法预估 token,必须先全额扣。
#[tokio::test]
async fn tc_int_tsk_001_pos_full_pre_consume_on_submit() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 断言提交时按估价全额预扣,不走信任旁路
}

/// TC-INT-TSK-002-POS:remix 锁定原任务渠道。
///
/// 变换类任务必须在生成原图的同一渠道执行,换渠道会失败。
#[tokio::test]
async fn tc_int_tsk_002_pos_remix_pins_origin_channel() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 提交 remix 时 origin_task_id 对应的 channel_id 被复用
}

/// TC-INT-TSK-003-POS:完成时按 adjust_billing_on_complete 补差。
#[tokio::test]
async fn tc_int_tsk_003_pos_settle_with_adjustment() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 适配器返回 Some(实际额度) → 按该值补差
}

/// TC-INT-TSK-004-POS:适配器返回 None 时按 tokens 重算。
#[tokio::test]
async fn tc_int_tsk_004_pos_settle_by_tokens_when_none() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): adjust_billing_on_complete 返回 None → 走 calculate_quota
}

/// TC-INT-TSK-005-POS:任务失败退款。
#[tokio::test]
async fn tc_int_tsk_005_pos_refund_on_failure() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 上游返回 FAILURE → CAS 置终态 → 全额退款
}

/// TC-INT-TSK-006-POS:超时任务被清扫并退款。
///
/// 上游可能永远不返回终态,没有超时清扫这笔预扣就永久悬挂。
#[tokio::test]
async fn tc_int_tsk_006_pos_timeout_sweep_refunds() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 提交时间超过阈值且仍非终态 → 置失败 + 退款
}

/// TC-INT-TSK-007-CNC:多节点并发轮询,退款恰好一次。★
///
/// 与 TC-INT-IDM-007 呼应,但从轮询循环的角度端到端验证。
#[tokio::test]
async fn tc_int_tsk_007_cnc_concurrent_polling_refunds_once() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现):
    //   同一失败任务,两个 poll_loop 并发处理
    //   断言用户额度只增加一次退款金额
    let refund_count = 0;
    assert_eq!(refund_count, 1, "多节点并发轮询时退款必须恰好一次");
}

/// TC-INT-TSK-008-POS:轮询周期为 15s。
#[test]
fn tc_int_tsk_008_pos_poll_interval() {
    use sea_weir_types::constants::defaults::TASK_POLL_INTERVAL_SECS;
    assert_eq!(TASK_POLL_INTERVAL_SECS, 15, "与 new-api service/task_polling.go:93 一致");
}
