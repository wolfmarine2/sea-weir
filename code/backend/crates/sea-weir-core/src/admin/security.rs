//! 安全域:统一安全验证(密码/2FA/Passkey 换敏感操作凭证)、邮箱验证码、Turnstile 人机校验
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   凭证一次性消费(第二次使用失败);凭证过期后失效;密码/TOTP/Passkey 三种验证方式等价换发凭证
