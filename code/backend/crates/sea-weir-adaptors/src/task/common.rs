//! 任务适配器共性逻辑。

/// 从上游任务响应中提取平台任务 id。
pub fn extract_task_id(_raw: &serde_json::Value, _path: &str) -> Option<String> {
    todo!("按各平台 JSON 路径提取")
}

/// 时长 / 分辨率 → 计费修正因子的通用换算。
pub fn duration_ratio(_seconds: f64, _base_seconds: f64) -> f64 {
    todo!("线性或阶梯换算,按平台配置")
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] extract_task_id 对各平台真实响应样本的提取
    // - [ ] duration_ratio 边界:0 秒、恰好基准时长、超长
}
