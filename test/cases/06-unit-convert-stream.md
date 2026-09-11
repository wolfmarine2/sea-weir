# 06. 格式转换与流式转发测试用例

**版本**:v1.0 | **层**:L1 | **测试代码**:`test/src/unit_convert.rs`、`unit_stream.rs`
**基准来源**:TEST-VECTORS.md §10;SEQ-004;ADR-006

## 定位

格式转换是纯函数,适合表驱动;流式是有状态管线,适合**录制回放**。

流式缺陷的特殊之处:表现是"客户端卡住"或"内容不完整"而非明确报错,
线上难以察觉,用户投诉时也说不清。所以必须在测试阶段用真实流回放覆盖。

## 用例清单

### 格式转换(`unit_convert.rs`)

| 编号 | 名称 | 优先级 | 关注点 |
|---|---|---|---|
| TC-UNI-CVT-001-POS | OpenAI→Claude:system 提取到顶层 ★ | P0 | 漏了会静默丢失 system prompt |
| TC-UNI-CVT-002-POS | OpenAI→Claude:补 max_tokens | P0 | Claude 必填,不补则上游 400 |
| TC-UNI-CVT-003-POS | OpenAI→Gemini:role 映射 | P0 | assistant → model |
| TC-UNI-CVT-004-POS | Claude→OpenAI:content blocks 合并 | P0 | usage 字段名也要映射 |
| TC-UNI-CVT-005-POS | 往返转换语义不丢失 | P1 | — |
| TC-UNI-CVT-006-POS | 转换链记录跳数 | P1 | 影响计费语义判定 |
| TC-UNI-CVT-007-BND | 同格式转换是恒等 | P1 | 不应无谓改写 |
| TC-UNI-CVT-008-POS | 多模态内容块 | P1 | image_url → image |

### 流式(`unit_stream.rs`)

| 编号 | 名称 | 优先级 | 关注点 |
|---|---|---|---|
| TC-UNI-STM-001-POS | OpenAI 原样透传 | P0 | 逐字节 |
| TC-UNI-STM-002-POS | `[DONE]` 保留 | P0 | 缺了客户端不会结束 |
| TC-UNI-STM-003-POS | OpenAI→Claude 事件序列 ★ | P0 | 有状态重组 |
| TC-UNI-STM-004-POS | 上游有 usage 不重复注入 | P0 | — |
| TC-UNI-STM-005-POS | 无 usage + 请求了 → 注入 ★ | P0 | 下游成本统计依赖 |
| TC-UNI-STM-006-NEG | 未请求 usage 不注入 | P0 | 多发字段会让严格客户端报错 |
| TC-UNI-STM-007-BND | 空行/注释行不破坏解析 | P1 | SSE 协议要求 |
| TC-UNI-STM-008-NEG | 畸形 JSON 不 panic | P0 | 上游截断是常态 |
| TC-UNI-STM-009-POS | ping 不污染数据流 | P1 | — |

## 关键用例详述

### TC-UNI-CVT-001-POS:system 消息提取 ★

```yaml
背景: >
  OpenAI 把 system 放在 messages 数组的第一条;Claude 放在顶层 system 字段。
  若照搬 messages 数组,Claude 会拒绝(role 只接受 user/assistant)或
  更糟 —— 某些兼容层会静默丢弃,导致 system prompt 完全失效。
  用户看到的现象是"模型不听指令",极难联想到是网关转换的问题。

input:
  messages:
    - { role: system,    content: "You are helpful." }
    - { role: user,      content: "hi" }
expected:
  system: "You are helpful."      # 提到顶层
  messages: [ { role: user, content: "hi" } ]   # 只剩一条
```

### TC-UNI-STM-003-POS:OpenAI → Claude 事件序列 ★

```yaml
背景: >
  Claude 的流式不是"字段改名",而是一套**有状态的事件序列**:
    message_start → content_block_start → content_block_delta*
    → content_block_stop → message_delta → message_stop
  必须由转换层维护状态机来生成,单看一个 chunk 无法决定该发什么事件。

策略: "用录制的完整 OpenAI SSE 流做输入,逐事件比对期望的 Claude 序列"
fixture: "cases/fixtures/baseline/sse_openai_basic.json"

assert:
  - "首个含 role 的 chunk → message_start"
  - "含 content delta 的 chunk → content_block_delta"
  - "含 finish_reason 的 chunk → message_delta + message_stop"
  - "事件顺序严格,不得乱序或缺失"
```

### TC-UNI-STM-005/006:usage 注入 ★

```yaml
背景: >
  OpenAI 的流式默认不返回 usage,需客户端显式传 stream_options.include_usage。
  部分上游不支持该选项,网关需要用 tiktoken 估算并注入。
  两个方向都会出问题:
  - 该注入没注入 → 下游成本统计全是 0(LangChain / LiteLLM 都会受影响)
  - 不该注入却注入 → 严格解析的客户端遇到未知 chunk 会报错

matrix:
  - { upstream_has_usage: true,  include_usage: true,  expect: "透传,不重复注入" }
  - { upstream_has_usage: false, include_usage: true,  expect: "[DONE] 前注入一个 usage chunk" }
  - { upstream_has_usage: false, include_usage: false, expect: "不注入" }
  - { upstream_has_usage: true,  include_usage: false, expect: "透传(上游给了就给)" }
```

### TC-UNI-STM-008-NEG:畸形 JSON 不 panic

```yaml
背景: >
  上游连接中断会产生半截 chunk(如 `data: {"id":"1","choi`)。
  若解析时 unwrap,整个 worker 线程会 panic,影响同进程的其他请求。

input:  'data: {"id":"1","choi'
expected: "返回 Err 或 Ok(None),不得 panic"
```
