//! 模型域:模型元数据/厂商 CRUD、模型同步、缺失模型检测、部署管理(io.net)
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   缺失模型检测 = abilities 有而 models 无;上游同步的 preview 与 apply 结果一致
