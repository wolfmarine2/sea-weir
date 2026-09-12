//! 领域模型。字段与 new-api 表结构 / JSON 契约逐字段对齐。
//!
//! **命名铁律**:所有对外序列化字段为 `snake_case`。额度统一 `i64`(BIGINT),
//! unix 秒时间戳保留 `i64`,仅软删除列用 `DateTime<Utc>`(doc/system-design.md §7.4)。

pub mod ability;
pub mod channel;
pub mod log;
pub mod option;
pub mod subscription;
pub mod task;
pub mod token;
pub mod user;

pub use ability::Ability;
pub use channel::Channel;
pub use log::Log;
pub use option::Option as SystemOption;
pub use subscription::{SubscriptionPlan, UserSubscription};
pub use task::Task;
pub use token::{mask_token_key, NewToken, Token};
pub use user::User;
