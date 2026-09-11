//! L1 单元 — 流式转发。用例文档:`cases/06-unit-convert-stream.md`
//!
//! 流式是最容易出兼容问题的地方(延迟一拍、usage 注入、[DONE] 时机),
//! 且线上故障表现为"客户端卡住"而非明确报错,排障成本高。
//!
//! 策略:用**录制的真实上游 SSE 流**做回放,逐 chunk 比对。
//! 溯源:SEQ-004、TEST-VECTORS.md §10

use sea_weir_core::relay::stream::{rewrite_chunk, StreamConfig};
use sea_weir_types::RelayFormat;

fn cfg(format: RelayFormat, include_usage: bool) -> StreamConfig {
    StreamConfig {
        ping_interval_secs: 10,
        idle_timeout_secs: 120,
        include_usage,
        format,
    }
}

/// TC-UNI-STM-001-POS:OpenAI → OpenAI 的 chunk 原样透传。
#[test]
fn tc_uni_stm_001_pos_openai_passthrough() {
    let line = r#"data: {"id":"1","choices":[{"delta":{"content":"hi"}}]}"#;
    let out = rewrite_chunk(line, RelayFormat::OpenAi).unwrap();
    assert_eq!(out.as_deref(), Some(line), "同格式应逐字节原样转发");
}

/// TC-UNI-STM-002-POS:`[DONE]` 标记原样保留。
#[test]
fn tc_uni_stm_002_pos_done_marker_preserved() {
    let out = rewrite_chunk("data: [DONE]", RelayFormat::OpenAi).unwrap();
    assert_eq!(out.as_deref(), Some("data: [DONE]"));
}

/// TC-UNI-STM-003-POS:OpenAI → Claude 的事件序列重组。
///
/// Claude 流式是有状态的事件序列(message_start → content_block_delta →
/// message_delta → message_stop),不是简单的字段改名。
#[test]
fn tc_uni_stm_003_pos_openai_to_claude_event_sequence() {
    // TODO(实现): 输入一段完整的 OpenAI SSE 序列,断言输出的 Claude 事件序列:
    //   1. 首个 chunk → message_start
    //   2. 中间 chunk → content_block_delta
    //   3. finish_reason 出现 → message_delta + message_stop
    // 用录制的 fixture 做黄金比对(cases/fixtures/baseline/sse_openai_basic.json)
}

/// TC-UNI-STM-004-POS:上游带 usage 时不重复注入。
#[test]
fn tc_uni_stm_004_pos_upstream_usage_not_duplicated() {
    // TODO(实现): 回放带 usage chunk 的流,断言输出中 usage 恰好出现一次
}

/// TC-UNI-STM-005-POS:上游无 usage + include_usage → 注入估算 usage。★
///
/// 客户端(如 LangChain)依赖 usage 做成本统计;缺了会让下游报错或统计为 0。
#[test]
fn tc_uni_stm_005_pos_inject_usage_when_requested() {
    let _c = cfg(RelayFormat::OpenAi, true);
    // TODO(实现): 回放无 usage 的流,断言 [DONE] 之前恰好注入一个 usage chunk,
    //             且 usage 的 token 数与 tiktoken 估算一致
}

/// TC-UNI-STM-006-POS:**未请求 usage 时同样注入**(录制实测校正)。★
///
/// 首版本用例断言的是"不注入",那是从文档推导的,**录制实测证明推导错了**:
/// `sse_openai_basic` 的请求体不含 `stream_options`,假上游发 4 帧,
/// new-api 输出 6 帧 —— 在 finish 帧之后、`[DONE]` 之前多注入了
/// `{"choices":[],"usage":{...}}`。
///
/// 即 new-api 的 usage 注入是**无条件**的。若按"不注入"实现,所有不显式传
/// `include_usage` 的客户端(LangChain / LiteLLM 等)流式成本统计都会是 0。
///
/// 溯源:`test/cases/fixtures/baseline/sse_openai_basic.json`;TEST-VECTORS §10 F4
#[test]
fn tc_uni_stm_006_pos_usage_injected_even_when_not_requested() {
    let _c = cfg(RelayFormat::OpenAi, false);
    // TODO(实现): 回放 sse_openai_basic 基线,断言:
    //   - 输出帧数 == 6(上游 4 帧 + 注入 usage 帧 + [DONE])
    //   - 倒数第二帧含 usage 且 choices 为空数组
}

/// TC-UNI-STM-007-BND:空行与注释行不破坏解析。
///
/// SSE 协议允许空行分隔与 `:` 注释行(常被上游用作心跳)。
#[rstest::rstest]
#[case("")]
#[case(":")]
#[case(": ping")]
fn tc_uni_stm_007_bnd_non_data_lines(#[case] line: &str) {
    let out = rewrite_chunk(line, RelayFormat::OpenAi);
    assert!(out.is_ok(), "非 data 行不应导致解析失败:{line:?}");
}

/// TC-UNI-STM-008-NEG:畸形 JSON 不得 panic。
///
/// 上游截断或网络中断会产生半截 JSON,必须优雅处理。
#[test]
fn tc_uni_stm_008_neg_malformed_json_no_panic() {
    let out = rewrite_chunk(r#"data: {"id":"1","choi"#, RelayFormat::Claude);
    assert!(out.is_err() || out.unwrap().is_none(), "畸形 chunk 应返回错误或跳过,不得 panic");
}

/// TC-UNI-STM-009-POS:ping 事件不污染业务数据流。
#[test]
fn tc_uni_stm_009_pos_ping_not_in_data_stream() {
    // TODO(实现): 断言 ping 以注释行(`: ping`)形式发送,
    //             不会被客户端当作 data 解析
}
