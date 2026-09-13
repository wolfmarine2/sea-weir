//! 中继面入口。对应 C4 组件 `relay_entry`。
//!
//! 入口:OpenAI `POST /v1/chat/completions`、Claude `POST /v1/messages`。
//! 两者共用 [`run_relay`]:认证 → 协议转换(入口格式 → OpenAI,上游统一按 OpenAI 兼容转发)
//! → 重试循环选路 → 扣费 + 消费日志 → 响应转回入口格式。
//!
//! 待补:Claude/Gemini 流式(SSE 事件改写)、上游 header/参数改写、
//! core::relay::pipeline 的预扣/结算状态机、MJ/任务类入口。

use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;

use sea_weir_core::relay::convert::{convert_request, convert_response, RequestConversionChain};
use sea_weir_core::relay::{autoban, billing, select};
use sea_weir_types::constants::{status as ch_status, LogType};
use sea_weir_types::domain::{Ability, Channel, Log};
use sea_weir_types::dto::{RelayRequest, Usage};
use sea_weir_types::{AppError, NewApiError, RelayFormat};

use crate::app_state::ServerState;
use crate::middleware::auth::{AuthToken, TokenAuth};

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

/// 发起一次上游调用。错误统一归一化为 `NewApiError`(供重试/自动禁用判定)。
async fn forward_once(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    body: &serde_json::Value,
    is_stream: bool,
) -> Result<UpstreamOutcome, NewApiError> {
    let resp = http
        .post(url)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {key}"))
        .json(body)
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

    // Claude/Gemini 流式的事件改写尚未实现。
    if is_stream && entry != RelayFormat::OpenAi {
        return crate::response::relay_err(
            NewApiError {
                status_code: 501,
                error_code: String::new(),
                error_type: "new_api_error".into(),
                message: "该入口暂不支持流式响应".into(),
                local_error: true,
                skip_retry: true,
                record_error_log: false,
            },
            entry,
        );
    }

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

        let url = upstream_url(base_url, "/v1/chat/completions");
        match forward_once(&state.http, &url, &channel_key, &upstream_body, is_stream).await {
            Ok(UpstreamOutcome::Stream(resp)) => {
                return stream_response(
                    state.clone(),
                    auth.clone(),
                    channel.id,
                    origin_model.clone(),
                    request_id.clone(),
                    resp,
                    price.clone(),
                );
            }
            Ok(UpstreamOutcome::Json(upstream_resp)) => {
                let usage = usage_from_body(&upstream_resp);
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

                let out = match convert_response(&upstream_resp, RelayFormat::OpenAi, entry) {
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
    resp: reqwest::Response,
    price: billing::PriceData,
) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(16);
    let started = Instant::now();

    tokio::spawn(async move {
        let mut usage = Usage::default();
        let mut line_buf = String::new();
        let mut stream = resp.bytes_stream();
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
async fn charge(state: &ServerState, auth: &AuthToken, quota: i64) -> Result<(), AppError> {
    if quota <= 0 {
        return Ok(());
    }
    let users = state
        .users
        .as_ref()
        .ok_or_else(|| AppError::Database("数据库未连接".into()))?;
    let tokens = state
        .tokens
        .as_ref()
        .ok_or_else(|| AppError::Database("数据库未连接".into()))?;

    if !users.try_decrease_quota(auth.user_id, quota).await? {
        return Err(AppError::QuotaExceeded);
    }
    if !tokens.try_decrease_quota(auth.token_id, quota).await? {
        users.increase_quota(auth.user_id, quota).await?;
        return Err(AppError::QuotaExceeded);
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
