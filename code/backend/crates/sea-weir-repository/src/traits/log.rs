use async_trait::async_trait;
use sea_weir_types::{domain::Log, AppResult};

/// 日志仓储。写入走**独立日志库连接池**。
#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait LogRepository: Send + Sync {
    async fn record(&self, log: &Log) -> AppResult<()>;
    async fn search(
        &self,
        filter: &LogFilter,
        offset: i64,
        limit: i64,
    ) -> AppResult<(Vec<Log>, i64)>;
    /// 统计:总额度 + 最近 60s rpm/tpm。
    async fn stat(&self, filter: &LogFilter) -> AppResult<LogStat>;
    /// 历史清理。
    async fn delete_before(&self, ts: i64) -> AppResult<u64>;
}

#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    pub user_id: Option<i64>,
    pub token_id: Option<i64>,
    pub channel_id: Option<i64>,
    pub log_type: Option<i32>,
    pub model_name: Option<String>,
    pub start_ts: Option<i64>,
    pub end_ts: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct LogStat {
    pub total_quota: i64,
    pub rpm: i64,
    pub tpm: i64,
}
