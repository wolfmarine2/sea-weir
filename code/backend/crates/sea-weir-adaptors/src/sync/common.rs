//! 适配器共性逻辑。35 个实现共享,先写测试再抽取。

use sea_weir_types::dto::RelayInfo;

/// base_url 与路径拼接。处理空 base_url(用渠道默认)、尾斜杠、已含路径三种情况。
pub fn join_url(base: Option<&str>, default_base: &str, path: &str) -> String {
    let base = base.unwrap_or(default_base).trim_end_matches('/');
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    // base 已含 /v1 时避免重复拼接,如 base=https://x/v1 + path=/v1/chat/completions。
    if base.ends_with("/v1") {
        if let Some(rest) = path.strip_prefix("/v1") {
            return format!("{base}{rest}");
        }
    }
    format!("{base}{path}")
}

/// 应用渠道的 `header_override`。
///
/// 值为 `null` 表示删除该头,字符串表示覆盖/新增。
pub fn apply_header_override(headers: &mut http::HeaderMap, info: &RelayInfo) {
    let Some(serde_json::Value::Object(map)) = info.header_override.as_ref() else {
        return;
    };
    for (name, value) in map {
        let Ok(header_name) = http::HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        match value {
            serde_json::Value::Null => {
                headers.remove(&header_name);
            }
            serde_json::Value::String(s) => {
                if let Ok(v) = http::HeaderValue::from_str(s) {
                    headers.insert(header_name, v);
                }
            }
            other => {
                if let Ok(v) = http::HeaderValue::from_str(&other.to_string()) {
                    headers.insert(header_name, v);
                }
            }
        }
    }
}

/// 应用渠道的 `param_override` 到请求体(**深合并**,而非整体替换)。
pub fn apply_param_override(body: &mut serde_json::Value, info: &RelayInfo) {
    let Some(override_value) = info.param_override.as_ref() else {
        return;
    };
    deep_merge(body, override_value);
}

fn deep_merge(target: &mut serde_json::Value, patch: &serde_json::Value) {
    match (target, patch) {
        (serde_json::Value::Object(target_map), serde_json::Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                deep_merge(
                    target_map
                        .entry(key.clone())
                        .or_insert(serde_json::Value::Null),
                    value,
                );
            }
        }
        (target_slot, patch_value) => *target_slot = patch_value.clone(),
    }
}

/// 上游状态码经渠道 `status_code_mapping` 重映射。
///
/// 配置形如 `{"500":200}`;无配置或未命中时原样返回。
pub fn map_status_code(upstream: u16, mapping: Option<&str>) -> u16 {
    let Some(raw) = mapping else {
        return upstream;
    };
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(raw) else {
        return upstream;
    };
    map.get(&upstream.to_string())
        .and_then(|v| v.as_u64())
        .map(|v| v as u16)
        .unwrap_or(upstream)
}

#[cfg(test)]
mod tests {
    // TDD 入口(这里的测试收益最高 —— 一次覆盖 35 个适配器的共性):
    // - [ ] join_url:base 为 None / 带尾斜杠 / 已含 /v1 三种情况
    // - [ ] apply_header_override 能覆盖既有头,也能新增
    // - [ ] apply_param_override 是深合并而非整体替换
    // - [ ] map_status_code 无映射时原样返回
}
