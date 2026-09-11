//! 分组域:分组列表、预填分组(model/tag/endpoint)CRUD
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   分组列表来自 abilities 去重;预填分组三种类型的校验
