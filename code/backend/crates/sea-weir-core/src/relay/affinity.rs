//! 渠道亲和性(粘性路由)。
//!
//! 规则(ADR-006):规则(模型/路径/UA/键源)匹配 → HybridCache(内存 LRU + Valkey)
//! 命中直连;请求成功(状态 < 400)后回写;**失败时按规则跳过重试**。
//!
//! 缓存键保留 new-api 前缀 `new-api:channel_affinity:v1:*` 以兼容存量。

use sea_weir_types::AppResult;

/// 亲和性规则命中结果。
pub struct AffinityHit {
    pub channel_id: i64,
    /// 命中该规则后若请求失败,是否跳过重试。
    pub skip_retry_on_failure: bool,
}

pub trait AffinityCache: Send + Sync {
    fn lookup(&self, key: &str) -> AppResult<Option<AffinityHit>>;
    fn record_success(&self, key: &str, channel_id: i64) -> AppResult<()>;
    fn invalidate(&self, key: &str) -> AppResult<()>;
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 缓存键前缀恰为 `new-api:channel_affinity:v1:`(存量兼容,不可改)
    // - [ ] 成功请求回写;失败请求不回写
    // - [ ] skip_retry_on_failure=true 时,失败后不进入重试循环
}
