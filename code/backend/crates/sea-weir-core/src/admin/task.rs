//! 任务域:异步任务(视频/音乐)与 Midjourney 记录查询
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   self 系列严格限本人;管理员可查全量
