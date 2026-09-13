//! 中继面入口。对应 C4 组件 `relay_entry`。
//!
//! 现状(最小可用):`POST /v1/chat/completions` —— sk-token 认证 → 选路 →
//! 转发上游 →(非流式)按 usage 扣费。流式为直通转发(暂不计费)。
//!
//! 待补:core::relay::pipeline 的预扣/结算/重试状态机、Claude/Gemini 入口格式、
//! SSE 的 usage 注入、上游 header/参数改写、自动禁用与消费日志。

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;
use rust_decimal::Decimal;

use sea_weir_core::relay::{billing, select};
use sea_weir_types::domain::Ability;
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

/// `POST /v1/chat/completions`(中继面,TokenAuth)。
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

    let channels = match state.channels.as_ref() {
        Some(repo) => repo,
        None => return relay_fail(AppError::Database("数据库未连接".into())),
    };

    // --- 选路:候选 → 优先级分档 → 档内加权随机 ---
    let candidates = match channels.list_candidates(&auth.group, &origin_model).await {
        Ok(c) => c,
        Err(e) => return relay_fail(e),
    };
    // 指定渠道优先
    let channel = if let Some(id) = auth.specific_channel_id {
        match candidates.into_iter().find(|c| c.id == id) {
            Some(c) => c,
            None => return relay_fail(AppError::NotFound("指定渠道不可用".into())),
        }
    } else {
        let abilities: Vec<Ability> = candidates
            .iter()
            .map(|c| Ability {
                group: auth.group.clone(),
                model: origin_model.clone(),
                channel_id: c.id,
                enabled: true,
                priority: c.priority,
                weight: c.weight,
                tag: c.tag.clone(),
            })
            .collect();
        let tiers = select::priority_tiers(&abilities);
        let picked = tiers
            .first()
            .and_then(|tier| select::weighted_pick(tier))
            .map(|a| a.channel_id);
        match picked.and_then(|id| candidates.into_iter().find(|c| c.id == id)) {
            Some(c) => c,
            None => return relay_fail(AppError::NotFound("无可用渠道".into())),
        }
    };

    // --- 渠道密钥(多 key 随机取一) ---
    let keys = channel.keys();
    if keys.is_empty() {
        return relay_fail(AppError::BadRequest("渠道未配置密钥".into()));
    }
    let idx = if keys.len() == 1 {
        0
    } else {
        rand::Rng::gen_range(&mut rand::thread_rng(), 0..keys.len())
    };
    let channel_key = keys[idx].to_string();

    // --- base_url 与模型映射 ---
    let Some(base_url) = channel.base_url.as_deref().filter(|s| !s.trim().is_empty()) else {
        return relay_fail(AppError::BadRequest("渠道未配置 base_url".into()));
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
    let upstream = state
        .http
        .post(&url)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {channel_key}"))
        .json(&body)
        .send()
        .await;

    let upstream = match upstream {
        Ok(resp) => resp,
        Err(e) => {
            return crate::response::relay_err(
                NewApiError {
                    status_code: 502,
                    error_code: "channel:request_failed".into(),
                    error_type: "new_api_error".into(),
                    message: format!("上游请求失败: {e}"),
                    local_error: false,
                    skip_retry: false,
                    record_error_log: true,
                },
                RelayFormat::OpenAi,
            );
        }
    };

    let status = upstream.status();
    if !status.is_success() {
        let code = if status == StatusCode::UNAUTHORIZED {
            "channel:invalid_key"
        } else {
            ""
        };
        let text = upstream.text().await.unwrap_or_default();
        return crate::response::relay_err(
            NewApiError {
                status_code: status.as_u16(),
                error_code: code.into(),
                error_type: "new_api_error".into(),
                message: text.chars().take(500).collect(),
                local_error: false,
                skip_retry: false,
                record_error_log: true,
            },
            RelayFormat::OpenAi,
        );
    }

    // --- 流式:直通转发(计费待补)---
    if is_stream {
        let stream = upstream.bytes_stream().map(|chunk| {
            chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        });
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from_stream(stream))
            .unwrap_or_else(|_| relay_fail(AppError::Internal("构造流式响应失败".into())));
    }

    // --- 非流式:解析 usage → 扣费 → 透传上游响应 ---
    let upstream_body: serde_json::Value = match upstream.json().await {
        Ok(v) => v,
        Err(e) => return relay_fail(AppError::Upstream(format!("解析上游响应失败: {e}"))),
    };

    if let Err(e) = charge(&state, &auth, &upstream_body).await {
        return relay_fail(e);
    }

    (StatusCode::OK, Json(upstream_body)).into_response()
}

/// 按上游 usage 扣费:用户钱包与令牌额度各扣一次(token 无限额度时跳过)。
///
/// NOTE(TDD): 这是**结算后扣费**的最小实现(无预扣/退款/日志);
/// 完整三段式(预扣→结算/退款)在 core::relay::pipeline 落地后替换。
async fn charge(
    state: &ServerState,
    auth: &crate::middleware::auth::AuthToken,
    upstream_body: &serde_json::Value,
) -> Result<(), AppError> {
    let usage = usage_from_body(upstream_body);
    let quota = billing::calculate_quota(&usage, &default_price());
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
