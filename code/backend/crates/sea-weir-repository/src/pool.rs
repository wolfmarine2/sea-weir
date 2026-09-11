//! 连接池装配。主库与日志库物理分离(逻辑上可同库)。

use sea_weir_types::AppResult;

/// 双连接池。日志表写入走独立 pool,避免日志洪峰挤占账务连接。
pub struct DbPools {
    pub main: sqlx::PgPool,
    /// 日志库。config 中 `log_dsn` 为空时与 `main` 指向同一实例。
    pub log: sqlx::PgPool,
}

impl DbPools {
    pub async fn connect(_cfg: &sea_weir_types::config::DatabaseConfig) -> AppResult<Self> {
        todo!("建立主库/日志库连接池;log_dsn 为空时复用主库")
    }

    /// 启动时执行 sqlx migrations。
    pub async fn migrate(&self) -> AppResult<()> {
        todo!("运行 migrations/ 下的 DDL")
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
