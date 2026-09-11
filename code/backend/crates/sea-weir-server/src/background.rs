//! 后台任务。进程启动时 spawn,随优雅关闭一并取消。
//!
//! 清单(CONTRACTS.md §14):
//! - Option 同步(60s 轮询兜底 + pub/sub 即时失效)
//! - 渠道索引同步
//! - 渠道定时测试(通过且开启自动恢复 → 启用渠道)
//! - 异步任务轮询(15s)
//! - 订阅周期重置
//! - **崩溃预扣对账**(启动时 + 每小时)

use sea_weir_types::AppResult;
use std::sync::Arc;

pub fn spawn_all(_state: Arc<crate::app_state::AppState>) -> AppResult<()> {
    todo!("spawn 全部后台任务,持有 CancellationToken 以便优雅关闭")
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 崩溃对账:构造悬挂预扣记录 → 启动对账 → 已退款
    // - [ ] 优雅关闭:收到信号后各任务在超时内退出
    // - [ ] pub/sub 断线重连后仍能收到失效广播
}
