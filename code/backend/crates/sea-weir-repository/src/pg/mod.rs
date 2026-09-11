//! Repository Trait 的 sqlx / openGauss 实现。
//!
//! 骨架阶段仅占位。TDD 阶段每个实现的落地顺序建议:
//!   1. 先写 Trait 层的集成测试(testcontainers 起 openGauss)
//!   2. 再补 SQL —— 全部走 `sqlx::query!` 宏,享受编译期校验
//!   3. 保留字 `"group"`、`"key"` 统一双引号引用
//!
//! 每个文件对应 traits/ 下的同名 Trait。

mod ability;
mod channel;
mod log;
mod option;
mod redemption;
mod subscription;
mod task;
mod token;
mod topup;
mod user;

pub use ability::PgAbilityRepository;
pub use channel::PgChannelRepository;
pub use log::PgLogRepository;
pub use option::PgOptionRepository;
pub use redemption::PgRedemptionRepository;
pub use subscription::PgSubscriptionRepository;
pub use task::PgTaskRepository;
pub use token::PgTokenRepository;
pub use topup::PgTopUpRepository;
pub use user::PgUserRepository;
