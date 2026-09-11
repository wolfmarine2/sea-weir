//! 中继管线编排。把 select / billing / adaptor / stream / autoban 串成一次请求。
//!
//! 对应 `relay_entry` 的业务侧(HTTP 侧在 sea-weir-server 的 handlers/relay.rs)。
//!
//! ```text
//! for attempt in 0..=retry_times {
//!     selection = select(group, model, attempt)
//!     session   = billing.pre_consume(estimated)
//!     result    = adaptor.do_request(...) → do_response(...)
//!     match result {
//!         Ok(usage)  => { session.settle(actual); log(); return }
//!         Err(err)   => {
//!             session.refund();
//!             if should_disable(err) { autoban(...) }
//!             if !should_retry(err)  { return Err(err) }
//!         }
//!     }
//! }
//! ```

use sea_weir_types::{dto::RelayInfo, NewApiError};

pub struct RelayOutcome {
    pub usage: sea_weir_types::dto::Usage,
    pub quota: i64,
    pub channel_id: i64,
    pub attempts: u32,
}

/// 执行一次完整中继(含重试循环)。
pub async fn execute(_info: &mut RelayInfo) -> Result<RelayOutcome, NewApiError> {
    todo!("按上方伪码编排;每次重试重新选渠并重新预扣")
}

#[cfg(test)]
mod tests {
    // TDD 入口(用 mock Repository + wiremock 上游,不需要真实依赖):
    // - [ ] 首次成功:预扣一次、结算一次、无退款
    // - [ ] 首次失败可重试 → 第二次成功:退款一次、预扣两次、结算一次,账目守恒
    // - [ ] 重试耗尽:每次都退款,最终返回最后一个错误
    // - [ ] 不可重试错误:立即返回,不进入第二次
    // - [ ] 无可用渠道:返回 model_not_found(404),不产生任何扣减
    // - [ ] 预扣失败(余额不足):返回 insufficient_quota,不调用上游
    // - [ ] 每次重试的 RelayInfo.retry_count 正确递增(影响选路档位)
}
