//! 用户域:注册/登录(账密、邮箱验证码、2FA、Passkey)、OAuth 登录、个人设置、访问令牌、签到、邀请返利;管理员用户 CRUD
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   密码 bcrypt 校验;2FA 连续失败锁定至 locked_until;注册赠额度并记系统日志;邀请额度转入走事务+行锁;签到唯一约束防重;越权操作 root 用户被拒
