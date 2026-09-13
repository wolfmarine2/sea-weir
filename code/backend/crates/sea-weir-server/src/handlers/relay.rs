//! 中继面入口。对应 C4 组件 `relay_entry`。
//!
//! 现状:sk-token 认证 → 重试循环(每次重新选渠取下一优先级档,档内加权随机)
//! → 转发上游 → 成功扣费 + 记消费日志;失败按 autoban 判定(自动禁用 / 是否重试)。
//!
//! 待补:core::relay::pipeline 的预扣/结算/退款状态机(SEQ-003)、SSE usage 注入、
//! 流式计费、Claude/Gemini 入口格式、上游 header/参数改写、request-id 贯穿。

use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;
use rust_decimal::Decimal;

use sea_weir_core::relay::{autoban, billing, select};
use sea_weir_types::constants::{status as ch_status, LogType};
use sea_weir_types::domain::{Ability, Channel, Log};
use sea_weir_types::dto::Usage;
use sea_weir_types::{AppError, NewApiError, RelayFormat};

use crate::app_state::ServerState;
use crate::middleware::auth::TokenAuth;

/// 暂缺定价视图,先用单位倍率;接入 options/定价缓存后替换。
fn default_price() -> billing::PriceData {
    billing::PriceData {
        model_ratio: Decimal::ONE,
        group_ratio: Decimal::ONE,
        completion_ratio: Decimal::ONE,
        cache_ratio: Decimal::ONE,
        create_cache_ratio: Decimal::ONE,
        cache_creation_5m_ratio: Decimal::ONE,
        cache_creation_1h_ratio: Decimal::ONE,
        image_ratio: Decimal::ONE,
        audio_ratio: Decimal::ONE,
        audio_input_price: Decimal::ZERO,
        model_price: None,
        tiered_expr: None,
        other_ratios: Vec::new(),
    }
}

fn relay_fail(e: AppError) -> Response {
    crate::response::relay_err(e.into(), RelayFormat::OpenAi)
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
    /// 流式响应(直通,计费等 SSE 管线落地后补)。
    Stream(reqwest::Response),
    /// 非流式响应(已解析出 JSON)。
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
        .map_err(|e| NewApiError {
            status_code: 502,
            error_code: "channel:request_failed".into(),
            error_type: "new_api_error".into(),
            message: format!("上游请求失败: {e}"),
            local_error: false,
            skip_retry: false,
            record_error_log: true,
        })?;

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
        .map_err(|e| NewApiError {
            status_code: 502,
            error_code: "channel:parse_failed".into(),
            error_type: "new_api_error".into(),
            message: format!("解析上游响应失败: {e}"),
            local_error: false,
            skip_retry: false,
            record_error_log: true,
        })
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
/// 说明:排除已失败渠道(而非固定取第 N 档),是为了正确处理自动禁用 —
/// 失败渠道被禁用后候选集会缩小,固定档位索引会越界。
fn pick_channel<'a>(
    candidates: &'a [Channel],
    auth: &crate::middleware::auth::AuthToken,
    origin_model: &str,
    tried: &std::collections::HashSet<i64>,
) -> Option<&'a Channel> {
    let available: Vec<&Channel> = candidates
        .iter()
        .filter(|c| !tried.contains(&c.id))
        .collect();
    // 若排除后为空(如唯一渠道的瞬时错误),回落到全部候选以允许重试。
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

/// `POST /v1/chat/completions`(中继面,TokenAuth)。含重试循环。
pub async fn chat_completions(
    State(state): State<Arc<ServerState>>,
    TokenAuth(auth): TokenAuth,
    _headers: HeaderMap,
    Json(mut body): Json<serde_json::Value>,
) -> Response {
    let origin_model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if origin_model.is_empty() {
        return relay_fail(AppError::BadRequest("缺少 model".into()));
    }
    let is_stream = body.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
    let request_id = uuid::Uuid::new_v4().to_string();
    let started = Instant::now();

    let channels = match state.channels.as_ref() {
        Some(repo) => repo.clone(),
        None => return relay_fail(AppError::Database("数据库未连接".into())),
    };

    let retry_times = state.config.relay.retry_times;
    let mut last_err: Option<NewApiError> = None;
    let mut tried = std::collections::HashSet::new();

    for attempt in 0..=retry_times {
        // 每次尝试重新拉候选(自动禁用的渠道已剔除),并排除本请求内已失败的渠道。
        let candidates = match channels.list_candidates(&auth.group, &origin_model).await {
            Ok(c) => c,
            Err(e) => return relay_fail(e),
        };
        let Some(channel) = pick_channel(&candidates, &auth, &origin_model, &tried) else {
            break;
        };

        let keys = channel.keys();
        if keys.is_empty() {
            last_err = Some(channel_err("channel:no_key", "渠道未配置密钥", 502));
            break; // 本地配置问题,非上游,不重试
        }
        let idx = if keys.len() == 1 {
            0
        } else {
            rand::Rng::gen_range(&mut rand::thread_rng(), 0..keys.len())
        };
        let channel_key = keys[idx].to_string();

        let Some(base_url) = channel.base_url.as_deref().filter(|s| !s.trim().is_empty()) else {
            last_err = Some(channel_err("channel:no_base_url", "渠道未配置 base_url", 502));
            break; // 本地配置问题
        };
        let upstream_model = channel
            .model_mapping
            .as_ref()
            .and_then(|m| m.get(&origin_model))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| origin_model.clone());
        if let Some(obj) = body.as_object_mut() {
            obj.insert("model".into(), serde_json::json!(upstream_model));
        }

        let url = upstream_url(base_url, "/v1/chat/completions");
        match forward_once(&state.http, &url, &channel_key, &body, is_stream).await {
            Ok(UpstreamOutcome::Stream(resp)) => {
                let stream = resp.bytes_stream().map(|chunk| {
                    chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
                });
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .header(header::CACHE_CONTROL, "no-cache")
                    .body(Body::from_stream(stream))
                    .unwrap_or_else(|_| relay_fail(AppError::Internal("构造流式响应失败".into())));
            }
            Ok(UpstreamOutcome::Json(upstream_body)) => {
                let usage = usage_from_body(&upstream_body);
                let quota = billing::calculate_quota(&usage, &default_price());
                let use_time = started.elapsed().as_secs() as i32;
                if let Err(e) = charge(&state, &auth, quota).await {
                    return relay_fail(e);
                }
                log_consume(
                    &state,
                    &auth,
                    &channel,
                    &origin_model,
                    &usage,
                    quota,
                    use_time,
                    is_stream,
                    &request_id,
                )
                .await;
                return (StatusCode::OK, Json(upstream_body)).into_response();
            }
            Err(err) => {
                // 自动禁用判定(命中禁用码,默认仅 401)。
                if autoban::should_disable(&err, channel.auto_ban != 0) {
                    let _ = channels
                        .update_status(
                            channel.id,
                            ch_status::AUTO_DISABLED,
                            &err.message,
                        )
                        .await;
                }
                tried.insert(channel.id);
                let ctx = autoban::RetryContext {
                    retry_times_left: retry_times.saturating_sub(attempt),
                    has_specific_channel: auth.specific_channel_id.is_some(),
                    affinity_skip_retry: false,
                };
                if !autoban::should_retry(&err, &ctx) {
                    return crate::response::relay_err(err, RelayFormat::OpenAi);
                }
                last_err = Some(err);
            }
        }
    }

    crate::response::relay_err(
        last_err.unwrap_or_else(|| NewApiError::from(AppError::NotFound("无可用渠道".into()))),
        RelayFormat::OpenAi,
    )
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

/// 按上游 usage 扣费:用户钱包与令牌额度各扣一次(token 无限额度时跳过)。
///
/// NOTE(TDD): 最小实现(无预扣/退款);三段式(预扣→结算)在 pipeline 落地后替换。
async fn charge(
    state: &ServerState,
    auth: &crate::middleware::auth::AuthToken,
    quota: i64,
) -> Result<(), AppError> {
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
        // 令牌额度不足 → 回滚用户扣减,保持账目一致。
        users.increase_quota(auth.user_id, quota).await?;
        return Err(AppError::QuotaExceeded);
    }
    Ok(())
}

/// 写消费日志(失败仅告警,不影响响应)。
async fn log_consume(
    state: &ServerState,
    auth: &crate::middleware::auth::AuthToken,
    channel: &Channel,
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
        channel_id: Some(channel.id),
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
