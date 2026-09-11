//! 日志域:消费/充值/管理/系统/错误/退款日志查询、统计(rpm/tpm)、历史清理、日志归档文件
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   按时间/类型/模型/用户/渠道过滤的组合查询;stat 的 rpm/tpm 窗口为最近 60s;self 系列严格限本人
