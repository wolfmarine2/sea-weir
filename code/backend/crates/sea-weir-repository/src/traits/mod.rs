//! Repository Trait 定义。签名与前置/后置条件见 doc/architecture/CONTRACTS.md §12。
//!
//! 全部 Trait 为 `Send + Sync`,由 `Arc<dyn ...>` 注入 core 层,
//! 使 core 的单元测试可用 mockall 替身,无需真实数据库。

pub mod ability;
pub mod channel;
pub mod log;
pub mod option;
pub mod prefill_group;
pub mod redemption;
pub mod subscription;
pub mod task;
pub mod token;
pub mod topup;
pub mod user;

pub use ability::AbilityRepository;
pub use channel::ChannelRepository;
pub use log::LogRepository;
pub use option::OptionRepository;
pub use prefill_group::PrefillGroupRepository;
pub use redemption::RedemptionRepository;
pub use subscription::SubscriptionRepository;
pub use task::TaskRepository;
pub use token::TokenRepository;
pub use topup::TopUpRepository;
pub use user::UserRepository;

// mockall::automock 生成的替身类型在各子模块内,需显式再导出,
// 供 sea-weir-core 的单元测试用(见 crate::mocks)。
#[cfg(feature = "mock")]
pub use {
    ability::MockAbilityRepository, channel::MockChannelRepository, log::MockLogRepository,
    option::MockOptionRepository, prefill_group::MockPrefillGroupRepository,
    redemption::MockRedemptionRepository,
    subscription::MockSubscriptionRepository, task::MockTaskRepository, token::MockTokenRepository,
    topup::MockTopUpRepository, user::MockUserRepository,
};
