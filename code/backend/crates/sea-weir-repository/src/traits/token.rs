use async_trait::async_trait;
use sea_weir_types::{domain::Token, AppResult};

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
