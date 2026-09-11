//! `model` 域 handler。契约见 doc/architecture/CONTRACTS.md;端点清单见附录 A。

// TODO(TDD): 每个端点一个 async fn,签名形如
//   async fn xxx(State(state), auth: AuthUser, Json(req)) -> Result<Response, AppErrorWrapper>
//
// handler 层测试要点(用 axum 的 oneshot,不起真实服务):
//   - 参数校验失败返回业务错误而非 500
//   - 响应字段名为 snake_case 且与 new-api 一致
//   - 分页参数 p/page_size 解析正确(p 为 1 起)
