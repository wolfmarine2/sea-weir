//! Repository Trait 的 mock 导出(feature = "mock")。
//!
//! 供 `sea-weir-core` 的单元测试使用 —— core 层不应为了测业务逻辑而启动数据库。
//! 各 `MockXxxRepository` 由 `traits/` 中的 `#[mockall::automock]` 自动生成。

pub use crate::traits::{
    MockAbilityRepository, MockChannelRepository, MockLogRepository, MockOptionRepository,
    MockRedemptionRepository, MockSubscriptionRepository, MockTaskRepository, MockTokenRepository,
    MockTopUpRepository, MockUserRepository,
};
