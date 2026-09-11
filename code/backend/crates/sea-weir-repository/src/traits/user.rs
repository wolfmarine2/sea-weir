use async_trait::async_trait;
use sea_weir_types::{domain::User, AppResult};

#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: i64) -> AppResult<Option<User>>;
    async fn find_by_username(&self, username: &str) -> AppResult<Option<User>>;
    async fn find_by_access_token(&self, token: &str) -> AppResult<Option<User>>;

    /// 条件原子扣减。
    ///
    /// `UPDATE users SET quota = quota - $1 WHERE id = $2 AND quota >= $1`
    ///
    /// - 后置:返回 `Ok(true)` 表示扣减成功;`Ok(false)` 表示**影响 0 行 = 余额不足**。
    /// - 不变量:任何情况下 quota 不得为负(这正是 new-api 无条件 `quota - ?` 的缺陷所在)。
    async fn try_decrease_quota(&self, id: i64, amount: i64) -> AppResult<bool>;

    /// 原子增加(退款/充值)。无需条件守卫。
    async fn increase_quota(&self, id: i64, amount: i64) -> AppResult<()>;

    /// 统计口径累加。可批量合并,**不在账务关键路径上**(ADR-005)。
    async fn accumulate_usage(&self, id: i64, used_quota: i64, request_count: i64)
        -> AppResult<()>;

    async fn update_status(&self, id: i64, status: i32) -> AppResult<()>;
    async fn soft_delete(&self, id: i64) -> AppResult<()>;
}

#[cfg(test)]
mod tests {
    // TDD 入口(账务正确性的核心,优先级最高):
    // - [ ] try_decrease_quota:余额刚好等于扣减额 → true,余额剩 0
    // - [ ] try_decrease_quota:余额小于扣减额 → false,余额**不变**
    // - [ ] 并发 N 个扣减请求,成功数 == floor(初始余额 / 单次额度),无负值
    // - [ ] increase_quota 与 try_decrease_quota 交叉并发后总额守恒
}
