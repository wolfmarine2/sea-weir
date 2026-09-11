//! 中继引擎。8 个组件与 doc/architecture/c4/c4-l3-component-relay-engine.puml 一一对应。
//!
//! 管线(SEQ-002 → SEQ-003 → SEQ-004):
//! ```text
//! TokenAuth → 限流 → Distribute(选渠)
//!   → 校验/敏感词 → token 预估 → 计价 → 预扣
//!   → adaptor.convert_request → 上游调用
//!   → 流式:stream_pipe 转发 | 非流式:解析 usage
//!   → 结算(差额补退)| 失败:退款 + 重试/禁用判定
//!   → 写消费日志
//! ```

pub mod affinity;
pub mod autoban;
pub mod billing;
pub mod channel_cache;
pub mod convert;
pub mod pipeline;
pub mod select;
pub mod stream;
pub mod task_polling;
