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

        let main = PgPoolOptions::new()
            .max_connections(cfg.max_connections.max(1))
            .acquire_timeout(Duration::from_secs(10))
            .connect(&cfg.dsn)
            .await
            .map_err(|e| AppError::Database(format!("主库连接失败: {e}")))?;

        let log = match log_dsn {
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

/// 兼容模式自愈。目标库若为 openGauss Oracle 兼容模式(`''` 等同 NULL)且尚无业务数据
/// (`users` 表不存在或为空),用同一 DSN 账号连维护库 `postgres` 重建为 PG 兼容库后再
/// 返回,使首装无需人工介入;已有业务数据则返回错误,绝不自动丢数据。
///
/// 兼容模式建库后不可修改,只能 `DROP DATABASE` 重建,故该检查必须在建立主连接池之前
/// (池一旦连上目标库,DROP 会因存在活动连接而失败)。
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

    // 短连接探测空串语义。用完立即关闭,否则随后的 DROP DATABASE 会失败。
    let mut probe = match PgConnection::connect_with(&opts).await {
        Ok(c) => c,
        // 连不上时不做自愈,交由主连接池给出原始错误。
        Err(_) => return Ok(()),
    };
    let empty_is_null: bool = sqlx::query_scalar("SELECT '' IS NULL")
        .fetch_one(&mut probe)
        .await
        .map_err(|e| AppError::Database(format!("兼容模式探测失败: {e}")))?;
    if !empty_is_null {
        probe.close().await.ok();
        return Ok(());
    }

    // Oracle 兼容:仅当无业务数据时才自动重建。
    let users: i64 = sqlx::query_scalar("SELECT count(*)::BIGINT FROM users")
        .fetch_one(&mut probe)
        .await
        .unwrap_or(0);
    probe.close().await.ok();

    if users > 0 {
        return Err(AppError::Database(format!(
            "数据库 {db_name} 为 Oracle 兼容模式(空串 '' 被视为 NULL),且已有 {users} 个用户;\
             不会自动重建,请人工迁移到 PG 兼容库(见 data/00-init-database.sql)"
        )));
    }

    rebuild_as_pg_compatible(&opts, &db_name).await
}

/// 以维护库 `postgres` 连接重建目标库。要求 DSN 账号具备 CREATEDB 权限;
/// 权限不足或无法连维护库时返回可直接照做的排查信息。
///
/// 两处并发防护:进入时取咨询锁串行化多副本;完成判定不看"我这条 DDL 成没成功",
/// 而看"目标库现在是不是 PG 语义",因此别的副本/部署脚本抢先建好时本副本同样算成功、
/// 不会降级。字面量先用探测库验证,避免"删了目标库却建不回来"。
async fn rebuild_as_pg_compatible(opts: &PgConnectOptions, db_name: &str) -> AppResult<()> {
    let mut maint = PgConnection::connect_with(&opts.clone().database("postgres"))
        .await
        .map_err(|e| {
            AppError::Database(format!(
                "数据库 {db_name} 为 Oracle 兼容模式且无数据,需重建,但无法以当前账号连接维护库 postgres: {e};\
                 请用超级用户执行 `DROP DATABASE {db_name}; CREATE DATABASE {db_name} DBCOMPATIBILITY 'PG';`(见 data/00-init-database.sql)"
            ))
        })?;

    let ident = quote_ident(&mut maint, db_name).await?;

    // 多副本串行化:同一 key 的启动自愈互斥,避免两个 Pod 同时 DROP/CREATE 互相踩。
    // 个别版本无此函数时也不报错,由下方的幂等判定兜底;会话级锁随连接关闭释放。
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(ADVISORY_LOCK_KEY)
        .execute(&mut maint)
        .await
        .ok();

    // 已被其它副本重建则直接跳过,避免把刚建好的空库再删一次。
    if skip_if_rebuilt(&mut maint, db_name).await {
        maint.close().await.ok();
        return Ok(());
    }

    // openGauss 各版本对 DBCOMPATIBILITY 字面量支持不一:'PG' 与 'D' 均表示 PostgreSQL。
    // 用一次性探测库逐个验证,取第一个可用者;探测库名含时间戳,避免多副本同名冲突。
    let probe_name = probe_db_name(db_name);
    let probe_ident = quote_ident(&mut maint, &probe_name).await?;
    let mut last_err = String::new();
    let mut usable: Option<&str> = None;
    for compat in ["PG", "D"] {
        sqlx::query(&format!("DROP DATABASE IF EXISTS {probe_ident}"))
            .execute(&mut maint)
            .await
            .ok();
        match sqlx::query(&format!("CREATE DATABASE {probe_ident} DBCOMPATIBILITY '{compat}'"))
            .execute(&mut maint)
            .await
        {
            Ok(_) => {
                usable = Some(compat);
                break;
            }
            Err(e) => last_err = e.to_string(),
        }
    }
    let Some(compat) = usable else {
        maint.close().await.ok();
        return Err(AppError::Database(format!(
            "无法自动重建数据库 {db_name}:当前 openGauss 不接受 DBCOMPATIBILITY 'PG'/'D'({last_err});\
             请用超级用户手工执行 `CREATE DATABASE {db_name} DBCOMPATIBILITY 'PG';`(见 data/00-init-database.sql)"
        )));
    };
    sqlx::query(&format!("DROP DATABASE IF EXISTS {probe_ident}"))
        .execute(&mut maint)
        .await
        .ok();

    // 探测期间另一副本/部署脚本可能已完成重建,删库前最后一次确认。
    if skip_if_rebuilt(&mut maint, db_name).await {
        maint.close().await.ok();
        return Ok(());
    }

    // DROP 前断开目标库上的其它连接;仍被占用时短暂等待后重试。
    let mut drop_err = None;
    for attempt in 0..3 {
        terminate_backends(&mut maint, db_name).await;
        match sqlx::query(&format!("DROP DATABASE IF EXISTS {ident}"))
            .execute(&mut maint)
            .await
        {
            Ok(_) => {
                drop_err = None;
                break;
            }
            Err(e) => {
                drop_err = Some(e.to_string());
                if attempt < 2 {
                    // 启动期一次性路径,短暂阻塞可接受(tokio 非本 crate 运行时依赖)。
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
        }
    }
    if let Some(e) = drop_err {
        // 另一种可能:别的实例已把库删了又建好 —— 目标库现在是 PG 语义即算完成。
        if target_is_pg_compatible(opts).await {
            tracing::info!(database = %db_name, "DROP 未成功但目标库已为 PG 兼容(其它实例已重建),视为完成");
            maint.close().await.ok();
            return Ok(());
        }
        return Err(AppError::Database(format!(
            "自动重建失败(DROP DATABASE {db_name} 需 CREATEDB 权限或库仍被占用): {e};\
             请用超级用户执行 `DROP DATABASE {db_name}; CREATE DATABASE {db_name} DBCOMPATIBILITY '{compat}';`"
        )));
    }

    if let Err(e) = sqlx::query(&format!("CREATE DATABASE {ident} DBCOMPATIBILITY '{compat}'"))
        .execute(&mut maint)
        .await
    {
        // CREATE 报错最常见的原因是另一实例抢先建好了同名库(duplicate key):只要目标库
        // 现在可连且为 PG 语义,就是期望结果,不应让本副本降级。
        if target_is_pg_compatible(opts).await {
            tracing::info!(database = %db_name, "目标库已由其它实例创建(PG 兼容),视为完成");
            maint.close().await.ok();
            return Ok(());
        }
        return Err(AppError::Database(format!(
            "自动重建失败(CREATE DATABASE {db_name} 需 CREATEDB 权限): {e};\
             请用超级用户执行 `CREATE DATABASE {db_name} DBCOMPATIBILITY '{compat}';`"
        )));
    }

    maint.close().await.ok();
    tracing::warn!(
        database = %db_name,
        compatibility = %compat,
        "检测到 Oracle 兼容模式空库,已自动重建为 PostgreSQL 兼容(兼容模式建库后不可改)"
    );
    Ok(())
}

/// 启动自愈的咨询锁 key(ASCII "seaweir"),多副本共用同一 key 以串行化重建。
const ADVISORY_LOCK_KEY: i64 = 0x7365_6177_6569_72;

/// 目标库已被(其它实例)重建为非 Oracle 兼容时返回 true。
async fn skip_if_rebuilt(conn: &mut PgConnection, db_name: &str) -> bool {
    let rebuilt = db_compat(conn, db_name).await.as_deref().is_some_and(|c| c != "A");
    if rebuilt {
        tracing::info!(database = %db_name, "目标库已被其它实例重建为兼容模式,跳过自愈");
    }
    rebuilt
}

/// 断开目标库上的其它连接,使 DROP DATABASE 不被占用。
async fn terminate_backends(conn: &mut PgConnection, db_name: &str) {
    sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE datname = $1 AND pid <> pg_backend_pid()",
    )
    .bind(db_name)
    .execute(&mut *conn)
    .await
    .ok();
}

/// 目标库当前是否可连且为 PG 语义(空串非 NULL)。用于"其它实例抢先完成重建"时的幂等判定。
async fn target_is_pg_compatible(opts: &PgConnectOptions) -> bool {
    let Ok(mut conn) = PgConnection::connect_with(opts).await else {
        return false;
    };
    let empty_is_null: Result<bool, sqlx::Error> =
        sqlx::query_scalar("SELECT '' IS NULL").fetch_one(&mut conn).await;
    conn.close().await.ok();
    matches!(empty_is_null, Ok(false))
}

/// 用数据库端 `quote_ident` 生成安全标识符,避免库名直接拼接 SQL。
async fn quote_ident(conn: &mut PgConnection, name: &str) -> AppResult<String> {
    sqlx::query_scalar("SELECT quote_ident($1)")
        .bind(name)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| AppError::Database(format!("构造数据库标识符失败: {e}")))
}

/// 读取目标库的兼容模式(openGauss 专有列)。查询失败返回 None,按"未知"处理。
async fn db_compat(conn: &mut PgConnection, db_name: &str) -> Option<String> {
    sqlx::query_scalar("SELECT datcompatibility FROM pg_database WHERE datname = $1")
        .bind(db_name)
        .fetch_optional(&mut *conn)
        .await
        .ok()
        .flatten()
}

/// 探测库名。openGauss 标识符上限 63 字节:前缀按字节截到 24,再拼
/// `_probe_<pid>_<纳秒时间戳>_<序号>`(≤59 字节)。容器内 PID 恒为 1,故时间戳+序号
/// 才是跨副本区分项;`pop()` 保证不切断多字节字符。
fn probe_db_name(db_name: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);

    let mut base = db_name.to_string();
    while base.len() > 24 {
        base.pop();
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() % 1_000_000_000_000);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{base}_probe_{}_{nonce}_{seq}", std::process::id())
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

    /// 探测库名带进程号与时间戳、不超过 63 字节标识符上限;多字节库名不切断字符。
    #[test]
    fn probe_db_name_is_bounded() {
        let a = probe_db_name("sea_weir");
        assert!(a.contains(&format!("_probe_{}_", std::process::id())));
        assert!(probe_db_name(&"x".repeat(80)).len() <= 63);
        let wide = probe_db_name(&"数据".repeat(40));
        assert!(wide.len() <= 63, "多字节库名应 ≤ 63 字节: {wide}");
        assert!(wide.is_char_boundary(wide.len()));
        // 同一进程连续调用也必须不同名,否则多副本(PID 恒为 1)会撞名。
        assert_ne!(probe_db_name("sea_weir"), a);
    }
}
