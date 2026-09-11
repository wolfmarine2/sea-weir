//! 渠道索引缓存。对应 C4 组件 `channel_cache`。
//!
//! 结构:进程内 moka `group → model → [channel](按 priority 排序)`。
//! 失效:Valkey pub/sub 即时失效 + 60s 全量同步兜底;渠道状态变更即时摘除。

use sea_weir_types::AppResult;

pub struct ChannelIndex {
    // TODO(TDD): moka::future::Cache<(String, String), Vec<CachedAbility>>
}

impl ChannelIndex {
    pub async fn rebuild(&self) -> AppResult<()> {
        todo!("从 ChannelRepository::list_enabled 全量重建")
    }

    /// 单渠道摘除(自动禁用时调用,必须即时生效)。
    pub async fn evict_channel(&self, _channel_id: i64) -> AppResult<()> {
        todo!("从所有 (group, model) 条目中摘除该渠道")
    }
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] rebuild 后按 (group, model) 能查到,且按 priority 降序
    // - [ ] evict_channel 后该渠道不再出现在任何条目
    // - [ ] 广播失效 → 本地索引更新(多节点一致性)
}
