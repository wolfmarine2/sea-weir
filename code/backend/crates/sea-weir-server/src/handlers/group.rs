//! `group` 域 handler:分组管理(AdminAuth)。
//!
//! 分组不落独立表,配置集中在 `options`:
//! - `GroupRatio`(`{group: 倍率}`):分组的计费倍率,定价视图直接读它;
//! - `UserUsableGroups`(`{group: 描述}`):用户可选分组及其展示名。
//!
//! 因此"增删改分组"= 维护这两张映射:
//! - 新增:写入 GroupRatio(可选同时进 UserUsableGroups);
//! - 修改:名称即身份(不支持改名),可改倍率 / 描述 / 是否用户可选;
//! - 删除:同时从两处移除;已被渠道(abilities)引用的分组拒绝删除,`default` 不可删。
//!
//! 变更后失效定价缓存并广播,使倍率即时生效(与 `option` 域同款处理)。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;

use sea_weir_repository::OptionRepository;
use sea_weir_types::{AppError, AppResult};

use crate::app_state::ServerState;
use crate::middleware::auth::AdminUser;
use crate::response;

const K_RATIO: &str = "GroupRatio";
const K_USABLE: &str = "UserUsableGroups";
/// 保留分组:`default` 不可删除;`auto` 为选路保留字,不可新建。
const DEFAULT_GROUP: &str = "default";
const RESERVED_GROUPS: &[&str] = &["auto"];

#[derive(Debug, Deserialize)]
pub struct GroupRequest {
    pub name: String,
    /// 计费倍率;缺省 1.0。
    #[serde(default)]
    pub ratio: Option<f64>,
    /// 展示名/描述(写入 UserUsableGroups 的值)。
    #[serde(default)]
    pub description: Option<String>,
    /// 是否进入 `UserUsableGroups`(用户可选)。新增缺省 true。
    #[serde(default)]
    pub usable: Option<bool>,
}

/// `GET /api/group/`(AdminAuth):分组列表。
///
/// 名称集合 = GroupRatio ∪ UserUsableGroups ∪ 已被渠道使用的分组;
/// `in_use` / `deletable` 供前端决定删除按钮是否可用。
pub async fn list(State(state): State<Arc<ServerState>>, _auth: AdminUser) -> Response {
    let Some(options) = state.options.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    let ratios = match load_ratio_map(options.as_ref()).await {
        Ok(m) => m,
        Err(e) => return response::err(e),
    };
    let usable = match load_usable_map(options.as_ref()).await {
        Ok(m) => m,
        Err(e) => return response::err(e),
    };
    let in_use: BTreeSet<String> = match state.channels.as_ref() {
        Some(channels) => channels
            .list_group_names()
            .await
            .unwrap_or_default()
            .into_iter()
            .collect(),
        None => BTreeSet::new(),
    };

    let mut names: BTreeSet<String> = ratios.keys().cloned().collect();
    names.extend(usable.keys().cloned());
    names.extend(in_use.iter().cloned());

    let items: Vec<serde_json::Value> = names
        .into_iter()
        .map(|name| group_json(&name, &ratios, &usable, &in_use))
        .collect();
    response::ok(serde_json::json!({ "items": items, "total": items.len() }))
}

/// `POST /api/group/`(AdminAuth):新增分组。
pub async fn create(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Json(req): Json<GroupRequest>,
) -> Response {
    let Some(options) = state.options.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    if let Err(e) = validate_group_name(&req.name) {
        return response::err(e);
    }
    let ratio = req.ratio.unwrap_or(1.0);
    if let Err(e) = validate_ratio(ratio) {
        return response::err(e);
    }
    let mut ratios = match load_ratio_map(options.as_ref()).await {
        Ok(m) => m,
        Err(e) => return response::err(e),
    };
    let name = req.name.trim().to_string();
    if ratios.contains_key(&name) {
        return response::err(AppError::Biz(format!("分组 {name} 已存在")));
    }
    let mut usable = match load_usable_map(options.as_ref()).await {
        Ok(m) => m,
        Err(e) => return response::err(e),
    };

    ratios.insert(name.clone(), ratio);
    if req.usable.unwrap_or(true) {
        usable.insert(name.clone(), req.description.clone().unwrap_or_default());
    }
    if let Err(e) = save_groups(options.as_ref(), &ratios, &usable).await {
        return response::err(e);
    }
    invalidate(&state).await;
    tracing::info!(group = %name, ratio, "分组已创建");
    response::ok(serde_json::json!({ "name": name }))
}

/// `PUT /api/group/`(AdminAuth):修改分组(名称即身份,不支持改名)。
///
/// 未传字段保持原值;`usable=false` 从 UserUsableGroups 移除(用户不再能选中,
/// 已有用户/渠道继续按原名工作)。
pub async fn update(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Json(req): Json<GroupRequest>,
) -> Response {
    let Some(options) = state.options.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    if let Err(e) = validate_group_name(&req.name) {
        return response::err(e);
    }
    if let Some(ratio) = req.ratio {
        if let Err(e) = validate_ratio(ratio) {
            return response::err(e);
        }
    }
    let name = req.name.trim().to_string();
    let mut ratios = match load_ratio_map(options.as_ref()).await {
        Ok(m) => m,
        Err(e) => return response::err(e),
    };
    let mut usable = match load_usable_map(options.as_ref()).await {
        Ok(m) => m,
        Err(e) => return response::err(e),
    };
    if !ratios.contains_key(&name) && !usable.contains_key(&name) {
        return response::err(AppError::NotFound(format!("分组 {name} 不存在")));
    }

    if let Some(ratio) = req.ratio {
        ratios.insert(name.clone(), ratio);
    }
    match req.usable {
        Some(true) => {
            let desc = req
                .description
                .clone()
                .or_else(|| usable.get(&name).cloned())
                .unwrap_or_default();
            usable.insert(name.clone(), desc);
        }
        Some(false) => {
            usable.remove(&name);
        }
        None => {
            if let (Some(desc), Some(existing)) = (req.description.clone(), usable.get_mut(&name)) {
                *existing = desc;
            }
        }
    }

    if let Err(e) = save_groups(options.as_ref(), &ratios, &usable).await {
        return response::err(e);
    }
    invalidate(&state).await;
    tracing::info!(group = %name, "分组已更新");
    response::ok(serde_json::json!({ "name": name }))
}

/// `DELETE /api/group/:name`(AdminAuth):删除分组。
///
/// 已被渠道引用的分组拒绝删除(会改变既有选路),需先调整渠道分组。
pub async fn delete(
    State(state): State<Arc<ServerState>>,
    _auth: AdminUser,
    Path(name): Path<String>,
) -> Response {
    let Some(options) = state.options.as_ref() else {
        return response::err(AppError::Database("数据库未连接".into()));
    };
    let name = name.trim().to_string();
    if name.is_empty() {
        return response::err(AppError::BadRequest("缺少分组名".into()));
    }
    if name == DEFAULT_GROUP {
        return response::err(AppError::Biz("默认分组不可删除".into()));
    }
    if let Some(channels) = state.channels.as_ref() {
        match channels.list_group_names().await {
            Ok(groups) if groups.iter().any(|g| g == &name) => {
                return response::err(AppError::Biz(format!(
                    "分组 {name} 仍被渠道使用,请先在渠道里移除该分组再删除"
                )));
            }
            Ok(_) => {}
            Err(e) => return response::err(e),
        }
    }

    let mut ratios = match load_ratio_map(options.as_ref()).await {
        Ok(m) => m,
        Err(e) => return response::err(e),
    };
    let mut usable = match load_usable_map(options.as_ref()).await {
        Ok(m) => m,
        Err(e) => return response::err(e),
    };
    ratios.remove(&name);
    usable.remove(&name);
    if let Err(e) = save_groups(options.as_ref(), &ratios, &usable).await {
        return response::err(e);
    }
    invalidate(&state).await;
    tracing::info!(group = %name, "分组已删除");
    response::ok(serde_json::json!({ "name": name }))
}

/// 组装列表项。
fn group_json(
    name: &str,
    ratios: &BTreeMap<String, f64>,
    usable: &BTreeMap<String, String>,
    in_use: &BTreeSet<String>,
) -> serde_json::Value {
    let used = in_use.contains(name);
    serde_json::json!({
        "name": name,
        "ratio": ratios.get(name).copied().unwrap_or(1.0),
        "description": usable.get(name).cloned().unwrap_or_default(),
        "usable": usable.contains_key(name),
        "in_use": used,
        "deletable": name != DEFAULT_GROUP && !used,
    })
}

async fn load_ratio_map(options: &dyn OptionRepository) -> AppResult<BTreeMap<String, f64>> {
    let raw = options.get(K_RATIO).await?;
    Ok(parse_ratio_map(raw.as_deref()))
}

async fn load_usable_map(options: &dyn OptionRepository) -> AppResult<BTreeMap<String, String>> {
    let raw = options.get(K_USABLE).await?;
    Ok(parse_usable_map(raw.as_deref()))
}

/// 两张映射一起写回(先写倍率,再写用户可选分组)。
async fn save_groups(
    options: &dyn OptionRepository,
    ratios: &BTreeMap<String, f64>,
    usable: &BTreeMap<String, String>,
) -> AppResult<()> {
    options
        .upsert(K_RATIO, &serde_json::to_string(ratios).unwrap_or_else(|_| "{}".into()))
        .await?;
    options
        .upsert(K_USABLE, &serde_json::to_string(usable).unwrap_or_else(|_| "{}".into()))
        .await?;
    Ok(())
}

/// 分组倍率/可用性变更后:定价缓存失效 + 多节点广播。
async fn invalidate(state: &ServerState) {
    state.pricing.invalidate().await;
    if let Some(cache) = state.cache.as_ref() {
        if let Err(e) = cache.publish("cache:invalidate:option", "1").await {
            tracing::warn!(error = %e, "发布缓存失效广播失败");
        }
    }
}

// ───────────────────────── 纯函数(便于单测) ─────────────────────────

/// 解析 `GroupRatio`。容忍空值/非对象/非法项(跳过单项而非整表失败)。
fn parse_ratio_map(raw: Option<&str>) -> BTreeMap<String, f64> {
    let Some(value) = raw.and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()) else {
        return BTreeMap::new();
    };
    let Some(obj) = value.as_object() else {
        return BTreeMap::new();
    };
    obj.iter()
        .filter_map(|(k, v)| {
            let ratio = v.as_f64()?;
            (ratio.is_finite() && ratio > 0.0).then(|| (k.clone(), ratio))
        })
        .collect()
}

/// 解析 `UserUsableGroups`。值非字符串时用其 JSON 文本兜底。
fn parse_usable_map(raw: Option<&str>) -> BTreeMap<String, String> {
    let Some(value) = raw.and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()) else {
        return BTreeMap::new();
    };
    let Some(obj) = value.as_object() else {
        return BTreeMap::new();
    };
    obj.iter()
        .map(|(k, v)| {
            let desc = v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string());
            (k.clone(), desc)
        })
        .collect()
}

/// 分组名校验:非空、≤64、不含逗号/空白/斜杠(渠道 `group` 是逗号分隔多值,
/// 且名称会作为 `DELETE /api/group/:name` 的路径参数)、非保留名。
fn validate_group_name(name: &str) -> Result<(), AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("分组名不能为空".into()));
    }
    if name.chars().count() > 64 {
        return Err(AppError::BadRequest("分组名长度不能超过 64".into()));
    }
    if name.chars().any(|c| c == ',' || c == '/' || c.is_whitespace()) {
        return Err(AppError::BadRequest(
            "分组名不能包含逗号、斜杠或空白(渠道按逗号分隔多分组)".into(),
        ));
    }
    if RESERVED_GROUPS.contains(&name) {
        return Err(AppError::BadRequest(format!("{name} 为系统保留分组名")));
    }
    Ok(())
}

fn validate_ratio(ratio: f64) -> Result<(), AppError> {
    if !ratio.is_finite() || ratio <= 0.0 {
        return Err(AppError::BadRequest("倍率必须为大于 0 的有限数".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_ratio_map_tolerates_bad_entries() {
        let map = parse_ratio_map(Some(r#"{"default":1,"vip":0.8,"bad":"x","zero":0}"#));
        assert_eq!(map.get("default"), Some(&1.0));
        assert_eq!(map.get("vip"), Some(&0.8));
        assert!(!map.contains_key("bad"), "非数字项应跳过");
        assert!(!map.contains_key("zero"), "非正数应跳过");
        assert!(parse_ratio_map(None).is_empty());
        assert!(parse_ratio_map(Some("not-json")).is_empty());
        assert!(parse_ratio_map(Some("[1,2]")).is_empty(), "非对象应为空");
    }

    #[test]
    fn parse_usable_map_uses_json_text_as_fallback() {
        let map = parse_usable_map(Some(r#"{"default":"默认分组","vip":null}"#));
        assert_eq!(map.get("default").map(String::as_str), Some("默认分组"));
        assert_eq!(map.get("vip").map(String::as_str), Some("null"));
        assert!(parse_usable_map(Some("oops")).is_empty());
    }

    #[test]
    fn validate_group_name_rules() {
        assert!(validate_group_name("vip").is_ok());
        assert!(validate_group_name("  vip  ").is_ok(), "应容忍首尾空白");
        assert!(validate_group_name("").is_err());
        assert!(validate_group_name("a,b").is_err(), "逗号是渠道多分组分隔符");
        assert!(validate_group_name("a/b").is_err(), "斜杠会破坏 DELETE 路径参数");
        assert!(validate_group_name("a b").is_err());
        assert!(validate_group_name("auto").is_err(), "auto 为保留名");
        assert!(validate_group_name(&"x".repeat(65)).is_err());
    }

    #[test]
    fn validate_ratio_rules() {
        assert!(validate_ratio(1.0).is_ok());
        assert!(validate_ratio(0.5).is_ok());
        assert!(validate_ratio(0.0).is_err());
        assert!(validate_ratio(-1.0).is_err());
        assert!(validate_ratio(f64::NAN).is_err());
        assert!(validate_ratio(f64::INFINITY).is_err());
    }

    #[test]
    fn group_json_marks_usage_and_deletability() {
        let ratios = BTreeMap::from([("default".to_string(), 1.0), ("vip".to_string(), 0.8)]);
        let usable = BTreeMap::from([("default".to_string(), "默认分组".to_string())]);
        let in_use = BTreeSet::from(["vip".to_string()]);

        let vip = group_json("vip", &ratios, &usable, &in_use);
        assert_eq!(vip["ratio"], json!(0.8));
        assert_eq!(vip["usable"], json!(false), "不在 UserUsableGroups 即不可选");
        assert_eq!(vip["in_use"], json!(true));
        assert_eq!(vip["deletable"], json!(false), "使用中不可删");

        let default = group_json("default", &ratios, &usable, &in_use);
        assert_eq!(default["description"], json!("默认分组"));
        assert_eq!(default["usable"], json!(true));
        assert_eq!(default["deletable"], json!(false), "default 不可删");
    }

    #[test]
    fn group_json_defaults_ratio_to_one() {
        let item = group_json("new", &BTreeMap::new(), &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(item["ratio"], json!(1.0));
        assert_eq!(item["deletable"], json!(true), "未使用的普通分组可删");
    }
}
