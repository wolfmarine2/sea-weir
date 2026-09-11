//! OAuth Provider 管理域:自定义 OAuth Provider CRUD 与发现(root)
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   OIDC discovery 解析;Provider 配置校验;删除时已绑定用户的处理
