//! 连接池装配。主库与日志库物理分离(逻辑上可同库)。

use std::time::Duration;

use sea_weir_types::config::DatabaseConfig;
use sea_weir_types::{AppError, AppResult};
use sqlx::postgres::PgPoolOptions;

/// 双连接池。日志表写入走独立 pool,避免日志洪峰挤占账务连接。
pub struct DbPools {
    pub main: sqlx::PgPool,
    /// 日志库。config 中 `log_dsn` 为空时与 `main` 指向同一实例。
    pub log: sqlx::PgPool,
}

impl DbPools {
    /// 建立主库/日志库连接池。`log_dsn` 为空时日志库复用主库 pool。
    pub async fn connect(cfg: &DatabaseConfig) -> AppResult<Self> {
        let main = PgPoolOptions::new()
            .max_connections(cfg.max_connections.max(1))
            .acquire_timeout(Duration::from_secs(10))
            .connect(&cfg.dsn)
            .await
            .map_err(|e| AppError::Database(format!("主库连接失败: {e}")))?;

        let log = match cfg.log_dsn.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(dsn) => PgPoolOptions::new()
                .max_connections(cfg.log_max_connections.max(1))
                .acquire_timeout(Duration::from_secs(10))
                .connect(dsn)
                .await
                .map_err(|e| AppError::Database(format!("日志库连接失败: {e}")))?,
            None => {
                tracing::info!("database.log_dsn 为空,日志库复用主库连接池");
                main.clone()
            }
        };

        Ok(Self { main, log })
    }

    /// 启动时执行 sqlx migrations。
    ///
    /// 现状:`migrations/` 目录目前只有 README,权威 DDL 在 `data/ddl.sql`
    /// (由数据阶段脚本 `data/cicd.sh` 应用),尚无 `{timestamp}_*.sql` 迁移文件,
    /// 故此处为 no-op。补齐迁移文件后改为 `sqlx::migrate!("../../migrations").run(&self.main)`。
    pub async fn migrate(&self) -> AppResult<()> {
        tracing::warn!(
            "未执行 sqlx migrations:仓库暂无迁移文件,建表由 data 阶段脚本负责(data/cicd.sh)"
        );
        Ok(())
    }
}

/// Repository 层共享上下文:连接池 + 缓存客户端。
/// 各 Repository 实现从中取用,避免逐个传参。
pub struct RepositoryContext {
    pub pools: DbPools,
    pub cache: cache_client::CacheClient,
}

/// 缓存客户端占位模块(TDD 阶段替换为 fred 具体类型)。
pub mod cache_client {
    pub struct CacheClient;
}

#[cfg(test)]
mod tests {
    // TDD 入口(需要 testcontainers 起 openGauss):
    // - [ ] log_dsn 为 None 时 main 与 log 指向同一 pool
    // - [ ] migrate() 幂等:重复执行不报错
    // - [ ] 连接池上限生效
}
