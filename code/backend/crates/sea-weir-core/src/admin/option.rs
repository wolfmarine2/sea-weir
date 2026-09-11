//! 设置域:系统选项 KV 读写、分层配置组、控制台设置迁移、渠道亲和性缓存管理
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   带 . 的键路由到分层配置组;写入后进程内即时生效并广播失效;非法值被拒绝且不落库
