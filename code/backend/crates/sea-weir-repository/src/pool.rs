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
    /// Valkey 客户端;未配置/连接失败时为 None(读路径各自回源 DB)。
    pub cache: Option<cache_client::CacheClient>,
}

/// Valkey(Redis 协议)客户端封装。
pub mod cache_client {
    use fred::prelude::*;

    /// 轻量封装:只暴露本项目需要的最小命令集。
    #[derive(Clone)]
    pub struct CacheClient {
        config: Config,
        inner: Client,
    }

    impl CacheClient {
        pub async fn connect(url: &str) -> Result<Self, String> {
            let config =
                Config::from_url(url).map_err(|e| format!("cache url 解析失败: {e}"))?;
            let client = Builder::from_config(config.clone())
                .build()
                .map_err(|e| format!("cache 客户端构建失败: {e}"))?;
            client
                .init()
                .await
                .map_err(|e| format!("cache 连接失败: {e}"))?;
            Ok(Self {
                config,
                inner: client,
            })
        }

        /// 构造订阅客户端(独立连接,用于 pub/sub;主客户端不被订阅阻塞)。
        pub async fn subscriber(&self) -> Result<fred::clients::SubscriberClient, String> {
            let subscriber = Builder::from_config(self.config.clone())
                .build_subscriber_client()
                .map_err(|e| format!("订阅客户端构建失败: {e}"))?;
            subscriber
                .init()
                .await
                .map_err(|e| format!("订阅客户端连接失败: {e}"))?;
            Ok(subscriber)
        }

        /// 发布失效广播(多节点即时失效)。
        pub async fn publish(&self, channel: &str, payload: &str) -> Result<(), String> {
            self.inner
                .publish::<i64, _, _>(channel, payload)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        }

        pub async fn get(&self, key: &str) -> Result<Option<String>, String> {
            self.inner
                .get::<Option<String>, _>(key)
                .await
                .map_err(|e| e.to_string())
        }

        pub async fn set_ex(&self, key: &str, value: &str, ttl_secs: i64) -> Result<(), String> {
            self.inner
                .set::<(), _, _>(
                    key,
                    value,
                    Some(Expiration::EX(ttl_secs.max(1))),
                    None,
                    false,
                )
                .await
                .map_err(|e| e.to_string())
        }

        pub async fn del(&self, key: &str) -> Result<(), String> {
            self.inner
                .del::<i64, _>(key)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        }

        pub async fn incr(&self, key: &str) -> Result<i64, String> {
            self.inner.incr::<i64, _>(key).await.map_err(|e| e.to_string())
        }

        pub async fn expire(&self, key: &str, secs: i64) -> Result<(), String> {
            self.inner
                .expire::<i64, _>(key, secs.max(1), None)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        }

        pub async fn ttl(&self, key: &str) -> Result<i64, String> {
            self.inner.ttl::<i64, _>(key).await.map_err(|e| e.to_string())
        }

        /// 固定窗口限流(多副本一致)。
        /// 返回 `Ok(Ok(()))` 放行;`Ok(Err(retry_after_secs))` 超限;`Err` 缓存故障。
        pub async fn check_window(
            &self,
            key: &str,
            limit: i64,
            window_secs: i64,
        ) -> Result<Result<(), i64>, String> {
            let count = self.incr(key).await?;
            if count == 1 {
                self.expire(key, window_secs).await?;
            }
            if count > limit {
                let ttl = self.ttl(key).await?;
                return Ok(Err(if ttl > 0 { ttl } else { window_secs }));
            }
            Ok(Ok(()))
        }
    }
}

#[cfg(test)]
mod tests {
    // TDD 入口(需要 testcontainers 起 openGauss):
    // - [ ] log_dsn 为 None 时 main 与 log 指向同一 pool
    // - [ ] migrate() 幂等:重复执行不报错
    // - [ ] 连接池上限生效
}
