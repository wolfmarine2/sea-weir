//! 异步任务轮询与结算。对应 C4 组件 `task_polling`。见 SEQ-006。
//!
//! 后台 15s 循环:
//! 1. 超时清扫(CAS 置失败 + 退款)
//! 2. 按平台分组回源 `TaskAdaptor::fetch_task`
//! 3. 终态时按 `adjust_billing_on_complete` 补差,或按 tokens 重算
//! 4. 失败 → `RefundTaskQuota`
//!
//! Gemini / Vertex 的任务查询走实时回源,不入轮询队列。

use sea_weir_types::AppResult;

/// 轮询循环。进程启动时 spawn 一个。
pub async fn poll_loop() -> AppResult<()> {
    todo!("15s 周期;超时清扫 → 分组回源 → 终态 CAS → 补差/退款")
}

/// 单个任务的终态处理。
pub async fn settle_task(_task_id: &str) -> AppResult<()> {
    todo!("CAS 迁移终态;CAS 失败说明已被其他节点处理,直接返回")
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] CAS 失败(其他节点抢先)→ 不重复退款
    // - [ ] 超时任务被置失败并退款
    // - [ ] 完成时 adjust_billing_on_complete 返回 Some → 按该值补差
    // - [ ] 返回 None → 按 usage tokens 重算
    // - [ ] 多节点并发轮询同一任务,退款恰好一次
}
