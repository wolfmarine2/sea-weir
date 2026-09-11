//! 异步任务平台适配器(10 个,对应 `TaskPlatform`)。
//!
//! 与同步适配器的关键差异:全额预扣、无信任旁路、终态由后台轮询驱动(SEQ-006)。

pub mod ali;
pub mod common;
pub mod doubao;
pub mod gemini;
pub mod generic;
pub mod hailuo;
pub mod jimeng;
pub mod kling;
pub mod sora;
pub mod suno;
pub mod vertex;
pub mod vidu;
