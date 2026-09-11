//! 中继面渠道分发。对应 C4 组件 `distribute_middleware`。见 SEQ-002。
//!
//! 职责:解析目标模型与 relay 模式 → 亲和性命中? → 否则 channel_selector 选渠
//! → **注入渠道上下文** → 进入 relay_entry。
//!
//! 注入内容(CONTRACTS.md §10 渠道上下文注入约定):
//! `{channel_id, channel_type, base_url, key(含 multi-key 选中下标), model_mapping,
//!   param_override, header_override, status_code_mapping, auto_ban, setting}`
//!
//! 后置:请求成功(状态 < 400)后回写亲和性缓存。

pub fn layer() -> impl Clone {
    todo!("选渠并注入 RelayInfo 到请求扩展")
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 模型解析:请求体 model 字段、Gemini 路径 `:action` 形式、MJ 无 model 的情况
    // - [ ] 令牌 model_limits 不含目标模型 → 403 permission_error
    // - [ ] 无可用渠道 → 404 model_not_found
    // - [ ] 亲和性命中时跳过选路
    // - [ ] 注入的上下文字段完整(逐字段断言,漏一个就是线上事故)
    // - [ ] GET 类查询端点(如 `/v1/videos/:task_id`)不选渠道
}
