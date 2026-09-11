//! 计费与定价域:模型定价、模型/分组/缓存倍率配置、上游倍率同步、阶梯计费、dashboard 计费兼容端点
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   定价视图 = abilities × models × vendors × 倍率合成;进程内 1 分钟缓存;倍率变更后缓存即时失效
