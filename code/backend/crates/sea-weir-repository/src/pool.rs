//! 连接池装配。主库与日志库物理分离(逻辑上可同库)。

use std::str::FromStr;
use std::time::Duration;

use sea_weir_types::config::DatabaseConfig;
use sea_weir_types::{AppError, AppResult};
use sqlx::postgres::{PgConnectOptions, PgConnection, PgPoolOptions};
use sqlx::Connection;

/// 双连接池。日志表写入走独立 pool,避免日志洪峰挤占账务连接。
pub struct DbPools {
    pub main: sqlx::PgPool,
    /// 日志库。config 中 `log_dsn` 为空时与 `main` 指向同一实例。
    pub log: sqlx::PgPool,
}

impl DbPools {
    /// 建立主库/日志库连接池。`log_dsn` 为空时日志库复用主库 pool。
    ///
    /// 建池前先做兼容模式自愈(`ensure_compatible_database`):openGauss 若以 Oracle
    /// 兼容建库,`''` 等同 NULL,会在首装写 `users.email` 时报 not-null 约束错误;
    /// 该属性建库后不可修改,只能重建空库。
    pub async fn connect(cfg: &DatabaseConfig) -> AppResult<Self> {
        let log_dsn = cfg.log_dsn.as_deref().map(str::trim).filter(|s| !s.is_empty());

        ensure_compatible_database(&cfg.dsn).await?;
        if let Some(dsn) = log_dsn.filter(|s| *s != cfg.dsn.trim()) {
            ensure_compatible_database(dsn).await?;
        }

        let main = connect_pool(&cfg.dsn, cfg.max_connections, "主库").await?;

        let log = match log_dsn {
            Some(dsn) => connect_pool(dsn, cfg.log_max_connections, "日志库").await?,
            None => {
                tracing::info!("database.log_dsn 为空,日志库复用主库连接池");
                main.clone()
            }
        };

        ensure_pg_empty_string(&main, "主库").await?;
        if log_dsn.is_some() {
            ensure_pg_empty_string(&log, "日志库").await?;
        }

        Ok(Self { main, log })
    }

    /// 启动时执行 sqlx migrations(`code/backend/migrations/`)。
    ///
    /// `0001_init.sql` 是 `data/ddl.sql` 的镜像(幂等 DDL),因此对已手工建过表的库
    /// 重复执行也安全;对空库则自动建表,无需额外的 data 阶段。
    pub async fn migrate(&self) -> AppResult<()> {
        sqlx::migrate!("../../migrations")
            .run(&self.main)
            .await
            .map_err(|e| AppError::Database(format!("migrations 执行失败: {e}")))?;
        tracing::info!("sqlx migrations 执行完成");
        Ok(())
    }
}

/// 建池,带有限重试。
///
/// 启动瞬间的一次性失败不值得直接降级:实测出现过库刚被重建(或 openGauss 在
/// 处理 `CREATE DATABASE`)时主库连接报 `Operation not permitted (os error 1)`,
/// 而同一次启动里的探测连接是通的 —— 稍等重试即可成功。
async fn connect_pool(dsn: &str, max_connections: u32, label: &str) -> AppResult<sqlx::PgPool> {
    const ATTEMPTS: u32 = 5;
    let mut last = String::new();
    for attempt in 1..=ATTEMPTS {
        match PgPoolOptions::new()
            .max_connections(max_connections.max(1))
            .acquire_timeout(Duration::from_secs(10))
            .connect(dsn)
            .await
        {
            Ok(pool) => {
                if attempt > 1 {
                    tracing::info!(label, attempt, "{label}连接在第 {attempt} 次尝试后成功");
                }
                return Ok(pool);
            }
            Err(e) => {
                last = e.to_string();
                if attempt < ATTEMPTS {
                    let backoff_ms = 500 * u64::from(attempt);
                    tracing::warn!(
                        label,
                        attempt,
                        error = %e,
                        "{}连接失败,{backoff_ms}ms 后重试",
                        label
                    );
                    // 启动期一次性路径;tokio 非本 crate 的直接依赖,故用同步 sleep。
                    std::thread::sleep(Duration::from_millis(backoff_ms));
                }
            }
        }
    }
    Err(AppError::Database(format!(
        "{label}连接失败(已重试 {ATTEMPTS} 次): {last}"
    )))
}

/// 校验库的空串语义。openGauss 以 `DBCOMPATIBILITY 'A'`(Oracle 兼容,默认值之一)建库时
/// `''` 会被当作 NULL,所有写 `''` 的 NOT NULL 列都会报
/// `null value in column ... violates not-null constraint`(如首装写 users.email)。
/// 该属性建库后不可修改,只能在启动时拦下并提示重建库。
async fn ensure_pg_empty_string(pool: &sqlx::PgPool, label: &str) -> AppResult<()> {
    let (empty_is_null,): (bool,) = sqlx::query_as("SELECT '' IS NULL")
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::Database(format!("{label}兼容模式检测失败: {e}")))?;
    if empty_is_null {
        return Err(AppError::Database(format!(
            "{label}为 Oracle 兼容模式(空串 '' 被视为 NULL),sea-weir 需要 PG 兼容模式;\
             请以 `CREATE DATABASE <库名> DBCOMPATIBILITY 'PG'` 重建数据库(见 data/00-init-database.sql)"
        )));
    }
    Ok(())
}

/// 启动前检查目标库的空串语义(**只检测,不修改**)。
///
/// openGauss 以 `DBCOMPATIBILITY 'A'`(Oracle 兼容)建库时 `''` 等同 NULL,sea-weir 按
/// PG 语义设计,该模式下写 NOT NULL 空串必然失败;而兼容模式建库后不可修改,只能重建。
///
/// 这里刻意**不做 `DROP DATABASE`**:库是破坏性资源,而线上可能同时有多个角色动手
/// (部署脚本、openGauss postStart、两个后端副本),实测已导致库被反复重建、表结构丢失、
/// 连接落在重建窗口里失败。兼容模式的纠正统一归部署侧(`cicd/deploy.sh` 的兼容模式自检、
/// `data/00-init-database.sql`),应用只负责如实报错。
async fn ensure_compatible_database(dsn: &str) -> AppResult<()> {
    let opts = PgConnectOptions::from_str(dsn)
        .map_err(|e| AppError::Database(format!("DATABASE_DSN 解析失败: {e}")))?;
    // 库名取自 DSN;未指定或指向维护库本身时无事可做。
    let Some(db_name) = opts.get_database().map(str::to_owned).filter(|d| !d.is_empty()) else {
        return Ok(());
    };
    if db_name == "postgres" {
        return Ok(());
    }

    // 短连接探测空串语义;连不上时交给建池阶段报原始错误,避免同一故障报两次。
    let mut probe = match PgConnection::connect_with(&opts).await {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let empty_is_null: bool = sqlx::query_scalar("SELECT '' IS NULL")
        .fetch_one(&mut probe)
        .await
        .map_err(|e| AppError::Database(format!("兼容模式探测失败: {e}")))?;
    probe.close().await.ok();

    if empty_is_null {
        return Err(AppError::Database(format!(
            "数据库 {db_name} 为 Oracle 兼容模式(空串 '' 被视为 NULL),sea-weir 需要 PG 兼容模式;             该属性建库后不可修改,需以 `CREATE DATABASE {db_name} DBCOMPATIBILITY 'PG'` 重建             (部署侧: cicd/deploy.sh 的兼容模式自检,或 data/00-init-database.sql)。             应用不自动 DROP 数据库,以免与部署脚本/多副本并发重建冲突"
        )));
    }
    Ok(())
}
/// Repository 层共享上下文:连接池 + 缓存客户端。
/// 各 Repository 实现从中取用,避免逐个传参。
pub struct RepositoryContext {
    pub pools: DbPools,
    /// Valkey 客户端;未配置/连接失败时为 None(读路径各自回源 DB)。
    pub cache: Option<cache_client::CacheClient>,
    /// 令牌缓存键的 HMAC 密钥(保证明文 key 不入缓存)。
    pub cache_secret: String,
}

/// 缓存 TTL(ADR-007:用户/令牌 60s 兜底)。
pub const CACHE_TTL_SECS: i64 = 60;

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

    use super::*;

    /// 维护库本身不做自愈;不应尝试连接(无需数据库即可通过)。
    #[tokio::test]
    async fn compat_probe_skips_postgres_db() {
        let r = ensure_compatible_database("postgres://u:p@127.0.0.1:1/postgres").await;
        assert!(r.is_ok(), "postgres 库应直接跳过: {r:?}");
    }

    /// DSN 未指定库名时无事可做,不应报错。
    #[tokio::test]
    async fn compat_probe_skips_dsn_without_db() {
        let r = ensure_compatible_database("postgres://u:p@127.0.0.1:1").await;
        assert!(r.is_ok(), "无库名应跳过: {r:?}");
    }

    /// 连不上目标库时不做自愈(交由主连接池报原始错误),且必须是 Ok 而非误删库。
    #[tokio::test]
    async fn compat_probe_skips_unreachable_target() {
        let r =
            ensure_compatible_database("postgres://u:p@127.0.0.1:1/sea_weir?connect_timeout=1")
                .await;
        assert!(r.is_ok(), "连不上应跳过自愈: {r:?}");
    }

    /// 非法 DSN 必须显式报错,避免"静默跳过"掩盖配置问题。
    #[tokio::test]
    async fn compat_probe_rejects_malformed_dsn() {
        let r = ensure_compatible_database("not-a-dsn").await;
        assert!(matches!(r, Err(AppError::Database(_))), "非法 DSN 应报错: {r:?}");
    }

}
