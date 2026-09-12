//! # sea-weir-repository
//!
//! 数据访问层:Repository Trait 定义 + sqlx(openGauss)实现 + Valkey 读穿与失效广播。
//!
//! **约束**:本 crate 只依赖 `sea-weir-types`,不依赖 core / adaptors。
//!
//! ## 关键不变量(doc/architecture/CONTRACTS.md §12)
//! 1. 额度增减必须是**数据库原子表达式**,且带条件守卫
//!    (`UPDATE ... SET quota = quota - $1 WHERE id = $2 AND quota >= $1`,0 行 = 余额不足)。
//!    这是相对 new-api 的实质改进 —— Go 版 `model/user.go` 是无条件的 `quota - ?`。
//! 2. 兑换 / 补单 / 订阅预扣必须**事务 + 行锁**(`SELECT ... FOR UPDATE`)。
//! 3. 幂等键唯一约束兜底(`trade_no`、`request_id`)。
//! 4. 缓存以 DB 为准:读穿 + 失效广播,TTL 60s 兜底。

#![forbid(unsafe_code)]

pub mod cache;
pub mod pg;
pub mod pool;
pub mod traits;

#[cfg(feature = "mock")]
pub mod mocks;

pub use pool::{DbPools, RepositoryContext};
pub use traits::*;
