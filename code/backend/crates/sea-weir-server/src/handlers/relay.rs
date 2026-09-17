//! 中继面入口。对应 C4 组件 `relay_entry`。
//!
//! 入口:OpenAI `POST /v1/chat/completions`、Claude `POST /v1/messages`。
//! 两者共用 [`run_relay`]:认证 → 协议转换(入口格式 → OpenAI canonical,再按渠道的
//! 上游协议 `setting.api_protocol` 转成 OpenAI Chat / OpenAI Responses / Anthropic)
//! → 重试循环选路 → 扣费 + 消费日志 → 响应转回入口格式。
//!
//! 待补:core::relay::pipeline 的预扣/结算状态机、MJ/任务类入口。

use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;

use sea_weir_core::relay::convert::{convert_request, convert_response, RequestConversionChain};
use sea_weir_core::relay::protocol::{self, StreamNormalizer, UpstreamProtocol};
use sea_weir_core::relay::{autoban, billing, select};
use sea_weir_types::constants::{status as ch_status, LogType};
use sea_weir_types::domain::{Ability, Channel, Log};
use sea_weir_types::dto::{RelayRequest, Usage};
use sea_weir_types::{AppError, NewApiError, RelayFormat};

use crate::app_state::ServerState;
use crate::middleware::auth::{AuthToken, TokenAuth};

/// 上游响应体字节流(已统一为 canonical chat SSE,或 OpenAI Chat 的原样透传)。
type ByteStream =
    std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send>>;

fn relay_fail(e: AppError, format: RelayFormat) -> Response {
    crate::response::relay_err(e.into(), format)
}

/// 拼接上游 URL:base_url 可能已含 `/v1` 或完整前缀。
fn upstream_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}{}", path.trim_start_matches("/v1"))
    } else {
        format!("{base}{path}")
    }
}

/// 一次上游调用的结果。
enum UpstreamOutcome {
    Stream(reqwest::Response),
    Json(serde_json::Value),
}

/// 按点分路径设置值(中间缺失则创建对象;路径含数组下标不支持)。
fn set_path(root: &mut serde_json::Value, path: &str, value: serde_json::Value) {
    let parts: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return;
    }
    let mut current = root;
    for part in &parts[..parts.len() - 1] {
        if !current.is_object() {
            *current = serde_json::json!({});
        }
        let entry = current
            .as_object_mut()
            .expect("上面已保证是 object")
            .entry((*part).to_string())
            .or_insert_with(|| serde_json::json!({}));
        // 中间层若是标量,覆盖为对象以保证路径可写。
        if !entry.is_object() {
            *entry = serde_json::json!({});
        }
        current = entry;
    }
    if let Some(obj) = current.as_object_mut() {
        obj.insert(parts[parts.len() - 1].to_string(), value);
    }
}

/// 按点分路径删除值(不存在则忽略)。
fn delete_path(root: &mut serde_json::Value, path: &str) {
    let parts: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return;
    }
    let mut current = root;
    for part in &parts[..parts.len() - 1] {
        match current.get_mut(part) {
            Some(next) => current = next,
            None => return,
        }
    }
    if let Some(obj) = current.as_object_mut() {
        obj.remove(parts[parts.len() - 1]);
    }
}

/// 应用渠道级请求参数改写。
///
/// 支持两种形态:
/// - 新式:`{"operations":[{"mode":"set"|"delete","path":"max_tokens","value":123}]}`
/// - 简式:`{"max_tokens": 123, "stream": null}`(null 表示删除该字段)
pub fn apply_param_override(body: &mut serde_json::Value, override_: &serde_json::Value) {
    if let Some(ops) = override_.get("operations").and_then(|v| v.as_array()) {
        for op in ops {
            let path = op.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if path.is_empty() {
                continue;
            }
            match op.get("mode").and_then(|m| m.as_str()).unwrap_or("set") {
                "delete" => delete_path(body, path),
                _ => {
                    if let Some(value) = op.get("value") {
                        if !value.is_null() {
                            set_path(body, path, value.clone());
                        }
                    }
                }
            }
        }
        return;
    }

    if let Some(map) = override_.as_object() {
        for (path, value) in map {
            if value.is_null() {
                delete_path(body, path);
            } else {
                set_path(body, path, value.clone());
            }
        }
    }
}

/// 应用渠道级 header 改写;值中的 `{api_key}` 占位替换为渠道密钥。
fn apply_header_override(
    request: reqwest::RequestBuilder,
    header_override: Option<&serde_json::Value>,
    api_key: &str,
) -> reqwest::RequestBuilder {
    let Some(map) = header_override.and_then(|v| v.as_object()) else {
        return request;
    };
    let mut request = request;
    for (name, value) in map {
        if let Some(text) = value.as_str() {
            request = request.header(name.as_str(), text.replace("{api_key}", api_key));
        }
    }
    request
}

/// 发起一次上游调用。错误统一归一化为 `NewApiError`(供重试/自动禁用判定)。
///
/// 鉴权按上游协议选择:Anthropic 用 `x-api-key` + `anthropic-version`,其余用
/// `Authorization: Bearer`。
async fn forward_once(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    body: &serde_json::Value,
    is_stream: bool,
    header_override: Option<&serde_json::Value>,
    protocol: UpstreamProtocol,
) -> Result<UpstreamOutcome, NewApiError> {
    let mut request = http
        .post(url)
        .header(header::CONTENT_TYPE, "application/json");
    request = if protocol.uses_anthropic_auth() {
        request
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
    } else {
        request.header(header::AUTHORIZATION, format!("Bearer {key}"))
    };
    let request = apply_header_override(request, header_override, key).json(body);

    let resp = request
        .send()
        .await
        .map_err(|e| channel_err("channel:request_failed", &format!("上游请求失败: {e}"), 502))?;

    let status = resp.status();
    if !status.is_success() {
        let code = if status == StatusCode::UNAUTHORIZED {
            "channel:invalid_key"
        } else {
            ""
        };
        let text = resp.text().await.unwrap_or_default();
        return Err(NewApiError {
            status_code: status.as_u16(),
            error_code: code.into(),
            error_type: "new_api_error".into(),
            message: text.chars().take(500).collect(),
            local_error: false,
            skip_retry: false,
            record_error_log: true,
        });
    }

    if is_stream {
        return Ok(UpstreamOutcome::Stream(resp));
    }
    resp.json::<serde_json::Value>()
        .await
        .map(UpstreamOutcome::Json)
        .map_err(|e| channel_err("channel:parse_failed", &format!("解析上游响应失败: {e}"), 502))
}

fn channel_err(code: &str, message: &str, status_code: u16) -> NewApiError {
    NewApiError {
        status_code,
        error_code: code.into(),
        error_type: "new_api_error".into(),
        message: message.into(),
        local_error: false,
        skip_retry: false,
        record_error_log: true,
    }
}

/// OpenAI Chat 上游:SSE 原样透传。
fn passthrough_stream(resp: reqwest::Response) -> ByteStream {
    Box::pin(resp.bytes_stream().map(|r| {
        r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }))
}

/// 非 OpenAI Chat 上游:把上游 SSE 归一化成 canonical chat SSE 再下发。
///
/// 上游自己的 `[DONE]` 会被丢弃,由 [`StreamNormalizer::finish`] 统一补发,避免重复。
fn normalize_stream(resp: reqwest::Response, protocol: UpstreamProtocol, model: String) -> ByteStream {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(16);
    tokio::spawn(async move {
        let mut norm = StreamNormalizer::new(protocol, &model);
        let mut buf = String::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let Ok(bytes) = chunk else { break };
            buf.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].to_string();
                buf.drain(..=pos);
                let line = line.trim();
                let Some(payload) = line.strip_prefix("data:") else {
                    continue; // 忽略 event:/注释行
                };
                let payload = payload.trim();
                if payload.is_empty() || payload == "[DONE]" {
                    continue;
                }
                match norm.push(payload) {
                    Ok(items) => {
                        for item in items {
                            if tx.send(Ok(sse_data(&item))).await.is_err() {
                                return; // 客户端断开
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                e.to_string(),
                            )))
                            .await;
                        return;
                    }
                }
            }
        }
        for item in norm.finish() {
            if tx.send(Ok(sse_data(&item))).await.is_err() {
                return;
            }
        }
    });
    Box::pin(futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    }))
}

fn sse_data(payload: &str) -> bytes::Bytes {
    bytes::Bytes::from(format!("data: {payload}\n\n"))
}

fn usage_from_body(body: &serde_json::Value) -> Usage {
    let usage = body.get("usage");
    let get = |key: &str| {
        usage
            .and_then(|u| u.get(key))
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    };
    Usage {
        prompt_tokens: get("prompt_tokens"),
        completion_tokens: get("completion_tokens"),
        total_tokens: get("total_tokens"),
        cached_tokens: usage
            .and_then(|u| u.pointer("/prompt_tokens_details/cached_tokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        ..Usage::default()
    }
}

/// 本次尝试的选路:在「尚未尝试过」的候选里取当前最高优先级档,档内加权随机。
///
/// 排除已失败渠道(而非固定取第 N 档),以正确处理自动禁用导致的候选集缩小;
/// 排除后为空时回落到全部候选,允许唯一渠道的瞬时错误重试。
fn pick_channel<'a>(
    candidates: &'a [Channel],
    auth: &AuthToken,
    origin_model: &str,
    tried: &std::collections::HashSet<i64>,
) -> Option<&'a Channel> {
    let available: Vec<&Channel> = candidates
        .iter()
        .filter(|c| !tried.contains(&c.id))
        .collect();
    let available: Vec<&Channel> = if available.is_empty() {
        candidates.iter().collect()
    } else {
        available
    };
    if let Some(id) = auth.specific_channel_id {
        return available.iter().find(|c| c.id == id).copied();
    }
    let abilities: Vec<Ability> = available
        .iter()
        .map(|c| Ability {
            group: auth.group.clone(),
            model: origin_model.to_string(),
            channel_id: c.id,
            enabled: true,
            priority: c.priority,
            weight: c.weight,
            tag: c.tag.clone(),
        })
        .collect();
    let tiers = select::priority_tiers(&abilities);
    tiers
        .first()
        .and_then(|tier| select::weighted_pick(tier))
        .map(|a| a.channel_id)
        .and_then(|id| available.iter().find(|c| c.id == id).copied())
}

/// `POST /v1/chat/completions`(OpenAI 入口)。
pub async fn chat_completions(
    State(state): State<Arc<ServerState>>,
    TokenAuth(auth): TokenAuth,
    _headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Response {
    run_relay(state, auth, RelayFormat::OpenAi, body).await
}

/// `POST /v1/messages/count_tokens`(TokenAuth):估算输入 token 数。
///
/// Anthropic 官方的 token 计数端点,Claude Code / 各类 Anthropic 客户端在**连接检查**
/// 与会话上下文预算时会调用它。此前未实现 → 落到 501 fallback,客户端显示
/// "endpoint not implemented yet",看起来像"连不上"。
/// 网关不做真实分词(Claude 分词器未公开),返回近似估计,不参与计费。
pub async fn claude_count_tokens(
    TokenAuth(_auth): TokenAuth,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let input_tokens = sea_weir_core::relay::estimate::estimate_claude_input_tokens(&body);
    (StatusCode::OK, Json(serde_json::json!({ "input_tokens": input_tokens }))).into_response()
}

/// `POST /v1/messages`(Claude Messages 入口)。
pub async fn claude_messages(
    State(state): State<Arc<ServerState>>,
    TokenAuth(auth): TokenAuth,
    _headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Response {
    run_relay(state, auth, RelayFormat::Claude, body).await
}

/// 中继主流程。`entry` 为入口协议格式;上游统一按 OpenAI 兼容协议调用。
async fn run_relay(
    state: Arc<ServerState>,
    auth: AuthToken,
    entry: RelayFormat,
    body: serde_json::Value,
) -> Response {
    let origin_model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if origin_model.is_empty() {
        return relay_fail(AppError::BadRequest("缺少 model".into()), entry);
    }
    let is_stream = body.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);

    let request_id = uuid::Uuid::new_v4().to_string();
    let started = Instant::now();

    // 定价视图(进程内 60s 缓存):按模型/分组合成倍率。
    let price = state
        .pricing
        .get(state.options.as_ref().map(|o| o.as_ref()))
        .await
        .price_for(&origin_model, &auth.group);

    // 协议转换:入口格式 → OpenAI(上游为 OpenAI 兼容)。
    let mut chain = RequestConversionChain::default();
    let relay_req = RelayRequest {
        model: origin_model.clone(),
        stream: is_stream,
        raw: body,
    };
    let mut upstream_body = match convert_request(
        &relay_req,
        entry,
        RelayFormat::OpenAi,
        &mut chain,
    ) {
        Ok(v) => v,
        Err(e) => return relay_fail(e, entry),
    };
    // 非 OpenAI 入口的流式:要求上游带上 usage(便于结算)。
    if is_stream && entry != RelayFormat::OpenAi {
        if let Some(obj) = upstream_body.as_object_mut() {
            obj.insert(
                "stream_options".into(),
                serde_json::json!({ "include_usage": true }),
            );
        }
    }

    let channels = match state.channels.as_ref() {
        Some(repo) => repo.clone(),
        None => return relay_fail(AppError::Database("数据库未连接".into()), entry),
    };

    let retry_times = state.config.relay.retry_times;
    let mut last_err: Option<NewApiError> = None;
    let mut tried = std::collections::HashSet::new();

    for attempt in 0..=retry_times {
        let candidates = match channels.list_candidates(&auth.group, &origin_model).await {
            Ok(c) => c,
            Err(e) => return relay_fail(e, entry),
        };
        let Some(channel) = pick_channel(&candidates, &auth, &origin_model, &tried) else {
            break;
        };

        let keys = channel.keys();
        if keys.is_empty() {
            last_err = Some(channel_err("channel:no_key", "渠道未配置密钥", 502));
            break;
        }
        let idx = if keys.len() == 1 {
            0
        } else {
            rand::Rng::gen_range(&mut rand::thread_rng(), 0..keys.len())
        };
        let channel_key = keys[idx].to_string();

        let Some(base_url) = channel.base_url.as_deref().filter(|s| !s.trim().is_empty()) else {
            last_err = Some(channel_err("channel:no_base_url", "渠道未配置 base_url", 502));
            break;
        };
        let upstream_model = channel
            .model_mapping
            .as_ref()
            .and_then(|m| m.get(&origin_model))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| origin_model.clone());
        if let Some(obj) = upstream_body.as_object_mut() {
            obj.insert("model".into(), serde_json::json!(upstream_model));
        }

        // 渠道的上游协议(可逐渠道不同):canonical chat → 上游协议;
        // param_override 作用在**上游**请求体上,便于覆盖各协议的原生参数。
        let protocol = UpstreamProtocol::resolve(channel);
        let mut attempt_body = match protocol::chat_to_upstream(&upstream_body, protocol) {
            Ok(v) => v,
            Err(e) => return relay_fail(e, entry),
        };
        if let Some(over) = channel.param_override.as_ref() {
            apply_param_override(&mut attempt_body, over);
        }

        let url = upstream_url(base_url, protocol.path());
        match forward_once(
            &state.http,
            &url,
            &channel_key,
            &attempt_body,
            is_stream,
            channel.header_override.as_ref(),
            protocol,
        )
        .await
        {
            Ok(UpstreamOutcome::Stream(resp)) => {
                let stream = if protocol == UpstreamProtocol::OpenAiChat {
                    passthrough_stream(resp)
                } else {
                    normalize_stream(resp, protocol, origin_model.clone())
                };
                if entry == RelayFormat::Claude {
                    return claude_stream_response(
                        state.clone(),
                        auth.clone(),
                        channel.id,
                        origin_model.clone(),
                        request_id.clone(),
                        stream,
                        price.clone(),
                    );
                }
                return stream_response(
                    state.clone(),
                    auth.clone(),
                    channel.id,
                    origin_model.clone(),
                    request_id.clone(),
                    stream,
                    price.clone(),
                );
            }
            Ok(UpstreamOutcome::Json(upstream_resp)) => {
                // 上游响应 → canonical chat,再做计费与入口格式转换。
                let canonical = match protocol::upstream_to_chat(&upstream_resp, protocol) {
                    Ok(v) => v,
                    Err(e) => return relay_fail(e, entry),
                };
                let usage = usage_from_body(&canonical);
                let quota = billing::calculate_quota(&usage, &price);
                let use_time = started.elapsed().as_secs() as i32;
                if let Err(e) = charge(&state, &auth, quota).await {
                    return relay_fail(e, entry);
                }
                log_consume(
                    &state,
                    &auth,
                    channel.id,
                    &origin_model,
                    &usage,
                    quota,
                    use_time,
                    is_stream,
                    &request_id,
                )
                .await;

                let out = match convert_response(&canonical, RelayFormat::OpenAi, entry) {
                    Ok(v) => v,
                    Err(e) => return relay_fail(e, entry),
                };
                return (StatusCode::OK, Json(out)).into_response();
            }
            Err(err) => {
                if autoban::should_disable(&err, channel.auto_ban != 0) {
                    let _ = channels
                        .update_status(channel.id, ch_status::AUTO_DISABLED, &err.message)
                        .await;
                }
                tried.insert(channel.id);
                let ctx = autoban::RetryContext {
                    retry_times_left: retry_times.saturating_sub(attempt),
                    has_specific_channel: auth.specific_channel_id.is_some(),
                    affinity_skip_retry: false,
                };
                if !autoban::should_retry(&err, &ctx) {
                    return crate::response::relay_err(err, entry);
                }
                last_err = Some(err);
            }
        }
    }

    crate::response::relay_err(
        last_err.unwrap_or_else(|| NewApiError::from(AppError::NotFound("无可用渠道".into()))),
        entry,
    )
}

/// 流式响应(仅 OpenAI 入口):透传 SSE 字节,同时扫描 `usage`;流结束后扣费 + 记账。
///
/// 客户端中途断开时已消费部分仍结算(与 SEQ-004 一致)。
fn stream_response(
    state: Arc<ServerState>,
    auth: AuthToken,
    channel_id: i64,
    model: String,
    request_id: String,
    stream: ByteStream,
    price: billing::PriceData,
) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(16);
    let started = Instant::now();

    tokio::spawn(async move {
        let mut usage = Usage::default();
        let mut line_buf = String::new();
        let mut stream = stream;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    scan_sse_usage(&mut line_buf, &bytes, &mut usage);
                    if tx.send(Ok(bytes)).await.is_err() {
                        break; // 客户端断开
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            e.to_string(),
                        )))
                        .await;
                    break;
                }
            }
        }
        let quota = billing::calculate_quota(&usage, &price);
        if let Err(e) = charge(&state, &auth, quota).await {
            tracing::warn!(error = %e, request_id, "流式扣费失败");
            return;
        }
        log_consume(
            &state,
            &auth,
            channel_id,
            &model,
            &usage,
            quota,
            started.elapsed().as_secs() as i32,
            true,
            &request_id,
        )
        .await;
    });

    let body_stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(body_stream))
        .unwrap_or_else(|_| relay_fail(AppError::Internal("构造流式响应失败".into()), RelayFormat::OpenAi))
}

/// 构造一行 Claude SSE 事件。
fn claude_sse(event: &str, data: serde_json::Value) -> bytes::Bytes {
    bytes::Bytes::from(format!("event: {event}\ndata: {data}\n\n"))
}

/// Claude 入口的流式响应:把上游 OpenAI SSE 实时改写为 Claude 事件序列。
///
/// 事件顺序:`message_start` → `content_block_start` → `content_block_delta`* →
/// `content_block_stop` → `message_delta`(带 usage)→ `message_stop`。
/// 计费与日志在流结束后按累计 usage 结算(与 OpenAI 流式一致)。
#[allow(clippy::too_many_arguments)]
fn claude_stream_response(
    state: Arc<ServerState>,
    auth: AuthToken,
    channel_id: i64,
    model: String,
    request_id: String,
    stream: ByteStream,
    price: billing::PriceData,
) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(16);
    let started = Instant::now();

    tokio::spawn(async move {
        let mut usage = Usage::default();
        let mut buf = String::new();
        let mut block_started = false;
        let mut closed = false;
        let mut stream = stream;

        while let Some(chunk) = stream.next().await {
            let Ok(bytes) = chunk else { break };
            buf.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].to_string();
                buf.drain(..=pos);
                let line = line.trim();
                let Some(payload) = line.strip_prefix("data:") else {
                    continue;
                };
                let payload = payload.trim();
                if payload.is_empty() {
                    continue;
                }
                if payload == "[DONE]" {
                    if block_started && !closed {
                        closed = true;
                        let _ = tx
                            .send(Ok(claude_sse(
                                "message_delta",
                                serde_json::json!({
                                    "type": "message_delta",
                                    "delta": {"stop_reason": "end_turn", "stop_sequence": null},
                                    "usage": {"output_tokens": usage.completion_tokens}
                                }),
                            )))
                            .await;
                        let _ = tx
                            .send(Ok(claude_sse(
                                "message_stop",
                                serde_json::json!({"type": "message_stop"}),
                            )))
                            .await;
                    }
                    continue;
                }

                let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
                    continue;
                };
                if value.get("usage").map(|u| !u.is_null()).unwrap_or(false) {
                    usage = usage_from_body(&value);
                }
                let text = value
                    .pointer("/choices/0/delta/content")
                    .and_then(|c| c.as_str());
                let finished = value
                    .pointer("/choices/0/finish_reason")
                    .map(|f| !f.is_null())
                    .unwrap_or(false);

                if !block_started {
                    block_started = true;
                    let _ = tx
                        .send(Ok(claude_sse(
                            "message_start",
                            serde_json::json!({
                                "type": "message_start",
                                "message": {
                                    "id": request_id, "type": "message", "role": "assistant",
                                    "model": model, "content": [], "stop_reason": null,
                                    "stop_sequence": null,
                                    "usage": {"input_tokens": usage.prompt_tokens, "output_tokens": 0}
                                }
                            }),
                        )))
                        .await;
                    let _ = tx
                        .send(Ok(claude_sse(
                            "content_block_start",
                            serde_json::json!({
                                "type": "content_block_start", "index": 0,
                                "content_block": {"type": "text", "text": ""}
                            }),
                        )))
                        .await;
                }

                if let Some(text) = text {
                    if !text.is_empty() {
                        let _ = tx
                            .send(Ok(claude_sse(
                                "content_block_delta",
                                serde_json::json!({
                                    "type": "content_block_delta", "index": 0,
                                    "delta": {"type": "text_delta", "text": text}
                                }),
                            )))
                            .await;
                    }
                }
                if finished {
                    let _ = tx
                        .send(Ok(claude_sse(
                            "content_block_stop",
                            serde_json::json!({"type": "content_block_stop", "index": 0}),
                        )))
                        .await;
                }
            }
        }

        let quota = billing::calculate_quota(&usage, &price);
        if let Err(e) = charge(&state, &auth, quota).await {
            tracing::warn!(error = %e, request_id, "Claude 流式扣费失败");
            return;
        }
        log_consume(
            &state,
            &auth,
            channel_id,
            &model,
            &usage,
            quota,
            started.elapsed().as_secs() as i32,
            true,
            &request_id,
        )
        .await;
    });

    let body_stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(body_stream))
        .unwrap_or_else(|_| {
            relay_fail(
                AppError::Internal("构造流式响应失败".into()),
                RelayFormat::Claude,
            )
        })
}

/// 从 SSE 字节里扫描 `usage`(OpenAI 流式在 `stream_options.include_usage` 下
/// 于末尾 chunk 带 `usage`;无 usage 则不计费)。
fn scan_sse_usage(line_buf: &mut String, chunk: &[u8], usage: &mut Usage) {
    line_buf.push_str(&String::from_utf8_lossy(chunk));
    while let Some(pos) = line_buf.find('\n') {
        let line = line_buf[..pos].to_string();
        line_buf.drain(..=pos);
        let line = line.trim();
        let Some(payload) = line.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) {
            if value.get("usage").map(|u| !u.is_null()).unwrap_or(false) {
                *usage = usage_from_body(&value);
            }
        }
    }
}

/// 按上游 usage 扣费:用户钱包与令牌额度各扣一次(token 无限额度时跳过)。
///
/// NOTE(TDD): 最小实现(无预扣/退款);三段式(预扣→结算)在 pipeline 落地后替换。
/// 结算:用户额度**不参与拦截**(内部使用——用户不需要额度,0 额度也能用),
/// 只累计 `used_quota / request_count` 作统计口径;
/// 令牌额度**保留为可选限制**:令牌非「无限额度」时按条件扣减,不足则拒绝。
///
/// 渠道侧的“额度”指上游账户余额,由渠道余额探测维护(见 `channel::update_balance`),
/// 不在此处拦截。
async fn charge(state: &ServerState, auth: &AuthToken, quota: i64) -> Result<(), AppError> {
    if quota <= 0 {
        return Ok(());
    }
    let tokens = state
        .tokens
        .as_ref()
        .ok_or_else(|| AppError::Database("数据库未连接".into()))?;

    // 令牌无限额度时 try_decrease_quota 直接返回 true(不写库)。
    if !tokens.try_decrease_quota(auth.token_id, quota).await? {
        return Err(AppError::QuotaExceeded);
    }

    // 统计口径(不拦截请求):累计用户已用额度与请求数,失败仅告警。
    if let Some(users) = state.users.as_ref() {
        if let Err(e) = users.accumulate_usage(auth.user_id, quota, 1).await {
            tracing::warn!(error = %e, "累计用户用量失败(不影响请求)");
        }
    }
    Ok(())
}

/// 写消费日志(失败仅告警,不影响响应)。
#[allow(clippy::too_many_arguments)]
async fn log_consume(
    state: &ServerState,
    auth: &AuthToken,
    channel_id: i64,
    model: &str,
    usage: &Usage,
    quota: i64,
    use_time: i32,
    is_stream: bool,
    request_id: &str,
) {
    let Some(logs) = state.logs.as_ref() else {
        return;
    };
    let log = Log {
        id: 0,
        user_id: auth.user_id,
        created_at: 0,
        r#type: LogType::Consume as i32,
        content: String::new(),
        username: auth.username.clone(),
        token_name: auth.token_name.clone(),
        model_name: model.to_string(),
        quota,
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        use_time,
        is_stream,
        channel_id: Some(channel_id),
        token_id: Some(auth.token_id),
        group: Some(auth.group.clone()),
        ip: None,
        request_id: Some(request_id.to_string()),
        other: None,
    };
    if let Err(e) = logs.record(&log).await {
        tracing::warn!(error = %e, request_id, "写消费日志失败");
    }
}

/// `GET /v1/models`(TokenAuth):令牌分组下可用的模型列表,OpenAI 原生格式。
pub async fn list_models(
    State(state): State<Arc<ServerState>>,
    TokenAuth(auth): TokenAuth,
) -> Response {
    let Some(channels) = state.channels.as_ref() else {
        return relay_fail(AppError::Database("数据库未连接".into()), RelayFormat::OpenAi);
    };
    match channels.list_models_by_group(&auth.group).await {
        Ok(models) => {
            let data: Vec<serde_json::Value> = models
                .iter()
                .map(|m| {
                    serde_json::json!({"id": m, "object": "model", "owned_by": "sea-weir"})
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!({"object": "list", "data": data})))
                .into_response()
        }
        Err(e) => relay_fail(e, RelayFormat::OpenAi),
    }
}

/// `GET /v1/models/:model`(TokenAuth):单个模型信息;分组下不可用则 404。
pub async fn get_model(
    State(state): State<Arc<ServerState>>,
    TokenAuth(auth): TokenAuth,
    axum::extract::Path(model): axum::extract::Path<String>,
) -> Response {
    let Some(channels) = state.channels.as_ref() else {
        return relay_fail(AppError::Database("数据库未连接".into()), RelayFormat::OpenAi);
    };
    match channels.list_models_by_group(&auth.group).await {
        Ok(models) if models.iter().any(|m| m == &model) => (
            StatusCode::OK,
            Json(serde_json::json!({"id": model, "object": "model", "owned_by": "sea-weir"})),
        )
            .into_response(),
        Ok(_) => crate::response::relay_err(
            NewApiError::from(AppError::NotFound(format!("模型 {model} 不可用"))),
            RelayFormat::OpenAi,
        ),
        Err(e) => relay_fail(e, RelayFormat::OpenAi),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn param_override_plain_set_and_delete() {
        let mut body = json!({"model": "gpt-4o", "stream": false, "extra": 1});
        apply_param_override(&mut body, &json!({"max_tokens": 4096, "stream": null}));
        assert_eq!(body["max_tokens"], json!(4096));
        assert!(body.get("stream").is_none(), "null 表示删除字段");
        assert_eq!(body["extra"], json!(1), "未涉及的字段保持不变");
    }

    #[test]
    fn param_override_nested_path() {
        let mut body = json!({"a": {"b": 1}});
        apply_param_override(&mut body, &json!({"a.b.c": "x"}));
        assert_eq!(body["a"]["b"]["c"], json!("x"));
        assert_eq!(body["a"]["b"] , json!({"c": "x"}), "嵌套覆盖中间层为对象");
    }

    #[test]
    fn param_override_operations_form() {
        let mut body = json!({"temperature": 1, "top_p": 1});
        apply_param_override(
            &mut body,
            &json!({"operations": [
                {"mode": "set", "path": "temperature", "value": 0.2},
                {"mode": "delete", "path": "top_p"}
            ]}),
        );
        assert_eq!(body["temperature"], json!(0.2));
        assert!(body.get("top_p").is_none());
    }

    #[test]
    fn param_override_ignores_non_object() {
        let mut body = json!({"model": "gpt-4o"});
        apply_param_override(&mut body, &json!("not-an-object"));
        assert_eq!(body, json!({"model": "gpt-4o"}));
    }

    #[test]
    fn upstream_url_joins_each_protocol_path() {
        assert_eq!(
            upstream_url("https://api.deepseek.com", UpstreamProtocol::OpenAiChat.path()),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            upstream_url("https://api.openai.com/v1", UpstreamProtocol::OpenAiResponses.path()),
            "https://api.openai.com/v1/responses",
            "base_url 已含 /v1 时不应重复"
        );
        assert_eq!(
            upstream_url("https://api.anthropic.com", UpstreamProtocol::Anthropic.path()),
            "https://api.anthropic.com/v1/messages"
        );
    }

    /// Anthropic 上游:必须用 `x-api-key` + `anthropic-version`,而不是 Bearer。
    #[tokio::test]
    async fn forward_once_uses_protocol_specific_auth() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "sk-a"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer sk-o"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .expect(1)
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let body = json!({"model": "m"});
        let url = upstream_url(&server.uri(), UpstreamProtocol::Anthropic.path());
        let out = forward_once(&http, &url, "sk-a", &body, false, None, UpstreamProtocol::Anthropic).await;
        assert!(matches!(out, Ok(UpstreamOutcome::Json(_))), "Anthropic 调用应成功");

        let url = upstream_url(&server.uri(), UpstreamProtocol::OpenAiChat.path());
        let out = forward_once(&http, &url, "sk-o", &body, false, None, UpstreamProtocol::OpenAiChat).await;
        assert!(matches!(out, Ok(UpstreamOutcome::Json(_))), "OpenAI 调用应成功");
        // wiremock 的 expect(1) 在 drop 时校验,未命中会 panic。
    }

    /// Responses 上游流 → canonical chat SSE(含 usage 与 [DONE])。
    #[tokio::test]
    async fn normalize_responses_stream_to_chat_sse() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let sse = concat!(
            "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\",\"model\":\"gpt-4o\"}}\n\n",
            "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"你\"}\n\n",
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":1,\"output_tokens\":2}}}\n\n"
        );
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
            .mount(&server)
            .await;

        let resp = reqwest::Client::new()
            .post(upstream_url(&server.uri(), "/v1/responses"))
            .send()
            .await
            .unwrap();
        let out = collect_stream(normalize_stream(resp, UpstreamProtocol::OpenAiResponses, "gpt-4o".into())).await;
        assert!(out.contains("\"content\":\"你\""), "文本增量应转成 chat delta: {out}");
        assert!(out.contains("\"completion_tokens\":2"), "应带 usage: {out}");
        assert!(out.contains("data: [DONE]"), "应补发 [DONE]: {out}");
    }

    /// Anthropic 上游流 → canonical chat SSE(文本 + tool_use + usage)。
    #[tokio::test]
    async fn normalize_anthropic_stream_to_chat_sse() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let sse = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-x\",\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"嗨\"}}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":7}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
        );
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
            .mount(&server)
            .await;

        let resp = reqwest::Client::new()
            .post(upstream_url(&server.uri(), "/v1/messages"))
            .send()
            .await
            .unwrap();
        let out = collect_stream(normalize_stream(resp, UpstreamProtocol::Anthropic, "claude-x".into())).await;
        assert!(out.contains("\"content\":\"嗨\""), "text_delta 应转成 chat delta: {out}");
        assert!(out.contains("\"finish_reason\":\"stop\""), "end_turn → stop: {out}");
        assert!(out.contains("\"prompt_tokens\":5"), "usage 应归一: {out}");
        assert!(out.contains("data: [DONE]"), "应补发 [DONE]: {out}");
    }

    async fn collect_stream(mut stream: ByteStream) -> String {
        let mut out = String::new();
        while let Some(item) = stream.next().await {
            if let Ok(bytes) = item {
                out.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
        out
    }
}
