use async_trait::async_trait;
use sea_weir_types::domain::{NewToken, Token};
use sea_weir_types::AppResult;

#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait TokenRepository: Send + Sync {
    /// 按 HMAC 摘要查找(Valkey 读穿,未命中回源 openGauss)。
    ///
    /// 前置:调用方已完成 `sk-` 前缀剥离与 `-{channelId}` 后缀解析。
    /// 不变量:**明文 key 不进入缓存**。
    async fn find_by_key(&self, key_hmac: &str) -> AppResult<Option<Token>>;

    async fn find_by_id(&self, id: i64) -> AppResult<Option<Token>>;
    async fn list_by_user(&self, user_id: i64, offset: i64, limit: i64) -> AppResult<Vec<Token>>;
    async fn count_by_user(&self, user_id: i64) -> AppResult<i64>;
    /// 全量令牌数(管理面概览用)。
    async fn count(&self) -> AppResult<i64>;
    /// 本人范围内名称是否已存在(创建前置校验)。
    async fn exists_by_name(&self, user_id: i64, name: &str) -> AppResult<bool>;

    /// 创建令牌。后置:返回新令牌 id。
    async fn create(&self, new: NewToken) -> AppResult<i64>;
    /// 全量更新(按 id + user_id 守卫,防越权)。返回是否命中。
    async fn update(&self, token: &Token) -> AppResult<bool>;
    /// 软删除(按 id + user_id 守卫,防越权)。返回是否命中。
    async fn soft_delete(&self, id: i64, user_id: i64) -> AppResult<bool>;

    /// 条件原子扣减,语义同 `UserRepository::try_decrease_quota`。
    /// `unlimited_quota` 为真时直接返回 true 且不写库。
    async fn try_decrease_quota(&self, id: i64, amount: i64) -> AppResult<bool>;
    async fn increase_quota(&self, id: i64, amount: i64) -> AppResult<()>;

    async fn update_accessed_time(&self, id: i64, ts: i64) -> AppResult<()>;
    /// 用户被禁用时批量失效其全部令牌缓存。
    async fn invalidate_cache_by_user(&self, user_id: i64) -> AppResult<()>;
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] unlimited_quota=true 时 try_decrease_quota 恒 true 且不改库
    // - [ ] find_by_key 缓存命中不打库;失效广播后回源
    // - [ ] 明文 key 不出现在任何缓存写入调用中
}
