use async_trait::async_trait;
use sea_weir_types::AppResult;

#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait RedemptionRepository: Send + Sync {
    /// 兑换。
    ///
    /// 不变量:**事务 + `SELECT ... FOR UPDATE` 行锁**,
    /// 在同一事务内完成「置已用 + 用户额度增加 + 记充值日志」。
    /// 返回兑换额度;已用/禁用/过期返回 `Err`。
    /// 并发兑换由行锁 + 状态唯一性保证只成功一次。
    async fn redeem(&self, key: &str, user_id: i64) -> AppResult<i64>;
}

#[cfg(test)]
mod tests {
    // TDD 入口(并发正确性):
    // - [ ] 同一兑换码 N 个并发请求,恰好 1 个成功、N-1 个失败
    // - [ ] 成功后用户额度增量 == 兑换码面额,且写入一条 LogType::Topup
    // - [ ] 兑换过程中任一步失败 → 兑换码状态与用户额度均回滚
}
