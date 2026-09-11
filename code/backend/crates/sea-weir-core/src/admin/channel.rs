//! 渠道域:渠道 CRUD、批量/标签管理、多 key 管理、渠道测试与余额探测、上游模型拉取、Codex OAuth、Ollama 管理、上游更新检测
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   创建/更新渠道时 abilities 在同一事务内重建;取密钥需 RootAuth + 有效且一次性消费的凭证;测试通过且开启自动恢复时启用渠道
