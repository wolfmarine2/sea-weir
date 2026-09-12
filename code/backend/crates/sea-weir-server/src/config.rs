//! 配置加载:环境变量 > 本地 YAML fallback。
//!
//! 结构体定义在 `sea_weir_types::config`,此处只负责「取值来源与合并」。
//! 说明:原设计三层里的「Nacos 下发」已从本地部署移除(见 cicd/README.md),
//! 运行期配置改由部署方直接经环境变量注入;YAML 仅作缺省模板。

use sea_weir_types::{config::AppConfig, AppError, AppResult};
use tracing_subscriber::{fmt, EnvFilter};

/// 内置缺省配置,与 `code/backend/config/config.example.yaml` 同源。
/// 容器内 `/etc/sea-weir/config.yaml` 缺失时用它兜底,保证结构完整可反序列化,
/// 再由环境变量覆盖 —— 避免因缺一个字段就启动失败。
const DEFAULT_CONFIG_YAML: &str = include_str!("../../../config/config.example.yaml");

/// 加载配置:读取 `path`(不存在时用内置模板)→ 反序列化 → 环境变量覆盖。
pub async fn load(path: &str) -> AppResult<AppConfig> {
    let raw = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("[warn] 配置文件 {path} 不存在,使用内置默认模板");
            DEFAULT_CONFIG_YAML.to_string()
        }
        Err(e) => return Err(AppError::Config(format!("读取配置文件 {path} 失败: {e}"))),
    };

    let source = config::Config::builder()
        .add_source(config::File::from_str(&raw, config::FileFormat::Yaml))
        .build()
        .map_err(|e| AppError::Config(format!("解析配置文件失败: {e}")))?;

    let mut cfg: AppConfig = source
        .try_deserialize()
        .map_err(|e| AppError::Config(format!("配置结构与 AppConfig 不匹配: {e}")))?;

    apply_env_overrides(&mut cfg)?;
    Ok(cfg)
}

/// 初始化 tracing。`RUST_LOG` 优先,其次回落到 `telemetry.log_level`。
pub fn init_tracing(cfg: &AppConfig) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(cfg.telemetry.log_level.clone()));

    let builder = fmt().with_env_filter(filter);
    // 已存在全局订阅者时(测试等)忽略错误,不 panic。
    let _ = if cfg.telemetry.json_format {
        builder.json().try_init()
    } else {
        builder.try_init()
    };
}

/// 取非空环境变量(空串视为未设置)。
fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// 环境变量覆盖。命名与部署清单 `cicd/kustomize/base/deployment.yaml` 的 env 对齐。
fn apply_env_overrides(cfg: &mut AppConfig) -> AppResult<()> {
    if let Some(v) = env_nonempty("DATABASE_DSN") {
        cfg.database.dsn = v;
    }
    if let Some(v) = env_nonempty("DATABASE_LOG_DSN") {
        cfg.database.log_dsn = Some(v);
    }
    if let Some(v) = env_nonempty("CACHE_URL") {
        cfg.cache.url = v;
    }
    if let Some(v) = env_nonempty("SESSION_SECRET") {
        cfg.session.secret = v;
    }
    if let Some(v) = env_nonempty("CRYPTO_SECRET") {
        cfg.session.crypto_secret = v;
    }
    if let Some(v) = env_nonempty("SESSION_TTL_DAYS") {
        cfg.session.ttl_days = v
            .parse()
            .map_err(|_| AppError::Config(format!("SESSION_TTL_DAYS 不是整数: {v}")))?;
    }
    if let Some(v) = env_nonempty("SERVER_PORT") {
        cfg.server.port = v
            .parse()
            .map_err(|_| AppError::Config(format!("SERVER_PORT 不是端口: {v}")))?;
    }
    if let Some(v) = env_nonempty("METRICS_PORT") {
        cfg.server.metrics_port = Some(
            v.parse()
                .map_err(|_| AppError::Config(format!("METRICS_PORT 不是端口: {v}")))?,
        );
    }
    if let Some(v) = env_nonempty("RUST_LOG") {
        cfg.telemetry.log_level = v;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_template_deserializes() {
        let source = config::Config::builder()
            .add_source(config::File::from_str(
                DEFAULT_CONFIG_YAML,
                config::FileFormat::Yaml,
            ))
            .build()
            .expect("模板应可解析");
        let cfg: AppConfig = source.try_deserialize().expect("模板应与 AppConfig 匹配");
        assert_eq!(cfg.server.port, 8080);
    }

    #[test]
    fn blank_env_is_absent() {
        std::env::set_var("SEA_WEIR_TEST_EMPTY", "  ");
        assert!(env_nonempty("SEA_WEIR_TEST_EMPTY").is_none());
        std::env::remove_var("SEA_WEIR_TEST_EMPTY");
    }
}
