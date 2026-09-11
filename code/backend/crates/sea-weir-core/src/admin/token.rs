//! 令牌域:令牌 CRUD、模型限制、IP 白名单、分组、批量操作、令牌自查(只读)
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   名称在本人范围内唯一;分组必须在可用分组内;生成 key 长度 48 且不含 sk- 前缀;expired_time 默认 -1
