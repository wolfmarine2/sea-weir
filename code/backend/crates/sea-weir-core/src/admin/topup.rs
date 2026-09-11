//! 充值与支付域:兑换码 CRUD、在线充值(易支付/Stripe/Creem/Waffo)、支付回调、人工补单
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   兑换码并发只成功一次;支付回调签名校验;重复回调幂等返回成功;人工补单同事务幂等
