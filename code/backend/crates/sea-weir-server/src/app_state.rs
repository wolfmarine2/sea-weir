//! 全局共享状态。以 `Arc<AppState>` 注入 axum。

use std::sync::Arc;

use sea_weir_repository::{OptionRepository, TokenRepository, UserRepository};
use sea_weir_types::config::AppConfig;

use crate::session::SessionSigner;

/// 线程安全共享状态。所有字段必须 `Send + Sync`。
pub struct AppState {
    pub config: sea_weir_types::config::AppConfig,
    pub repos: Repositories,
    pub adaptors: Arc<sea_weir_adaptors::AdaptorRegistry>,
    // TODO(TDD): channel_index、affinity_cache、pricing_view 等进程内缓存
}

/// 全量 Repository 集合(待四路由面落地后启用)。以 trait object 持有,便于测试替身。
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

/// 当前已落地端点(状态 / 首装 / 登录)使用的最小状态。
///
/// 说明:骨架阶段 Repository 实现尚在逐个补齐,故 user/option 以 `Option` 持有 ——
/// 数据库不可用时为 `None`,相关端点返回 5xx 而不是让进程起不来;
/// 待全部 Repository 就绪后并入 [`AppState`]。
pub struct ServerState {
    pub config: AppConfig,
    pub sessions: SessionSigner,
    pub users: Option<Arc<dyn UserRepository>>,
    pub options: Option<Arc<dyn OptionRepository>>,
    pub tokens: Option<Arc<dyn TokenRepository>>,
    /// 进程启动时刻(unix 秒)
    pub started: u64,
}

impl ServerState {
    /// 数据库是否可用(user 与 option 仓库均已装配)。
    pub fn db_ready(&self) -> bool {
        self.users.is_some() && self.options.is_some()
    }
}

