//! 分层配置结构。
//!
//! 加载优先级(doc/system-design.md §9.2):**Nacos 下发 > 环境变量 > 本地 YAML fallback**。
//! 运行期业务配置(倍率/分组/支付等)不在此处 —— 它们在 `options` 表 + 进程内缓存,
//! 经管理面 API 热更新并由 Valkey pub/sub 广播失效。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub cache: CacheConfig,
    pub session: SessionConfig,
    pub relay: RelayConfig,
    pub telemetry: TelemetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 单协议单端口:管理面 + 中继面 + 兼容面同端口按路径分流。
    pub port: u16,
    /// 可选 pprof/metrics 侧口(仅集群内)。
    pub metrics_port: Option<u16>,
    /// 是否用 rust-embed 内嵌前端 dist(单容器形态);K8s 形态由 nginx 托管。
    pub embed_frontend: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub dsn: String,
    pub max_connections: u32,
    /// 日志库独立 DSN。为空时与主库同库(对应 new-api 的 `LOG_SQL_DSN` 语义)。
    pub log_dsn: Option<String>,
    pub log_max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub url: String,
    /// 缓存不可用时的降级策略。鉴权类 fail-close,非关键路径 fail-open(ADR-007)。
    pub fail_open_on_outage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// 签名会话 Cookie 密钥(K8s Secret 注入,不入 Nacos)。
    pub secret: String,
    pub ttl_days: i64,
    /// 渠道密钥等敏感字段的加密密钥。
    pub crypto_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// 最大尝试次数 = retry_times + 1。
    pub retry_times: u32,
    pub upstream_timeout_secs: u64,
    pub stream_idle_timeout_secs: u64,
    /// 出网 SSRF 防护:允许的目标网段/域名白名单策略。
    pub ssrf_allow_private_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub log_level: String,
    pub json_format: bool,
}

impl AppConfig {
    /// 三层合并加载。
    pub fn load(_config_path: Option<&str>) -> crate::AppResult<Self> {
        todo!("Nacos > env > YAML 三层合并;缺失必填项返回 AppError::Config")
    }
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 三层优先级:Nacos 覆盖 env,env 覆盖 YAML
    // - [ ] 缺失必填项(dsn/secret)返回 AppError::Config 而非 panic
    // - [ ] log_dsn 为 None 时回落主库 dsn
    // - [ ] secret 不出现在 Debug 输出中(防日志泄漏)
}
