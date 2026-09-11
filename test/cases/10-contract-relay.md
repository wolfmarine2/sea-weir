# 10. 中继面契约测试用例

**版本**:v1.0 | **层**:L3(录制基线)
**测试代码**:`test/src/contract_relay_api.rs`

## 定位

中继面的契约对象是**上游原生协议**(OpenAI / Claude / Gemini / MJ),
不是我们自己的 DTO。客户端(OpenAI SDK、Anthropic SDK、LangChain、LiteLLM、
各种 Chat 客户端)会严格按各自协议解析,形状错了会直接抛异常。

## 用例清单

| 编号 | 名称 | 优先级 | 关注点 |
|---|---|---|---|
| TC-CTR-RLY-001-POS | OpenAI 非流式结构 | P0 | 6 个必需字段 |
| TC-CTR-RLY-002-POS | usage 字段完整且为整数 | P0 | 下游成本统计 |
| TC-CTR-RLY-003-POS | Claude 原生结构 | P0 | input/output_tokens 命名 |
| TC-CTR-RLY-004-POS | 三种错误体形状 ★ | P0 | 按入口格式分派 |
| TC-CTR-RLY-005-POS | `/v1/models` 列表结构 | P1 | — |
| TC-CTR-RLY-006-POS | dashboard 计费兼容 | P1 | 成本监控工具依赖 |
| TC-CTR-RLY-010-POS | SSE 事件序列 ★ | P0 | 逐 chunk 回放 |
| TC-CTR-RLY-011-POS | 占位端点不扣费 | P1 | — |

## 关键用例详述

### TC-CTR-RLY-004-POS:错误体按入口格式分派 ★

```yaml
背景: >
  同一个内部错误,出口形状取决于**客户端用的是哪套协议**:
  - OpenAI SDK 读 `error.message`
  - Anthropic SDK 校验顶层 `type == "error"`
  - MJ 客户端读 `code` / `description`
  发错形状,客户端拿不到错误信息,只能看到"解析失败",排障极困难。

matrix:
  OpenAI:
    shape: '{"error":{"message":…,"type":…,"code":…}}'
  Claude:
    shape: '{"type":"error","error":{"type":…,"message":…}}'
    note:  "顶层 type 必须是字符串 'error',Anthropic SDK 据此判定"
  Midjourney:
    shape: '{"code":…,"description":…,"result":…}'
    note:  "code=30 对应 HTTP 429(队列满)"
```

### TC-CTR-RLY-010-POS:SSE 事件序列 ★

```yaml
背景: >
  流式契约错误的表现是"客户端卡住不结束"或"内容缺一段",
  不会有明确报错。最典型的是漏发 `data: [DONE]` ——
  OpenAI SDK 会一直等到超时。

方法: "用录制的完整流做黄金比对,允许 id/created 等易变字段有差异"
fixture: "relay_openai_chat_stream(含 chunks 数组)"

assert:
  - "每个事件以 'data: ' 开头"
  - "最后一个事件含 [DONE]"
  - "首个 chunk 含 role"
  - "逐 chunk 与基线比对(忽略 id/created)"
```

## 待录制的 fixture 清单

| fixture 名 | 端点 | 备注 |
|---|---|---|
| `relay_openai_chat_nonstream` | POST /v1/chat/completions | stream=false |
| `relay_openai_chat_stream` | POST /v1/chat/completions | stream=true,存 chunks 数组 |
| `relay_claude_messages` | POST /v1/messages | Claude 原生 |
| `relay_models_list` | GET /v1/models | — |
| `relay_error_openai` | 无效模型请求 | OpenAI 错误体 |
| `relay_error_claude` | Claude 路径错误 | Claude 错误体 |
| `relay_error_mj` | MJ 路径错误 | MJ 错误体 |
| `dashboard_billing_subscription` | GET /dashboard/billing/subscription | — |
| `relay_not_implemented` | GET /v1/files | 占位端点 |
| `sse_openai_basic` | 完整 SSE 流 | 供转换测试回放 |
