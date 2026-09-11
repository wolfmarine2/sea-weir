//! 订阅域:套餐计划 CRUD、订阅购买、用户订阅管理、周期额度重置
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   购买数不超过 MaxPurchasePerUser;完成回调事务幂等;失效/删除触发分组回退;周期重置只影响到期订阅
