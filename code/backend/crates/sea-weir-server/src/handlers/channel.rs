//! `channel` 域 handler:渠道管理(AdminAuth,CONTRACTS §4)。
//!
//! 已落地:分页列表(key 脱敏,含多 key 数量)/ 创建 / 更新 / 删除。
//! 创建与更新都会事务内重建 abilities 三元组(见 ChannelRepository::sync_abilities)。
//! 待补:测试渠道、余额探测、多 key 管理、上游模型拉取、密钥揭示(敏感凭证)。

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;

use sea_weir_adaptors::ApiType;
use sea_weir_repository::ChannelRepository;
use sea_weir_types::domain::Channel;
use sea_weir_types::dto::common::PageQuery;
use sea_weir_types::AppError;

use crate::app_state::ServerState;
use crate::middleware::auth::AdminUser;
use crate::response;

fn channel_repo(state: &ServerState) -> Result<Arc<dyn ChannelRepository>, Response> {
    state
        .channels
        .as_ref()
        .cloned()
        .ok_or_else(|| response::err(AppError::Database("数据库未连接".into())))
}

/// 渠道列表项:序列化渠道本体(`key` 已 `skip_serializing`),追加 key 数量。
fn channel_json(channel: Channel) -> serde_json::Value {
    let key_count = channel.keys().len();
    let mut value = serde_json::to_value(&channel).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = value.as_object_mut() {
        obj.insert("key_count".into(), serde_json::json!(key_count));
    }
    value
}

/// `GET /api/channel/`、`/search`:渠道分页(AdminAuth)。
pub async fn list(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Query(q): Query<PageQuery>,
) -> Response {
    let repo = match channel_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };
    let page = q.normalized_page();
    let page_size = if q.page_size <= 0 {
        10
    } else {
        q.page_size.min(100)
    };
    let offset = (page - 1) * page_size;

    let rows = match repo.list_paged(offset, page_size).await {
        Ok(rows) => rows,
        Err(e) => return response::err(e),
    };
    let total = match repo.count().await {
        Ok(total) => total,
        Err(e) => return response::err(e),
    };
    let items: Vec<serde_json::Value> = rows.into_iter().map(channel_json).collect();

    response::ok(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "page_size": page_size,
    }))
}

/// `GET /api/channel/:id`:单个渠道(key 不返回)。
pub async fn get(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Path(id): Path<i64>,
) -> Response {
    let repo = match channel_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };
    match repo.find_by_id(id).await {
        Ok(Some(channel)) => response::ok(channel_json(channel)),
        Ok(None) => response::err(AppError::NotFound("渠道不存在".into())),
        Err(e) => response::err(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct ChannelRequest {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(rename = "type")]
    pub r#type: i32,
    /// 创建时必填;更新时留空表示沿用原密钥(列表不回传 key)。
    #[serde(default)]
    pub key: String,
    pub name: String,
    pub models: String,
    pub group: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub weight: Option<i64>,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub auto_ban: Option<i32>,
    #[serde(default)]
    pub test_model: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub setting: Option<serde_json::Value>,
    #[serde(default)]
    pub model_mapping: Option<serde_json::Value>,
    #[serde(default)]
    pub param_override: Option<serde_json::Value>,
    #[serde(default)]
    pub header_override: Option<serde_json::Value>,
    #[serde(default)]
    pub status_code_mapping: Option<String>,
}

impl ChannelRequest {
    fn into_channel(self, existing: Option<&Channel>) -> Channel {
        let base = existing.cloned().unwrap_or(Channel {
            id: 0,
            r#type: self.r#type,
            key: String::new(),
            status: 1,
            name: String::new(),
            weight: 0,
            priority: 0,
            group: String::new(),
            models: String::new(),
            model_mapping: None,
            param_override: None,
            header_override: None,
            base_url: None,
            openai_organization: None,
            test_model: None,
            balance: 0.0,
            balance_updated_time: 0,
            used_quota: 0,
            auto_ban: 1,
            tag: None,
            setting: None,
            other_settings: None,
            channel_info: None,
            status_code_mapping: None,
            other_info: None,
            created_time: 0,
            test_time: 0,
            response_time: 0,
        });

        Channel {
            id: self.id.unwrap_or(base.id),
            r#type: self.r#type,
            // 更新时若未传 key,保留原密钥(列表不回传 key,UI 编辑无法带出)。
            key: if self.key.trim().is_empty() {
                base.key
            } else {
                self.key
            },
            status: self.status.unwrap_or(base.status),
            name: self.name,
            weight: self.weight.unwrap_or(base.weight),
            priority: self.priority.unwrap_or(base.priority),
            group: self.group,
            models: self.models,
            model_mapping: self.model_mapping.or(base.model_mapping),
            param_override: self.param_override.or(base.param_override),
            header_override: self.header_override.or(base.header_override),
            base_url: self.base_url.or(base.base_url),
            openai_organization: base.openai_organization,
            test_model: self.test_model.or(base.test_model),
            balance: base.balance,
            balance_updated_time: base.balance_updated_time,
            used_quota: base.used_quota,
            auto_ban: self.auto_ban.unwrap_or(base.auto_ban),
            tag: self.tag.or(base.tag),
            setting: self.setting.or(base.setting),
            other_settings: base.other_settings,
            channel_info: base.channel_info,
            status_code_mapping: self.status_code_mapping.or(base.status_code_mapping),
            other_info: base.other_info,
            created_time: base.created_time,
            test_time: base.test_time,
            response_time: base.response_time,
        }
    }
}

fn validate(req: &ChannelRequest, require_key: bool) -> Result<(), AppError> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("渠道名称不能为空".into()));
    }
    if req.models.trim().is_empty() {
        return Err(AppError::BadRequest("模型列表不能为空".into()));
    }
    if req.group.trim().is_empty() {
        return Err(AppError::BadRequest("分组不能为空".into()));
    }
    if require_key && req.key.trim().is_empty() {
        return Err(AppError::BadRequest("渠道密钥不能为空".into()));
    }
    Ok(())
}

/// `POST /api/channel/`:创建渠道并同步 abilities(AdminAuth)。
pub async fn create(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Json(req): Json<ChannelRequest>,
) -> Response {
    let repo = match channel_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };
    if let Err(e) = validate(&req, true) {
        return response::err(e);
    }
    match repo.exists_name(req.name.trim()).await {
        Ok(true) => return response::err(AppError::Biz("渠道名称已存在".into())),
        Ok(false) => {}
        Err(e) => return response::err(e),
    }

    let channel = req.into_channel(None);
    match repo.create(&channel).await {
        Ok(id) => match repo.find_by_id(id).await {
            Ok(Some(created)) => response::ok(channel_json(created)),
            Ok(None) => response::err(AppError::Internal("创建后查询渠道失败".into())),
            Err(e) => response::err(e),
        },
        Err(e) => response::err(e),
    }
}

/// `PUT /api/channel/`:更新渠道并重建 abilities(AdminAuth)。
pub async fn update(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Json(req): Json<ChannelRequest>,
) -> Response {
    let repo = match channel_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };
    let Some(id) = req.id else {
        return response::err(AppError::BadRequest("缺少 id".into()));
    };
    let existing = match repo.find_by_id(id).await {
        Ok(Some(channel)) => channel,
        Ok(None) => return response::err(AppError::NotFound("渠道不存在".into())),
        Err(e) => return response::err(e),
    };
    if let Err(e) = validate(&req, false) {
        return response::err(e);
    }

    let channel = req.into_channel(Some(&existing));
    if let Err(e) = repo.update(&channel).await {
        return response::err(e);
    }
    match repo.find_by_id(id).await {
        Ok(Some(updated)) => response::ok(channel_json(updated)),
        Ok(None) => response::err(AppError::NotFound("渠道不存在".into())),
        Err(e) => response::err(e),
    }
}

/// `DELETE /api/channel/:id`:删除渠道 + abilities(AdminAuth)。
pub async fn delete(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Path(id): Path<i64>,
) -> Response {
    let repo = match channel_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };
    match repo.find_by_id(id).await {
        Ok(None) => return response::err(AppError::NotFound("渠道不存在".into())),
        Ok(Some(_)) => {}
        Err(e) => return response::err(e),
    }
    match repo.delete(id).await {
        Ok(()) => response::ok(serde_json::json!({})),
        Err(e) => response::err(e),
    }
}

/// `GET /api/channel/update_balance/:id`(AdminAuth):刷新单渠道余额。
pub async fn update_balance_by_id(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Path(id): Path<i64>,
) -> Response {
    let repo = match channel_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };
    let channel = match repo.find_by_id(id).await {
        Ok(Some(c)) => c,
        Ok(None) => return response::err(AppError::NotFound("渠道不存在".into())),
        Err(e) => return response::err(e),
    };

    match refresh_balance(&state, &channel).await {
        Ok(balance) => {
            let now = chrono::Utc::now().timestamp();
            if let Err(e) = repo.update_balance(id, balance, now).await {
                return response::err(e);
            }
            response::ok(serde_json::json!({
                "id": id,
                "balance": balance,
                "balance_updated_time": now,
            }))
        }
        Err(resp) => resp,
    }
}

/// `GET /api/channel/update_balance`(AdminAuth):刷新全部渠道余额(尽力而为)。
pub async fn update_balance_all(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
) -> Response {
    let repo = match channel_repo(&state) {
        Ok(repo) => repo,
        Err(resp) => return resp,
    };
    let channels = match repo.list_paged(0, 1000).await {
        Ok(rows) => rows,
        Err(e) => return response::err(e),
    };

    let mut results = Vec::with_capacity(channels.len());
    for channel in channels {
        let id = channel.id;
        let name = channel.name.clone();
        match refresh_balance(&state, &channel).await {
            Ok(balance) => {
                let now = chrono::Utc::now().timestamp();
                if let Err(e) = repo.update_balance(id, balance, now).await {
                    results.push(serde_json::json!({"id": id, "name": name, "error": e.to_string()}));
                } else {
                    results.push(serde_json::json!({"id": id, "name": name, "balance": balance}));
                }
            }
            Err(_resp) => {
                results.push(serde_json::json!({"id": id, "name": name, "error": "不支持或上游错误"}));
            }
        }
    }
    response::ok(serde_json::json!({ "results": results }))
}

/// 按渠道类型探测余额。未知类型按 OpenAI 兼容尝试。
async fn refresh_balance(state: &ServerState, channel: &Channel) -> Result<f64, Response> {
    match ApiType::from_channel_type(channel.r#type).unwrap_or(ApiType::OpenAi) {
        ApiType::OpenAi => query_openai_balance(state, channel).await,
        other => Err(response::err(AppError::Biz(format!(
            "渠道类型 {other:?} 暂不支持余额探测"
        )))),
    }
}

/// OpenAI 兼容平台:`GET {base}/dashboard/billing/subscription`。
///
/// 优先取 `total_granted - total_used`(剩余额度);否则取 `hard_limit_usd`(额度上限)。
async fn query_openai_balance(state: &ServerState, channel: &Channel) -> Result<f64, Response> {
    let base = match channel.base_url.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(b) => b.trim_end_matches("/v1").trim_end_matches('/'),
        None => return Err(response::err(AppError::BadRequest("渠道未配置 base_url".into()))),
    };
    let key = channel
        .keys()
        .first()
        .map(|k| k.to_string())
        .filter(|k| !k.is_empty());
    let Some(key) = key else {
        return Err(response::err(AppError::BadRequest("渠道未配置密钥".into())));
    };

    let url = format!("{base}/dashboard/billing/subscription");
    let resp = state
        .http
        .get(&url)
        .bearer_auth(&key)
        .send()
        .await
        .map_err(|e| response::err(AppError::Upstream(format!("余额查询请求失败: {e}"))))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(response::err(AppError::Upstream(format!(
            "上游余额接口返回 {status}: {}",
            text.chars().take(300).collect::<String>()
        ))));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| response::err(AppError::Upstream(format!("解析余额响应失败: {e}"))))?;
    parse_balance(&body)
        .map_err(|e| response::err(AppError::Upstream(format!("无法解析余额: {e}"))))
}

fn parse_balance(body: &serde_json::Value) -> Result<f64, String> {
    let num = |key: &str| body.get(key).and_then(|v| v.as_f64());
    match (num("total_granted"), num("total_used")) {
        (Some(granted), Some(used)) => Ok(granted - used),
        _ => num("hard_limit_usd").ok_or_else(|| {
            "响应中无 total_granted/total_used/hard_limit_usd 字段".to_string()
        }),
    }
}
