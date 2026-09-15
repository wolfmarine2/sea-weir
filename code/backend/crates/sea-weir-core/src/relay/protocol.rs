//! 上游协议适配:同一个渠道可以对接三种外部协议。
//!
//! - `openai_chat`     :`POST /v1/chat/completions`(默认,`Authorization: Bearer`)
//! - `openai_responses`:`POST /v1/responses`(OpenAI Responses API,`Authorization: Bearer`)
//! - `anthropic`       :`POST /v1/messages`(`x-api-key` + `anthropic-version`)
//!
//! 中继内部统一以 **OpenAI Chat Completions** 为规范格式(canonical):
//! 入口协议 → canonical(见 [`super::convert`])→ 本模块转成上游协议;
//! 上游响应(含 SSE)→ canonical → 入口协议。
//!
//! 选择来源:渠道 `setting.api_protocol`;未配置时按渠道类型推断
//! (Anthropic 渠道 → anthropic,其余 → openai_chat),保持既有行为不变。

use std::collections::HashMap;

use serde_json::{json, Map, Value};
use sea_weir_adaptors::ApiType;
use sea_weir_types::{domain::Channel, AppError, AppResult};

use super::convert::openai_to_claude;

/// 上游协议。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamProtocol {
    OpenAiChat,
    OpenAiResponses,
    Anthropic,
}

impl UpstreamProtocol {
    /// 从 `setting.api_protocol` 解析;空/未知值返回 `None`(走类型推断)。
    pub fn from_setting(setting: Option<&Value>) -> Option<Self> {
        match setting?.get("api_protocol")?.as_str()?.trim() {
            "openai_chat" | "openai" | "chat" => Some(Self::OpenAiChat),
            "openai_responses" | "responses" => Some(Self::OpenAiResponses),
            "anthropic" | "claude" => Some(Self::Anthropic),
            _ => None,
        }
    }

    /// 渠道生效的上游协议:显式配置优先,否则按渠道类型推断。
    pub fn resolve(channel: &Channel) -> Self {
        Self::resolve_for(channel.r#type, channel.setting.as_ref())
    }

    /// 与 [`resolve`](Self::resolve) 同逻辑,便于脱离 `Channel` 单测。
    pub fn resolve_for(channel_type: i32, setting: Option<&Value>) -> Self {
        if let Some(p) = Self::from_setting(setting) {
            return p;
        }
        match ApiType::from_channel_type(channel_type) {
            Some(ApiType::Anthropic) => Self::Anthropic,
            _ => Self::OpenAiChat,
        }
    }

    /// 上游路径(配合 `upstream_url` 去重 `/v1`)。
    pub fn path(self) -> &'static str {
        match self {
            Self::OpenAiChat => "/v1/chat/completions",
            Self::OpenAiResponses => "/v1/responses",
            Self::Anthropic => "/v1/messages",
        }
    }

    /// 是否需要 Anthropic 原生鉴权头(`x-api-key` + `anthropic-version`)。
    pub fn uses_anthropic_auth(self) -> bool {
        matches!(self, Self::Anthropic)
    }
}

/// canonical(OpenAI Chat)→ 上游请求体。
pub fn chat_to_upstream(body: &Value, protocol: UpstreamProtocol) -> AppResult<Value> {
    Ok(match protocol {
        UpstreamProtocol::OpenAiChat => body.clone(),
        UpstreamProtocol::OpenAiResponses => chat_to_responses(body),
        UpstreamProtocol::Anthropic => chat_to_anthropic(body),
    })
}

/// 上游响应体(非流式)→ canonical(OpenAI Chat)。
pub fn upstream_to_chat(body: &Value, protocol: UpstreamProtocol) -> AppResult<Value> {
    Ok(match protocol {
        UpstreamProtocol::OpenAiChat => body.clone(),
        UpstreamProtocol::OpenAiResponses => responses_to_chat(body),
        UpstreamProtocol::Anthropic => anthropic_to_chat(body),
    })
}

// ───────────────────────── 请求:Chat → Responses ─────────────────────────

/// Responses API 允许的顶层参数(未列出的一律丢弃,避免上游 400)。
const RESPONSES_KEYS: &[&str] = &[
    "model",
    "input",
    "instructions",
    "max_output_tokens",
    "temperature",
    "top_p",
    "stream",
    "tools",
    "tool_choice",
    "parallel_tool_calls",
    "metadata",
    "user",
    "reasoning",
    "text",
    "truncation",
    "store",
];

fn chat_to_responses(body: &Value) -> Value {
    let mut out = Map::new();
    let mut instructions: Vec<String> = Vec::new();
    let mut input: Vec<Value> = Vec::new();

    if let Some(list) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in list {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            match role {
                "system" | "developer" => {
                    if let Some(text) = text_of(msg.get("content")) {
                        instructions.push(text);
                    }
                }
                "tool" => {
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": msg.get("tool_call_id").cloned().unwrap_or(json!("")),
                        "output": text_of(msg.get("content")).unwrap_or_default(),
                    }));
                }
                "assistant" if msg.get("tool_calls").is_some() => {
                    if let Some(text) = text_of(msg.get("content")) {
                        if !text.is_empty() {
                            input.push(message_item("assistant", &text));
                        }
                    }
                    if let Some(calls) = msg.get("tool_calls").and_then(|c| c.as_array()) {
                        for call in calls {
                            input.push(json!({
                                "type": "function_call",
                                "call_id": call.get("id").cloned().unwrap_or(json!("")),
                                "name": call.pointer("/function/name").cloned().unwrap_or(json!("")),
                                "arguments": call
                                    .pointer("/function/arguments")
                                    .cloned()
                                    .unwrap_or(json!("{}")),
                            }));
                        }
                    }
                }
                _ => {
                    input.push(responses_message_item(role, msg.get("content")));
                }
            }
        }
    }

    for (src, dst) in [
        ("model", "model"),
        ("stream", "stream"),
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("user", "user"),
        ("metadata", "metadata"),
    ] {
        if let Some(v) = body.get(src) {
            if !v.is_null() {
                out.insert(dst.into(), v.clone());
            }
        }
    }
    // max_tokens / max_completion_tokens → max_output_tokens
    for key in ["max_output_tokens", "max_completion_tokens", "max_tokens"] {
        if let Some(v) = body.get(key) {
            if !v.is_null() {
                out.insert("max_output_tokens".into(), v.clone());
                break;
            }
        }
    }
    if !instructions.is_empty() {
        out.insert("instructions".into(), json!(instructions.join("\n")));
    }
    out.insert("input".into(), Value::Array(input));
    if let Some(tools) = chat_tools_to_responses(body) {
        out.insert("tools".into(), tools);
    }
    if let Some(choice) = chat_tool_choice_to_responses(body.get("tool_choice")) {
        out.insert("tool_choice".into(), choice);
    }
    // 只保留上游认识的键(param_override 在这之后应用,可再补原生参数)。
    prune(&mut out, RESPONSES_KEYS);
    Value::Object(out)
}

fn responses_message_item(role: &str, content: Option<&Value>) -> Value {
    match content {
        Some(Value::Array(parts)) => {
            let mut out_parts: Vec<Value> = Vec::new();
            for part in parts {
                match part.get("type").and_then(|t| t.as_str()) {
                    Some("image_url") => {
                        out_parts.push(json!({
                            "type": "input_image",
                            "image_url": part.pointer("/image_url/url").cloned().unwrap_or(json!("")),
                        }));
                    }
                    Some("text") | None => {
                        if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                            out_parts.push(part_text(role, t));
                        }
                    }
                    _ => {}
                }
            }
            json!({"role": role, "content": out_parts})
        }
        Some(Value::String(s)) => message_item(role, s),
        _ => message_item(role, ""),
    }
}

fn message_item(role: &str, text: &str) -> Value {
    json!({"role": role, "content": [part_text(role, text)]})
}

/// assistant 用 `output_text`,user 用 `input_text`。
fn part_text(role: &str, text: &str) -> Value {
    let ty = if role == "assistant" {
        "output_text"
    } else {
        "input_text"
    };
    json!({"type": ty, "text": text})
}

/// Chat `tools`/`functions` → Responses `tools`。
fn chat_tools_to_responses(body: &Value) -> Option<Value> {
    let raw = body
        .get("tools")
        .and_then(|t| t.as_array())
        .or_else(|| body.get("functions").and_then(|t| t.as_array()))?;
    let tools: Vec<Value> = raw
        .iter()
        .filter_map(|t| {
            let f = t.get("function").unwrap_or(t);
            let name = f.get("name")?.as_str()?;
            let mut item = Map::new();
            item.insert("type".into(), json!("function"));
            item.insert("name".into(), json!(name));
            if let Some(d) = f.get("description") {
                item.insert("description".into(), d.clone());
            }
            if let Some(p) = f.get("parameters") {
                item.insert("parameters".into(), p.clone());
            }
            Some(Value::Object(item))
        })
        .collect();
    (!tools.is_empty()).then_some(Value::Array(tools))
}

/// Chat `tool_choice` → Responses `tool_choice`。
fn chat_tool_choice_to_responses(choice: Option<&Value>) -> Option<Value> {
    match choice? {
        Value::String(s) if s == "auto" || s == "none" || s == "required" => Some(json!(s)),
        Value::Object(o) => {
            let v = Value::Object(o.clone());
            let named = v.pointer("/function/name").and_then(|n| n.as_str()).or_else(|| {
                (v.get("type").and_then(|t| t.as_str()) == Some("function"))
                    .then(|| v.get("name").and_then(|n| n.as_str()))
                    .flatten()
            });
            named.map(|name| json!({"type": "function", "name": name}))
        }
        _ => None,
    }
}

// ───────────────────────── 请求:Chat → Anthropic ─────────────────────────

/// Anthropic Messages 允许的顶层参数。
const ANTHROPIC_KEYS: &[&str] = &[
    "model",
    "messages",
    "system",
    "max_tokens",
    "stop_sequences",
    "stream",
    "temperature",
    "top_p",
    "top_k",
    "metadata",
    "tools",
    "tool_choice",
    "thinking",
];

fn chat_to_anthropic(body: &Value) -> Value {
    // 复用既有转换:system 提取、内容块、max_tokens 兜底、去 stream_options。
    let base = openai_to_claude(body);
    let mut out = base.as_object().cloned().unwrap_or_default();

    // 消息:assistant.tool_calls → tool_use 块;role=tool → user.tool_result;并合并同角色相邻消息。
    let messages = out
        .get("messages")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    let converted = anthropic_messages(&messages);
    out.insert("messages".into(), Value::Array(converted));

    // stop → stop_sequences
    if let Some(stop) = body.get("stop") {
        let seqs = match stop {
            Value::String(s) => json!([s]),
            Value::Array(a) => json!(a),
            _ => json!([]),
        };
        if seqs.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
            out.insert("stop_sequences".into(), seqs);
        }
    }
    // tools / functions
    if let Some(tools) = chat_tools_to_anthropic(body) {
        out.insert("tools".into(), tools);
    }
    if let Some(choice) = chat_tool_choice_to_anthropic(body.get("tool_choice")) {
        out.insert("tool_choice".into(), choice);
    }
    // 透传原生 Anthropic 参数(top_k 等),但仅限白名单。
    for key in ["top_k", "thinking", "metadata"] {
        if let Some(v) = body.get(key) {
            if !v.is_null() {
                out.insert(key.into(), v.clone());
            }
        }
    }
    prune(&mut out, ANTHROPIC_KEYS);
    Value::Object(out)
}

/// 把 OpenAI 风格消息改写成 Anthropic 消息(含 tool_use / tool_result),并合并相邻同角色。
fn anthropic_messages(messages: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        let item = match role {
            "tool" => json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": msg.get("tool_call_id").cloned().unwrap_or(json!("")),
                    "content": text_of(msg.get("content")).unwrap_or_default(),
                }],
            }),
            "assistant" if msg.get("tool_calls").is_some() => {
                let mut blocks: Vec<Value> = Vec::new();
                if let Some(text) = text_of(msg.get("content")) {
                    if !text.is_empty() {
                        blocks.push(json!({"type": "text", "text": text}));
                    }
                }
                if let Some(calls) = msg.get("tool_calls").and_then(|c| c.as_array()) {
                    for call in calls {
                        let args = call
                            .pointer("/function/arguments")
                            .and_then(|a| a.as_str())
                            .unwrap_or("{}");
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": call.get("id").cloned().unwrap_or(json!("")),
                            "name": call.pointer("/function/name").cloned().unwrap_or(json!("")),
                            "input": serde_json::from_str::<Value>(args).unwrap_or(json!({})),
                        }));
                    }
                }
                json!({"role": "assistant", "content": blocks})
            }
            "assistant" | "user" => msg.clone(),
            // system 已提到顶层;其余角色按 user 处理。
            _ => json!({"role": "user", "content": msg.get("content").cloned().unwrap_or(json!(""))}),
        };
        merge_anthropic_message(&mut out, item);
    }
    out
}

/// 合并相邻同角色消息(Anthropic 要求 user/assistant 交替)。
fn merge_anthropic_message(out: &mut Vec<Value>, item: Value) {
    let role = item.get("role").and_then(|r| r.as_str()).unwrap_or("user");
    if let Some(last) = out.last_mut() {
        if last.get("role").and_then(|r| r.as_str()) == Some(role) {
            let mut blocks = blocks_of(last.get("content"));
            blocks.extend(blocks_of(item.get("content")));
            if let Some(obj) = last.as_object_mut() {
                obj.insert("content".into(), Value::Array(blocks));
            }
            return;
        }
    }
    out.push(item);
}

fn blocks_of(content: Option<&Value>) -> Vec<Value> {
    match content {
        Some(Value::Array(a)) => a.clone(),
        Some(Value::String(s)) => vec![json!({"type": "text", "text": s})],
        _ => Vec::new(),
    }
}

/// Chat `tools`/`functions` → Anthropic `tools`。
fn chat_tools_to_anthropic(body: &Value) -> Option<Value> {
    let raw = body
        .get("tools")
        .and_then(|t| t.as_array())
        .or_else(|| body.get("functions").and_then(|t| t.as_array()))?;
    let tools: Vec<Value> = raw
        .iter()
        .filter_map(|t| {
            let f = t.get("function").unwrap_or(t);
            let name = f.get("name")?.as_str()?;
            let mut item = Map::new();
            item.insert("name".into(), json!(name));
            if let Some(d) = f.get("description") {
                item.insert("description".into(), d.clone());
            }
            item.insert(
                "input_schema".into(),
                f.get("parameters").cloned().unwrap_or(json!({"type": "object"})),
            );
            Some(Value::Object(item))
        })
        .collect();
    (!tools.is_empty()).then_some(Value::Array(tools))
}

/// Chat `tool_choice` → Anthropic `tool_choice`。
fn chat_tool_choice_to_anthropic(choice: Option<&Value>) -> Option<Value> {
    match choice? {
        Value::String(s) => match s.as_str() {
            "auto" => Some(json!({"type": "auto"})),
            "required" | "any" => Some(json!({"type": "any"})),
            "none" => None,
            _ => None,
        },
        Value::Object(o) => {
            let v = Value::Object(o.clone());
            v.pointer("/function/name")
                .and_then(|n| n.as_str())
                .map(|name| json!({"type": "tool", "name": name}))
        }
        _ => None,
    }
}

// ───────────────────────── 响应:上游 → Chat ─────────────────────────

fn responses_to_chat(body: &Value) -> Value {
    let mut text = String::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    if let Some(output) = body.get("output").and_then(|o| o.as_array()) {
        for item in output {
            match item.get("type").and_then(|t| t.as_str()) {
                Some("message") => {
                    if let Some(parts) = item.get("content").and_then(|c| c.as_array()) {
                        for p in parts {
                            if p.get("type").and_then(|t| t.as_str()) == Some("output_text") {
                                if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                                    text.push_str(t);
                                }
                            }
                        }
                    }
                }
                Some("function_call") => {
                    tool_calls.push(json!({
                        "id": item.get("call_id").cloned().unwrap_or(json!("")),
                        "type": "function",
                        "function": {
                            "name": item.get("name").cloned().unwrap_or(json!("")),
                            "arguments": item.get("arguments").cloned().unwrap_or(json!("{}")),
                        }
                    }));
                }
                _ => {}
            }
        }
    }
    let finish = if !tool_calls.is_empty() {
        "tool_calls"
    } else if body.get("status").and_then(|s| s.as_str()) == Some("incomplete") {
        "length"
    } else {
        "stop"
    };
    let input = body.pointer("/usage/input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
    let output = body.pointer("/usage/output_tokens").and_then(|v| v.as_i64()).unwrap_or(0);

    let mut message = Map::new();
    message.insert("role".into(), json!("assistant"));
    message.insert(
        "content".into(),
        if text.is_empty() { Value::Null } else { json!(text) },
    );
    if !tool_calls.is_empty() {
        message.insert("tool_calls".into(), Value::Array(tool_calls));
    }
    json!({
        "id": body.get("id").cloned().unwrap_or(json!("chatcmpl-seaweir")),
        "object": "chat.completion",
        "created": body.get("created_at").cloned().unwrap_or(json!(0)),
        "model": body.get("model").cloned().unwrap_or(Value::Null),
        "choices": [{"index": 0, "message": Value::Object(message), "finish_reason": finish}],
        "usage": {"prompt_tokens": input, "completion_tokens": output, "total_tokens": input + output},
    })
}

fn anthropic_to_chat(body: &Value) -> Value {
    let mut text = String::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    if let Some(blocks) = body.get("content").and_then(|c| c.as_array()) {
        for b in blocks {
            match b.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                        text.push_str(t);
                    }
                }
                Some("tool_use") => {
                    tool_calls.push(json!({
                        "id": b.get("id").cloned().unwrap_or(json!("")),
                        "type": "function",
                        "function": {
                            "name": b.get("name").cloned().unwrap_or(json!("")),
                            "arguments": b.get("input").cloned().unwrap_or(json!({})).to_string(),
                        }
                    }));
                }
                _ => {}
            }
        }
    }
    let finish = match body.get("stop_reason").and_then(|s| s.as_str()) {
        Some("max_tokens") => "length",
        Some("tool_use") => "tool_calls",
        _ => "stop",
    };
    let input = body.pointer("/usage/input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
    let output = body.pointer("/usage/output_tokens").and_then(|v| v.as_i64()).unwrap_or(0);

    let mut message = Map::new();
    message.insert("role".into(), json!("assistant"));
    message.insert(
        "content".into(),
        if text.is_empty() { Value::Null } else { json!(text) },
    );
    if !tool_calls.is_empty() {
        message.insert("tool_calls".into(), Value::Array(tool_calls));
    }
    json!({
        "id": body.get("id").cloned().unwrap_or(json!("chatcmpl-seaweir")),
        "object": "chat.completion",
        "model": body.get("model").cloned().unwrap_or(Value::Null),
        "choices": [{"index": 0, "message": Value::Object(message), "finish_reason": finish}],
        "usage": {"prompt_tokens": input, "completion_tokens": output, "total_tokens": input + output},
    })
}

// ───────────────────────── 流式:上游 SSE → Chat SSE ─────────────────────────

/// 上游 SSE → canonical(OpenAI Chat)SSE 的状态机。
///
/// 用法:把上游每个 `data:` 负载喂给 [`push`](Self::push),拿回若干条 chat SSE 负载
/// (JSON 文本,不含 `data: ` 前缀);流结束后调 [`finish`](Self::finish) 收尾。
/// `[DONE]` 由 `finish` 统一产生,上游自己的 `[DONE]` 不要喂进来。
pub struct StreamNormalizer {
    proto: UpstreamProtocol,
    id: String,
    model: String,
    created: i64,
    role_sent: bool,
    input_tokens: i64,
    output_tokens: i64,
    finish_reason: Option<String>,
    finished: bool,
    /// Anthropic content block index → OpenAI tool_calls index。
    block_tool_index: HashMap<i64, usize>,
    /// Responses output item id → OpenAI tool_calls index。
    item_tool_index: HashMap<String, usize>,
    next_tool_index: usize,
}

impl StreamNormalizer {
    pub fn new(proto: UpstreamProtocol, model: &str) -> Self {
        Self {
            proto,
            id: "chatcmpl-seaweir".into(),
            model: model.to_string(),
            created: chrono::Utc::now().timestamp(),
            role_sent: false,
            input_tokens: 0,
            output_tokens: 0,
            finish_reason: None,
            finished: false,
            block_tool_index: HashMap::new(),
            item_tool_index: HashMap::new(),
            next_tool_index: 0,
        }
    }

    /// 消费一条上游 SSE `data:` 负载,返回要下发的 chat SSE 负载。
    pub fn push(&mut self, payload: &str) -> AppResult<Vec<String>> {
        let value: Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            // 上游偶发非 JSON 心跳/注释行:忽略而非中断整条流。
            Err(_) => return Ok(Vec::new()),
        };
        match self.proto {
            UpstreamProtocol::OpenAiChat => Ok(vec![payload.to_string()]),
            UpstreamProtocol::OpenAiResponses => self.push_responses(&value),
            UpstreamProtocol::Anthropic => self.push_anthropic(&value),
        }
    }

    /// 流收尾:补发 usage 与 `[DONE]`。
    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.finished {
            self.finished = true;
            out.push(self.usage_chunk());
        }
        out.push("[DONE]".to_string());
        out
    }

    fn push_responses(&mut self, value: &Value) -> AppResult<Vec<String>> {
        let mut out = Vec::new();
        match value.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "response.created" | "response.in_progress" => {
                if let Some(resp) = value.get("response") {
                    if let Some(id) = resp.get("id").and_then(|i| i.as_str()) {
                        self.id = id.to_string();
                    }
                    if let Some(m) = resp.get("model").and_then(|m| m.as_str()) {
                        self.model = m.to_string();
                    }
                }
            }
            "response.output_text.delta" => {
                if let Some(d) = value.get("delta").and_then(|d| d.as_str()) {
                    self.emit_delta(&mut out, json!({"content": d}));
                }
            }
            "response.output_item.added" => {
                if let Some(item) = value.get("item") {
                    if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                        let idx = self.next_tool_index;
                        self.next_tool_index += 1;
                        if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                            self.item_tool_index.insert(id.to_string(), idx);
                        }
                        self.emit_delta(
                            &mut out,
                            json!({
                                "tool_calls": [{
                                    "index": idx,
                                    "id": item.get("call_id").cloned().unwrap_or(json!("")),
                                    "type": "function",
                                    "function": {
                                        "name": item.get("name").cloned().unwrap_or(json!("")),
                                        "arguments": "",
                                    }
                                }]
                            }),
                        );
                    }
                }
            }
            "response.function_call_arguments.delta" => {
                if let Some(idx) = value
                    .get("item_id")
                    .and_then(|i| i.as_str())
                    .and_then(|id| self.item_tool_index.get(id))
                    .copied()
                {
                    self.emit_delta(
                        &mut out,
                        json!({
                            "tool_calls": [{
                                "index": idx,
                                "function": {"arguments": value.get("delta").cloned().unwrap_or(json!(""))}
                            }]
                        }),
                    );
                }
            }
            "response.completed" => {
                if let Some(resp) = value.get("response") {
                    self.absorb_responses_usage(resp);
                }
                let reason = if self.next_tool_index > 0 { "tool_calls" } else { "stop" };
                out.extend(self.finish_events(reason));
            }
            "response.incomplete" => {
                if let Some(resp) = value.get("response") {
                    self.absorb_responses_usage(resp);
                }
                out.extend(self.finish_events("length"));
            }
            "response.failed" | "error" => {
                return Err(AppError::BadRequest(format!(
                    "上游 Responses 流式错误: {}",
                    value
                        .get("message")
                        .or_else(|| value.pointer("/error/message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown")
                )));
            }
            _ => {}
        }
        Ok(out)
    }

    fn push_anthropic(&mut self, value: &Value) -> AppResult<Vec<String>> {
        let mut out = Vec::new();
        match value.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "message_start" => {
                if let Some(m) = value.get("message") {
                    if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                        self.id = id.to_string();
                    }
                    if let Some(model) = m.get("model").and_then(|m| m.as_str()) {
                        self.model = model.to_string();
                    }
                    self.input_tokens = m
                        .pointer("/usage/input_tokens")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    self.output_tokens = m
                        .pointer("/usage/output_tokens")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                }
            }
            "content_block_start" => {
                let block = value.get("content_block");
                if block.and_then(|b| b.get("type")).and_then(|t| t.as_str()) == Some("tool_use") {
                    let block_index = value.get("index").and_then(|i| i.as_i64()).unwrap_or(0);
                    let idx = self.next_tool_index;
                    self.next_tool_index += 1;
                    self.block_tool_index.insert(block_index, idx);
                    self.emit_delta(
                        &mut out,
                        json!({
                            "tool_calls": [{
                                "index": idx,
                                "id": block.and_then(|b| b.get("id")).cloned().unwrap_or(json!("")),
                                "type": "function",
                                "function": {
                                    "name": block.and_then(|b| b.get("name")).cloned().unwrap_or(json!("")),
                                    "arguments": "",
                                }
                            }]
                        }),
                    );
                }
            }
            "content_block_delta" => {
                let delta = value.get("delta");
                match delta.and_then(|d| d.get("type")).and_then(|t| t.as_str()) {
                    Some("text_delta") => {
                        if let Some(t) = delta.and_then(|d| d.get("text")).and_then(|t| t.as_str()) {
                            self.emit_delta(&mut out, json!({"content": t}));
                        }
                    }
                    Some("input_json_delta") => {
                        let block_index = value.get("index").and_then(|i| i.as_i64()).unwrap_or(0);
                        if let Some(idx) = self.block_tool_index.get(&block_index).copied() {
                            self.emit_delta(
                                &mut out,
                                json!({
                                    "tool_calls": [{
                                        "index": idx,
                                        "function": {"arguments": delta
                                            .and_then(|d| d.get("partial_json"))
                                            .cloned()
                                            .unwrap_or(json!(""))}
                                    }]
                                }),
                            );
                        }
                    }
                    _ => {}
                }
            }
            "message_delta" => {
                if let Some(reason) = value.pointer("/delta/stop_reason").and_then(|r| r.as_str()) {
                    self.finish_reason = Some(map_anthropic_stop(reason).to_string());
                }
                if let Some(n) = value.pointer("/usage/output_tokens").and_then(|v| v.as_i64()) {
                    self.output_tokens = n;
                }
            }
            "message_stop" => {
                let reason = self.finish_reason.clone().unwrap_or_else(|| {
                    if self.next_tool_index > 0 { "tool_calls".into() } else { "stop".into() }
                });
                out.extend(self.finish_events(&reason));
            }
            "error" => {
                return Err(AppError::BadRequest(format!(
                    "上游 Anthropic 流式错误: {}",
                    value.pointer("/error/message").and_then(|m| m.as_str()).unwrap_or("unknown")
                )));
            }
            // ping / content_block_stop 等无需下发。
            _ => {}
        }
        Ok(out)
    }

    fn absorb_responses_usage(&mut self, resp: &Value) {
        if let Some(n) = resp.pointer("/usage/input_tokens").and_then(|v| v.as_i64()) {
            self.input_tokens = n;
        }
        if let Some(n) = resp.pointer("/usage/output_tokens").and_then(|v| v.as_i64()) {
            self.output_tokens = n;
        }
    }

    /// 结束事件:finish chunk + usage chunk。
    fn finish_events(&mut self, reason: &str) -> Vec<String> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        let mut out = Vec::new();
        self.ensure_role(&mut out);
        out.push(self.delta_chunk_with_finish(json!({}), json!(reason)));
        out.push(self.usage_chunk());
        out
    }

    /// 首条 delta 前补一个 role 块(OpenAI 流式惯例)。
    fn ensure_role(&mut self, out: &mut Vec<String>) {
        if !self.role_sent {
            self.role_sent = true;
            out.push(self.delta_chunk_with_finish(json!({"role": "assistant", "content": ""}), Value::Null));
        }
    }

    /// 追加一条内容 delta(必要时先补 role 块)。
    fn emit_delta(&mut self, out: &mut Vec<String>, delta: Value) {
        self.ensure_role(out);
        out.push(self.delta_chunk_with_finish(delta, Value::Null));
    }

    fn delta_chunk_with_finish(&self, delta: Value, finish: Value) -> String {
        json!({
            "id": self.id,
            "object": "chat.completion.chunk",
            "created": self.created,
            "model": self.model,
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
        })
        .to_string()
    }

    fn usage_chunk(&self) -> String {
        json!({
            "id": self.id,
            "object": "chat.completion.chunk",
            "created": self.created,
            "model": self.model,
            "choices": [],
            "usage": {
                "prompt_tokens": self.input_tokens,
                "completion_tokens": self.output_tokens,
                "total_tokens": self.input_tokens + self.output_tokens,
            },
        })
        .to_string()
    }
}

fn map_anthropic_stop(reason: &str) -> &'static str {
    match reason {
        "max_tokens" => "length",
        "tool_use" => "tool_calls",
        _ => "stop",
    }
}

// ───────────────────────── 工具函数 ─────────────────────────

fn text_of(content: Option<&Value>) -> Option<String> {
    match content? {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join(""),
        ),
        _ => None,
    }
}

/// 只保留白名单内的键(上游对未知参数通常直接 400)。
fn prune(obj: &mut Map<String, Value>, allowed: &[&str]) {
    obj.retain(|k, _| allowed.contains(&k.as_str()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn chunk(v: &str) -> Value {
        serde_json::from_str(v).expect("chat chunk 应为合法 JSON")
    }

    #[test]
    fn resolve_prefers_setting_then_type() {
        assert_eq!(
            UpstreamProtocol::resolve_for(1, Some(&json!({"api_protocol": "anthropic"}))),
            UpstreamProtocol::Anthropic
        );
        assert_eq!(
            UpstreamProtocol::resolve_for(1, Some(&json!({"api_protocol": "openai_responses"}))),
            UpstreamProtocol::OpenAiResponses
        );
        // 未配置时按类型推断:14=Anthropic,其余=OpenAI Chat
        assert_eq!(UpstreamProtocol::resolve_for(14, None), UpstreamProtocol::Anthropic);
        assert_eq!(UpstreamProtocol::resolve_for(1, None), UpstreamProtocol::OpenAiChat);
        assert_eq!(
            UpstreamProtocol::resolve_for(43, Some(&json!({"balance": {}}))),
            UpstreamProtocol::OpenAiChat
        );
    }

    #[test]
    fn chat_to_responses_maps_system_and_input() {
        let chat = json!({
            "model": "gpt-4o", "stream": true, "max_tokens": 128, "temperature": 0.2,
            "messages": [
                {"role": "system", "content": "你是助手"},
                {"role": "user", "content": "你好"},
                {"role": "assistant", "content": "在"}
            ],
            "stream_options": {"include_usage": true},
            "frequency_penalty": 0.5
        });
        let out = chat_to_upstream(&chat, UpstreamProtocol::OpenAiResponses).unwrap();
        assert_eq!(out["model"], json!("gpt-4o"));
        assert_eq!(out["instructions"], json!("你是助手"));
        assert_eq!(out["max_output_tokens"], json!(128), "max_tokens 应改名为 max_output_tokens");
        assert_eq!(out["stream"], json!(true));
        assert_eq!(out["input"][0]["role"], json!("user"));
        assert_eq!(out["input"][0]["content"][0]["type"], json!("input_text"));
        assert_eq!(out["input"][1]["content"][0]["type"], json!("output_text"));
        assert!(out.get("stream_options").is_none(), "Responses 不认 stream_options");
        assert!(out.get("frequency_penalty").is_none(), "非白名单参数应丢弃");
    }

    #[test]
    fn chat_to_responses_maps_tools_and_tool_choice() {
        let chat = json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": "call_1", "type": "function", "function": {"name": "f", "arguments": "{\"a\":1}"}}
                ]},
                {"role": "tool", "tool_call_id": "call_1", "content": "42"}
            ],
            "tools": [{"type": "function", "function": {"name": "f", "description": "d", "parameters": {"type": "object"}}}],
            "tool_choice": {"type": "function", "function": {"name": "f"}}
        });
        let out = chat_to_upstream(&chat, UpstreamProtocol::OpenAiResponses).unwrap();
        assert_eq!(out["tools"][0]["type"], json!("function"));
        assert_eq!(out["tools"][0]["name"], json!("f"));
        assert_eq!(out["tool_choice"], json!({"type": "function", "name": "f"}));
        assert_eq!(out["input"][0]["type"], json!("function_call"));
        assert_eq!(out["input"][0]["call_id"], json!("call_1"));
        assert_eq!(out["input"][1]["type"], json!("function_call_output"));
        assert_eq!(out["input"][1]["output"], json!("42"));
    }

    #[test]
    fn responses_to_chat_maps_text_and_usage() {
        let resp = json!({
            "id": "resp_1", "object": "response", "status": "completed", "model": "gpt-4o",
            "output": [{"type": "message", "role": "assistant",
                        "content": [{"type": "output_text", "text": "你好"}]}],
            "usage": {"input_tokens": 3, "output_tokens": 5, "total_tokens": 8}
        });
        let out = upstream_to_chat(&resp, UpstreamProtocol::OpenAiResponses).unwrap();
        assert_eq!(out["choices"][0]["message"]["content"], json!("你好"));
        assert_eq!(out["choices"][0]["finish_reason"], json!("stop"));
        assert_eq!(out["usage"]["prompt_tokens"], json!(3));
        assert_eq!(out["usage"]["completion_tokens"], json!(5));
    }

    #[test]
    fn responses_to_chat_maps_function_call() {
        let resp = json!({
            "id": "resp_2", "status": "completed", "model": "gpt-4o",
            "output": [
                {"type": "function_call", "call_id": "call_9", "name": "f", "arguments": "{\"a\":1}"}
            ]
        });
        let out = upstream_to_chat(&resp, UpstreamProtocol::OpenAiResponses).unwrap();
        assert_eq!(out["choices"][0]["finish_reason"], json!("tool_calls"));
        assert_eq!(out["choices"][0]["message"]["tool_calls"][0]["id"], json!("call_9"));
        assert_eq!(out["choices"][0]["message"]["content"], Value::Null);
    }

    #[test]
    fn chat_to_anthropic_maps_system_stop_tools() {
        let chat = json!({
            "model": "claude-x", "max_tokens": 64, "stop": ["END"],
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{"type": "function", "function": {"name": "f", "parameters": {"type": "object"}}}],
            "tool_choice": "required",
            "presence_penalty": 1
        });
        let out = chat_to_upstream(&chat, UpstreamProtocol::Anthropic).unwrap();
        assert_eq!(out["stop_sequences"], json!(["END"]));
        assert_eq!(out["tools"][0]["name"], json!("f"));
        assert_eq!(out["tools"][0]["input_schema"], json!({"type": "object"}));
        assert_eq!(out["tool_choice"], json!({"type": "any"}));
        assert!(out.get("presence_penalty").is_none(), "Anthropic 不认该参数");
        assert!(out.get("stop").is_none(), "stop 应改名为 stop_sequences");
    }

    #[test]
    fn chat_to_anthropic_tool_roundtrip_and_alternation() {
        let chat = json!({
            "model": "claude-x",
            "messages": [
                {"role": "user", "content": "天气"},
                {"role": "assistant", "content": "查一下", "tool_calls": [
                    {"id": "toolu_1", "type": "function", "function": {"name": "get_weather", "arguments": "{\"city\":\"上海\"}"}}
                ]},
                {"role": "tool", "tool_call_id": "toolu_1", "content": "晴"},
                {"role": "user", "content": "谢谢"}
            ]
        });
        let out = chat_to_upstream(&chat, UpstreamProtocol::Anthropic).unwrap();
        let msgs = out["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], json!("user"));
        assert_eq!(msgs[1]["role"], json!("assistant"));
        let blocks = msgs[1]["content"].as_array().unwrap();
        assert!(blocks.iter().any(|b| b["type"] == json!("tool_use")), "tool_calls → tool_use");
        assert_eq!(blocks.iter().find(|b| b["type"] == json!("tool_use")).unwrap()["input"], json!({"city": "上海"}));
        // tool_result(user) 与其后的 user 消息合并,保证交替
        assert_eq!(msgs[2]["role"], json!("user"));
        let merged = msgs[2]["content"].as_array().unwrap();
        assert!(merged.iter().any(|b| b["type"] == json!("tool_result")), "tool → tool_result");
        assert_eq!(msgs.len(), 3, "相邻同角色消息应合并");
    }

    #[test]
    fn anthropic_to_chat_maps_text_tool_and_stop() {
        let body = json!({
            "id": "msg_1", "model": "claude-x", "stop_reason": "max_tokens",
            "content": [
                {"type": "text", "text": "部分"},
                {"type": "tool_use", "id": "toolu_1", "name": "f", "input": {"a": 1}}
            ],
            "usage": {"input_tokens": 7, "output_tokens": 2}
        });
        let out = upstream_to_chat(&body, UpstreamProtocol::Anthropic).unwrap();
        assert_eq!(out["choices"][0]["message"]["content"], json!("部分"));
        assert_eq!(out["choices"][0]["finish_reason"], json!("length"));
        let call = &out["choices"][0]["message"]["tool_calls"][0];
        assert_eq!(call["id"], json!("toolu_1"));
        assert_eq!(call["function"]["arguments"], json!("{\"a\":1}"));
        assert_eq!(out["usage"]["prompt_tokens"], json!(7));
        assert_eq!(out["usage"]["completion_tokens"], json!(2));
    }

    #[test]
    fn responses_stream_emits_text_finish_and_usage() {
        let mut n = StreamNormalizer::new(UpstreamProtocol::OpenAiResponses, "gpt-4o");
        let mut events: Vec<String> = Vec::new();
        events.extend(n.push(r#"{"type":"response.created","response":{"id":"resp_1","model":"gpt-4o"}}"#).unwrap());
        events.extend(n.push(r#"{"type":"response.output_text.delta","delta":"你"}"#).unwrap());
        events.extend(n.push(r#"{"type":"response.output_text.delta","delta":"好"}"#).unwrap());
        events.extend(n.push(r#"{"type":"response.completed","response":{"usage":{"input_tokens":2,"output_tokens":3}}}"#).unwrap());
        let text: String = events
            .iter()
            .filter_map(|e| chunk(e).pointer("/choices/0/delta/content").and_then(|c| c.as_str()).map(str::to_string))
            .collect();
        assert_eq!(text, "你好");
        let finish = events
            .iter()
            .find_map(|e| chunk(e).pointer("/choices/0/finish_reason").and_then(|f| f.as_str()).map(str::to_string));
        assert_eq!(finish.as_deref(), Some("stop"));
        let usage = events
            .iter()
            .find_map(|e| chunk(e).get("usage").filter(|u| !u.is_null()).cloned());
        assert_eq!(usage.unwrap()["completion_tokens"], json!(3));
        assert_eq!(n.finish(), vec!["[DONE]".to_string()], "收尾只补 [DONE]");
    }

    #[test]
    fn anthropic_stream_emits_text_tool_and_usage() {
        let mut n = StreamNormalizer::new(UpstreamProtocol::Anthropic, "claude-x");
        let mut events: Vec<String> = Vec::new();
        events.extend(n.push(r#"{"type":"message_start","message":{"id":"msg_1","model":"claude-x","usage":{"input_tokens":5,"output_tokens":0}}}"#).unwrap());
        events.extend(n.push(r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#).unwrap());
        events.extend(n.push(r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"嗨"}}"#).unwrap());
        events.extend(n.push(r#"{"type":"content_block_stop","index":0}"#).unwrap());
        events.extend(n.push(r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"f"}}"#).unwrap());
        events.extend(n.push(r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"a\":"}}"#).unwrap());
        events.extend(n.push(r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"1}"}}"#).unwrap());
        events.extend(n.push(r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":9}}"#).unwrap());
        events.extend(n.push(r#"{"type":"message_stop"}"#).unwrap());

        let text: String = events
            .iter()
            .filter_map(|e| chunk(e).pointer("/choices/0/delta/content").and_then(|c| c.as_str()).map(str::to_string))
            .collect();
        assert_eq!(text, "嗨");
        let call = events
            .iter()
            .filter_map(|e| chunk(e).pointer("/choices/0/delta/tool_calls/0").cloned())
            .collect::<Vec<_>>();
        assert_eq!(call[0]["id"], json!("toolu_1"));
        assert_eq!(call[0]["function"]["name"], json!("f"));
        let args: String = call
            .iter()
            .filter_map(|c| c.pointer("/function/arguments").and_then(|a| a.as_str()))
            .collect();
        assert_eq!(args, "{\"a\":1}", "分段 partial_json 应拼接为完整 arguments");
        let finish = events.iter().find_map(|e| {
            chunk(e).pointer("/choices/0/finish_reason").and_then(|f| f.as_str()).map(str::to_string)
        });
        assert_eq!(finish.as_deref(), Some("tool_calls"));
        let usage = events.iter().find_map(|e| chunk(e).get("usage").filter(|u| !u.is_null()).cloned());
        let usage = usage.unwrap();
        assert_eq!(usage["prompt_tokens"], json!(5));
        assert_eq!(usage["completion_tokens"], json!(9));
        assert_eq!(n.finish(), vec!["[DONE]".to_string()]);
    }

    #[test]
    fn stream_ignores_non_json_and_ping() {
        let mut n = StreamNormalizer::new(UpstreamProtocol::Anthropic, "claude-x");
        assert!(n.push("not-json").unwrap().is_empty());
        assert!(n.push(r#"{"type":"ping"}"#).unwrap().is_empty());
    }
}
