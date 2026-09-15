use async_trait::async_trait;
use sea_weir_types::{domain::Channel, AppResult};

#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait ChannelRepository: Send + Sync {
    /// 渠道索引全量重建用。
    async fn list_enabled(&self) -> AppResult<Vec<Channel>>;
    async fn find_by_id(&self, id: i64) -> AppResult<Option<Channel>>;

    /// 管理面分页列表(不限状态)。key 由上层脱敏。
    async fn list_paged(&self, offset: i64, limit: i64) -> AppResult<Vec<Channel>>;
    async fn count(&self) -> AppResult<i64>;
    /// 渠道名是否已存在(创建前置校验)。
    async fn exists_name(&self, name: &str) -> AppResult<bool>;

    /// 中继选路:按 (group, model) 取启用中的候选渠道(join abilities,按 priority 降序)。
    async fn list_candidates(&self, group: &str, model: &str) -> AppResult<Vec<Channel>>;

    /// 某分组下可用的模型名(去重,来自启用中的 abilities)。
    async fn list_models_by_group(&self, group: &str) -> AppResult<Vec<String>>;

    /// 全部可用模型名(去重;模型广场用,不区分分组)。
    async fn list_all_models(&self) -> AppResult<Vec<String>>;

    /// 已被渠道使用(经 abilities 展开)的分组名(去重)。分组管理用于展示"使用中"的分组,
    /// 并在删除分组前做引用保护。
    async fn list_group_names(&self) -> AppResult<Vec<String>>;

    async fn create(&self, channel: &Channel) -> AppResult<i64>;
    async fn update(&self, channel: &Channel) -> AppResult<()>;
    async fn delete(&self, id: i64) -> AppResult<()>;

    /// 状态变更(含自动禁用)。`reason` 进审计日志。
    /// 后置:必须触发渠道索引失效广播。
    async fn update_status(&self, id: i64, status: i32, reason: &str) -> AppResult<()>;

    /// 多 key 渠道按 key 粒度禁用,状态写入 `channel_info` JSONB。
    async fn disable_key(&self, id: i64, key_index: usize, reason: &str) -> AppResult<()>;

    /// 同步 abilities 三元组。
    ///
    /// 不变量:**事务内先删后插**,失败整体回滚;成功后广播索引失效。
    async fn sync_abilities(&self, channel: &Channel) -> AppResult<()>;

    async fn update_balance(&self, id: i64, balance: f64, ts: i64) -> AppResult<()>;
    async fn record_test_result(&self, id: i64, response_time_ms: i64, ts: i64) -> AppResult<()>;
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] sync_abilities 中途失败 → abilities 表回到调用前状态(无半删)
    // - [ ] update_status 后 list_enabled 不再含该渠道
    // - [ ] disable_key 只影响指定下标,其余 key 仍可用
}
