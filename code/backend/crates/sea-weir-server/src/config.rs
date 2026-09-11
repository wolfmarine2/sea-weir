//! 配置加载:Nacos > 环境变量 > 本地 YAML。
//!
//! 结构体定义在 `sea_weir_types::config`,此处只负责「取值来源与合并」。

use sea_weir_types::{config::AppConfig, AppResult};

pub async fn load(_path: &str) -> AppResult<AppConfig> {
    todo!("三层合并;Nacos 变更监听在 background 中订阅")
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 三层优先级正确
    // - [ ] Nacos 不可达时降级到 env + YAML 并告警(不 panic)
}
