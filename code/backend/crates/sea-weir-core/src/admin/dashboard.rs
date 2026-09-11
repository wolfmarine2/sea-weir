//! 数据看板域:额度消耗聚合数据(按模型/用户)、Uptime 监控状态
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   按小时聚合的 quota_data 与 logs 明细对账一致
