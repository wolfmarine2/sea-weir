use async_trait::async_trait;
use sea_weir_types::AppResult;

#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait TopUpRepository: Send + Sync {
    /// 创建待支付订单。`trade_no` 唯一约束是幂等兜底。
    async fn create_pending(
        &self,
        user_id: i64,
        amount: i64,
        money: rust_decimal::Decimal,
        trade_no: &str,
        provider: &str,
    ) -> AppResult<i64>;

    /// 支付完成。
    ///
    /// 不变量:状态机 `pending → success` 用 **CAS** 完成
    /// (`UPDATE ... SET status='success' WHERE trade_no=$1 AND status='pending'`),
    /// 影响 0 行说明已处理过 → **重复回调直接返回成功**(幂等)。
    /// 同事务内完成用户额度入账与 TopupGroupRatio 赠送。
    async fn complete(&self, trade_no: &str) -> AppResult<bool>;

    async fn find_by_trade_no(&self, trade_no: &str) -> AppResult<Option<TopUpOrder>>;
}

#[derive(Debug, Clone)]
pub struct TopUpOrder {
    pub id: i64,
    pub user_id: i64,
    pub amount: i64,
    pub money: rust_decimal::Decimal,
    pub trade_no: String,
    pub status: String,
}

#[cfg(test)]
mod tests {
    // TDD 入口(支付幂等):
    // - [ ] 同一 trade_no 重复回调:第 2 次返回 Ok(false),用户额度只加一次
    // - [ ] 并发回调只入账一次
}
