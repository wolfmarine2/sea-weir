//! 性能运维域:运行统计、GC、磁盘缓存与性能日志管理(root)
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   root 才可调用;GC 与磁盘缓存清理不影响在途请求
