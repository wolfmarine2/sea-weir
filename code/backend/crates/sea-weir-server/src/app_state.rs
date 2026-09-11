//! 全局共享状态。以 `Arc<AppState>` 注入 axum。

use std::sync::Arc;

/// 线程安全共享状态。所有字段必须 `Send + Sync`。
pub struct AppState {
    pub config: sea_weir_types::config::AppConfig,
    pub repos: Repositories,
    pub adaptors: Arc<sea_weir_adaptors::AdaptorRegistry>,
    // TODO(TDD): channel_index、affinity_cache、pricing_view 等进程内缓存
}

/// 全部 Repository 的集合。以 trait object 持有,便于测试替身。
pub struct Repositories {
    pub user: Arc<dyn sea_weir_repository::UserRepository>,
    pub token: Arc<dyn sea_weir_repository::TokenRepository>,
    pub channel: Arc<dyn sea_weir_repository::ChannelRepository>,
    pub ability: Arc<dyn sea_weir_repository::AbilityRepository>,
    pub log: Arc<dyn sea_weir_repository::LogRepository>,
    pub option: Arc<dyn sea_weir_repository::OptionRepository>,
    pub redemption: Arc<dyn sea_weir_repository::RedemptionRepository>,
    pub topup: Arc<dyn sea_weir_repository::TopUpRepository>,
    pub subscription: Arc<dyn sea_weir_repository::SubscriptionRepository>,
    pub task: Arc<dyn sea_weir_repository::TaskRepository>,
}
