use async_trait::async_trait;
use sea_weir_types::AppResult;

#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait SubscriptionRepository: Send + Sync {
    /// 幂等预扣。
    ///
    /// 不变量:
    /// - `request_id` 唯一约束兜底 —— 重复调用返回首次结果,不重复扣减;
    /// - 事务内按 `end_time` **升序** `FOR UPDATE` 逐个扣(先到期先用);
    /// - 返回实际扣减额度。
    async fn pre_consume(&self, request_id: &str, user_id: i64, amount: i64) -> AppResult<i64>;

    /// 结算:按实际用量补扣或退还差额。幂等。
    async fn settle(&self, request_id: &str, actual: i64) -> AppResult<()>;

    /// 失败退款。幂等;已结算的不退。
    async fn refund(&self, request_id: &str) -> AppResult<()>;

    /// 崩溃对账:找出预扣后既未结算也未退款的悬挂记录。
    async fn find_dangling(&self, before_ts: i64) -> AppResult<Vec<String>>;

    /// 周期额度重置。
    async fn reset_expired_periods(&self, now: i64) -> AppResult<u64>;
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 同 request_id 重复 pre_consume 只扣一次
    // - [ ] 多个订阅时按 end_time 升序消耗
    // - [ ] settle 后再 refund 无效(不退款)
    // - [ ] find_dangling 能捞出「预扣后进程崩溃」的记录
}
