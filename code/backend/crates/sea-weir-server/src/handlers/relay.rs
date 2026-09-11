//! 中继面入口。对应 C4 组件 `relay_entry`。
//!
//! 职责:解析入口格式 → 调用 `sea_weir_core::relay::pipeline::execute`
//! → 按 `RelayFormat` 格式化出口(成功透传上游 / 失败转错误体)。
//!
//! 重试循环在 core 的 pipeline 内,handler 只负责 HTTP 边界。

// TODO(TDD):
//   - relay_openai / relay_claude / relay_gemini / relay_task / relay_playground
//   - not_implemented(11 条占位端点)
//
// 测试要点:
//   - 四种入口格式的成功响应逐字节透传
//   - 错误出口按格式分派
//   - 流式响应的 Content-Type 与 header 正确(text/event-stream、no-cache)
//   - 客户端断开时不泄漏上游连接
