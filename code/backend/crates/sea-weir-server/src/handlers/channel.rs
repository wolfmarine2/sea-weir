//! `channel` 域 handler:渠道管理(AdminAuth,CONTRACTS §4)。
//!
//! 已落地:分页列表(key 脱敏,含多 key 数量)/ 创建 / 更新 / 删除 /
//! 余额探测(国内平台内置 + setting.balance 可配)/ 渠道测试 / 拉取上游模型列表。
//! 创建与更新都会事务内重建 abilities 三元组(见 ChannelRepository::sync_abilities)。
//! 待补:多 key 管理、批量/标签、密钥揭示(敏感凭证)、Codex/Ollama 专项。

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;

use sea_weir_adaptors::ApiType;
use sea_weir_repository::ChannelRepository;
use sea_weir_types::constants::status as ch_status;
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

/// `GET /api/channel/types`(AdminAuth):渠道类型目录,供表单下拉。
///
/// 返回 `{items: [{type, name, default_base_url}]}`,由适配器注册表的 ChannelType
/// 登记表派生(与中继/余额探测的解析同源),避免前端硬编码编号。
pub async fn types(_auth: AdminUser) -> Response {
    let items: Vec<serde_json::Value> = sea_weir_adaptors::channel_type_catalog()
        .into_iter()
        .map(|(r#type, name, default_base_url)| {
            serde_json::json!({
                "type": r#type,
                "name": name,
                "default_base_url": default_base_url,
            })
        })
        .collect();
    response::ok(serde_json::json!({ "items": items, "total": items.len() }))
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

/// 按渠道类型探测余额。
///
/// 优先级:渠道 `setting.balance` 自定义配置 > 类型内置默认(国内平台为主)。
/// 未内置的类型返回业务错误并提示如何配置。
async fn refresh_balance(state: &ServerState, channel: &Channel) -> Result<f64, Response> {
    if let Some((path, field)) = balance_spec(channel.setting.as_ref()) {
        return query_balance(state, channel, &path, field.as_deref(), None).await;
    }
    match ApiType::from_channel_type(channel.r#type).unwrap_or(ApiType::OpenAi) {
        // OpenAI 兼容:额度上限 / 已用额度之差。
        ApiType::OpenAi => {
            query_balance(state, channel, "/dashboard/billing/subscription", None, Some("openai")).await
        }
        // 国内平台内置默认(字段名依据各自公开 HTTP API;如与线上不符可在 setting.balance 覆盖)。
        ApiType::DeepSeek => {
            query_balance(state, channel, "/user/balance", Some("/balance_infos/0/total_balance"), None).await
        }
        ApiType::Moonshot => {
            query_balance(state, channel, "/v1/users/me/balance", Some("/data/available_balance"), None).await
        }
        ApiType::SiliconFlow => {
            query_balance(state, channel, "/v1/user/info", Some("/data/totalBalance"), None).await
        }
        other => Err(response::err(AppError::Biz(format!(
            "渠道类型 {other:?} 未内置余额查询;可在渠道 setting 配置 balance: {{\"path\":\"...\",\"field\":\"/data/balance\"}}"
        )))),
    }
}

/// 从渠道 `setting` 读取自定义余额配置:`{"balance": {"path": "...", "field": "/a/b"}}`。
fn balance_spec(setting: Option<&serde_json::Value>) -> Option<(String, Option<String>)> {
    let balance = setting?.get("balance")?;
    let path = balance.get("path")?.as_str()?.trim().to_string();
    if path.is_empty() {
        return None;
    }
    let field = balance
        .get("field")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Some((path, field))
}

/// 通用余额查询:GET `base_url + path`,按 `field`(JSON Pointer)或常见字段名提取数值。
async fn query_balance(
    state: &ServerState,
    channel: &Channel,
    path: &str,
    field: Option<&str>,
    preset: Option<&str>,
) -> Result<f64, Response> {
    let base = match channel.base_url.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(b) => b.trim_end_matches('/'),
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

    // base_url 以 /v1 结尾且 path 也以 /v1 开头时避免重复。
    let url = if base.ends_with("/v1") && path.starts_with("/v1") {
        format!("{base}{}", &path[3..])
    } else {
        format!("{base}{path}")
    };

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

    extract_balance(&body, field, preset)
        .map_err(|e| response::err(AppError::Upstream(format!("无法解析余额: {e}"))))
}

/// 提取余额数值。支持 JSON Pointer 指定路径,或按常见字段名查找。
fn extract_balance(
    body: &serde_json::Value,
    field: Option<&str>,
    preset: Option<&str>,
) -> Result<f64, String> {
    if preset == Some("openai") {
        let num = |key: &str| body.get(key).and_then(as_f64_value);
        return match (num("total_granted"), num("total_used")) {
            (Some(granted), Some(used)) => Ok(granted - used),
            _ => num("hard_limit_usd").ok_or_else(|| {
                "响应中无 total_granted/total_used/hard_limit_usd 字段".to_string()
            }),
        };
    }

    if let Some(pointer) = field {
        let value = body
            .pointer(pointer)
            .ok_or_else(|| format!("响应中找不到字段 {pointer}"))?;
        return as_f64_value(value).ok_or_else(|| format!("字段 {pointer} 不是数值"));
    }

    // 常见字段名(含嵌套一层 data)。
    const KEYS: &[&str] = &[
        "total_balance",
        "available_balance",
        "totalBalance",
        "balance",
        "remain",
    ];
    if let Some(v) = KEYS.iter().find_map(|k| body.get(*k)) {
        if let Some(amount) = as_f64_value(v) {
            return Ok(amount);
        }
    }
    if let Some(data) = body.get("data") {
        if let Some(v) = KEYS.iter().find_map(|k| data.get(*k)) {
            if let Some(amount) = as_f64_value(v) {
                return Ok(amount);
            }
        }
    }
    Err("响应中未找到常见余额字段,请在渠道 setting.balance 指定 field".into())
}

/// 数值或数字字符串 → f64。
fn as_f64_value(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// 拼接上游 URL(base_url 以 `/v1` 结尾且 path 也以 `/v1` 开头时去重)。
fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.ends_with("/v1") && path.starts_with("/v1") {
        format!("{base}{}", &path[3..])
    } else {
        format!("{base}{path}")
    }
}

/// 渠道首个可用密钥。
fn first_key(channel: &Channel) -> Result<String, Response> {
    channel
        .keys()
        .first()
        .map(|k| k.to_string())
        .filter(|k| !k.is_empty())
        .ok_or_else(|| response::err(AppError::BadRequest("渠道未配置密钥".into())))
}

fn base_url(channel: &Channel) -> Result<&str, Response> {
    channel
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| response::err(AppError::BadRequest("渠道未配置 base_url".into())))
}

/// 拉取上游模型列表(`GET {base}/v1/models`)。
async fn fetch_upstream_models(
    state: &ServerState,
    channel: &Channel,
) -> Result<Vec<String>, Response> {
    let url = join_url(base_url(channel)?, "/v1/models");
    let resp = state
        .http
        .get(&url)
        .bearer_auth(first_key(channel)?)
        .send()
        .await
        .map_err(|e| response::err(AppError::Upstream(format!("拉取模型请求失败: {e}"))))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(response::err(AppError::Upstream(format!(
            "上游返回 {status}: {}",
            text.chars().take(300).collect::<String>()
        ))));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| response::err(AppError::Upstream(format!("解析模型列表失败: {e}"))))?;

    // OpenAI 兼容:`{"data":[{"id":...}]}`;部分平台:`{"models":["a","b"]}`。
    let mut models: Vec<String> = Vec::new();
    if let Some(list) = body.get("data").and_then(|v| v.as_array()) {
        for item in list {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                models.push(id.to_string());
            }
        }
    }
    if models.is_empty() {
        if let Some(list) = body.get("models").and_then(|v| v.as_array()) {
            for item in list {
                if let Some(id) = item.as_str() {
                    models.push(id.to_string());
                }
            }
        }
    }
    models.sort();
    models.dedup();
    Ok(models)
}

/// `GET /api/channel/fetch_models/:id`(AdminAuth):回源拉取该渠道的模型列表。
pub async fn fetch_models_by_id(
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
    match fetch_upstream_models(&state, &channel).await {
        Ok(models) => response::ok(serde_json::json!({ "id": id, "models": models })),
        Err(resp) => resp,
    }
}

/// 对渠道发一次最小请求测试连通性,并记录耗时。
async fn probe_channel(state: &ServerState, channel: &Channel) -> Result<(bool, i64), Response> {
    let test_model = channel
        .test_model
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| channel.models.split(',').map(str::trim).find(|s| !s.is_empty()).map(str::to_string))
        .ok_or_else(|| response::err(AppError::BadRequest("渠道未配置测试模型或模型列表".into())))?;

    let url = join_url(base_url(channel)?, "/v1/chat/completions");
    let body = serde_json::json!({
        "model": test_model,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 1,
    });

    let started = Instant::now();
    let resp = state
        .http
        .post(&url)
        .bearer_auth(first_key(channel)?)
        .json(&body)
        .send()
        .await
        .map_err(|e| response::err(AppError::Upstream(format!("测试请求失败: {e}"))))?;
    let elapsed = started.elapsed().as_millis() as i64;
    Ok((resp.status().is_success(), elapsed))
}

/// `GET /api/channel/test/:id`(AdminAuth):测试单渠道,通过则记录耗时;
/// 自动禁用的渠道测试通过后自动恢复启用并重建 abilities。
pub async fn test_by_id(
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

    let (ok, elapsed) = match probe_channel(&state, &channel).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let now = chrono::Utc::now().timestamp();
    if let Err(e) = repo.record_test_result(id, elapsed, now).await {
        tracing::warn!(error = %e, channel_id = id, "记录测试结果失败");
    }

    if ok && channel.status == ch_status::AUTO_DISABLED {
        // 自动恢复:仅对「自动禁用」生效,手动禁用(2)不自动恢复。
        if let Err(e) = repo
            .update_status(id, ch_status::ENABLED, "渠道测试通过,自动恢复")
            .await
        {
            tracing::warn!(error = %e, channel_id = id, "自动恢复启用失败");
        }
    }

    response::ok(serde_json::json!({
        "id": id,
        "success": ok,
        "response_time": elapsed,
        "message": if ok { "测试通过" } else { "测试失败(上游返回非 2xx)" },
    }))
}

/// `GET /api/channel/test`(AdminAuth):测试全部渠道(尽力而为)。
pub async fn test_all(State(state): State<Arc<ServerState>>, _auth: AdminUser) -> Response {
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
        match probe_channel(&state, &channel).await {
            Ok((ok, elapsed)) => {
                let now = chrono::Utc::now().timestamp();
                let _ = repo.record_test_result(id, elapsed, now).await;
                if ok && channel.status == ch_status::AUTO_DISABLED {
                    let _ = repo
                        .update_status(id, ch_status::ENABLED, "渠道测试通过,自动恢复")
                        .await;
                }
                results.push(serde_json::json!({
                    "id": id, "name": name, "success": ok, "response_time": elapsed
                }));
            }
            Err(_) => results.push(serde_json::json!({
                "id": id, "name": name, "success": false, "error": "未配置测试模型/base_url/密钥"
            })),
        }
    }
    response::ok(serde_json::json!({ "results": results }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deepseek_balance_by_pointer() {
        let body = json!({
            "is_available": true,
            "balance_infos": [{"currency": "CNY", "total_balance": "128.50", "granted_balance": "0.00"}]
        });
        let amount = extract_balance(&body, Some("/balance_infos/0/total_balance"), None).unwrap();
        assert!((amount - 128.5).abs() < f64::EPSILON);
    }

    #[test]
    fn moonshot_balance_numeric_string() {
        let body = json!({"code": 0, "status": true, "data": {"available_balance": 42.75}});
        assert_eq!(
            extract_balance(&body, Some("/data/available_balance"), None).unwrap(),
            42.75
        );
    }

    #[test]
    fn common_field_fallback() {
        let body = json!({"data": {"balance": "9.99"}});
        assert_eq!(extract_balance(&body, None, None).unwrap(), 9.99);
    }

    #[test]
    fn openai_preset_subtracts_used() {
        let body = json!({"total_granted": 100.0, "total_used": 40.0, "hard_limit_usd": 120.0});
        assert_eq!(extract_balance(&body, None, Some("openai")).unwrap(), 60.0);
    }

    #[test]
    fn missing_field_reports_error() {
        let body = json!({"foo": 1});
        assert!(extract_balance(&body, Some("/nope"), None).is_err());
    }

    #[test]
    fn balance_spec_from_setting() {
        let setting = json!({"balance": {"path": "/x", "field": "/a/b"}});
        assert_eq!(
            balance_spec(Some(&setting)),
            Some(("/x".to_string(), Some("/a/b".to_string())))
        );
        assert_eq!(balance_spec(Some(&json!({}))), None);
        assert_eq!(balance_spec(None), None);
    }
}
