//! sea-weir 服务入口。
//!
//! 启动装配顺序(doc/architecture/module-dependency.puml 的 main.rs note):
//! 1. clap 解析 `-c/--config`                              ✅
//! 2. 配置加载(env > YAML;Nacos 层已从本地部署移除)         ✅
//! 3. tracing 初始化                                        ✅
//! 4. sqlx 双连接池(主库/日志库)+ migrations                ✅(连接池;migrations 见 pool.rs)
//! 5. Valkey 客户端 + pub/sub 订阅                          ⏳ TODO(TDD)
//! 6. 适配器注册表构建                                     ⏳ TODO(TDD)
//! 7. `Arc<AppState>` 组装                                 ⏳ TODO(TDD,依赖 6/8 与 Repository 实现)
//! 8. 后台任务 spawn                                        ⏳ TODO(TDD)
//! 9. axum 服务启动                                        ✅(当前为引导路由:健康/状态)
//! 10. `signal::ctrl_c()` 优雅关闭                          ✅
//!
//! 说明:骨架阶段 `router::build` / `background::spawn_all` 及其依赖的 Repository
//! 实现尚未落地(见各文件 TODO(TDD))。为保证容器可启动、探针可用,这里先起一个
//! **引导路由**(`/api/status`、`/healthz`、`/readyz`),其余路径返回 501;
//! 待四路由面与 Repository 实现补齐后,改为 `router::build(state)` 即可。

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use clap::Parser;
use sea_weir_repository::DbPools;
use sea_weir_server::response;
use sea_weir_types::config::AppConfig;
use sea_weir_types::dto::common::ApiResponse;

#[derive(Parser, Debug)]
#[command(name = "sea-weir-server", version)]
struct Cli {
    /// 配置文件路径。
    #[arg(short, long, default_value = "/etc/sea-weir/config.yaml")]
    config: String,
}

/// 进程启动时刻(unix 秒),用于 `/api/status` 的 `start_time`。
static START_TIME: OnceLock<u64> = OnceLock::new();

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 命令行
    let cli = Cli::parse();

    // 2. 配置加载
    let config = sea_weir_server::config::load(&cli.config).await?;

    // 3. tracing
    sea_weir_server::config::init_tracing(&config);
    let started = *START_TIME.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    });
    tracing::info!(version = env!("CARGO_PKG_VERSION"), started, "sea-weir-server 启动");

    // 4. 主库/日志库连接池(尽力而为:失败仅告警,不阻塞进程启动)
    let db_ready = match connect_pools(&config).await {
        Some(pools) => {
            drop(pools); // 业务层接入前不持有连接,避免空闲连接被回收告警
            true
        }
        None => false,
    };

    // 5. Valkey 客户端 + pub/sub:TODO(TDD)。当前仅记录配置来源,避免误连。
    tracing::info!(cache_url = %config.cache.url, "缓存配置已加载(Valkey 客户端待接入)");

    // 9. HTTP 服务
    let app = bootstrap_router(db_ready, started);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.server.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "HTTP 监听中");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("已优雅关闭");
    Ok(())
}

/// 建立连接池并执行 migrations;不可用或未配置时返回 `None`(降级启动)。
async fn connect_pools(config: &AppConfig) -> Option<DbPools> {
    let dsn = config.database.dsn.trim();
    if dsn.is_empty() || dsn.contains("CHANGE_ME") {
        tracing::warn!("database.dsn 未配置(仍为占位值),跳过数据库连接");
        return None;
    }

    match DbPools::connect(&config.database).await {
        Ok(pools) => {
            if let Err(e) = pools.migrate().await {
                tracing::warn!(error = %e, "migrations 执行失败(继续启动)");
            }
            tracing::info!("数据库连接池就绪");
            Some(pools)
        }
        Err(e) => {
            tracing::warn!(error = %e, "数据库不可用,以降级模式启动");
            None
        }
    }
}

/// 引导路由。业务面(四路由面共 305 条端点)待 TDD 落地后由 `router::build` 取代。
#[derive(Clone)]
struct BootstrapState {
    system_name: String,
    started: u64,
    db_ready: bool,
}

fn bootstrap_router(db_ready: bool, started: u64) -> Router {
    let state = BootstrapState {
        system_name: "sea-weir".to_string(),
        started,
        db_ready,
    };

    Router::new()
        // 公开:全站状态(前端 status store 的唯一来源,契约见 CONTRACTS.md 附录 A.5)
        .route("/api/status", get(status))
        .route("/api/uptime/status", get(uptime_status))
        // K8s 探针
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .fallback(not_implemented)
        .with_state(state)
    // TODO(TDD): 合并 router::build(state) 的四个路由面
}

async fn status(State(s): State<BootstrapState>) -> Response {
    response::ok(serde_json::json!({
        "system_name": s.system_name,
        "version": env!("CARGO_PKG_VERSION"),
        "start_time": s.started,
        "setup": false,
        "db_ready": s.db_ready,
    }))
}

async fn uptime_status() -> Response {
    response::ok(serde_json::json!({ "status": "up" }))
}

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz(State(s): State<BootstrapState>) -> Response {
    if s.db_ready {
        (StatusCode::OK, "ready").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "database not ready").into_response()
    }
}

/// 未实现的业务端点统一 501,避免骨架期返回误导性的成功响应。
async fn not_implemented() -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiResponse::<()>::fail(
            "endpoint not implemented yet (skeleton stage)",
        )),
    )
        .into_response()
}

/// 优雅关闭:SIGINT(ctrl-c)或 SIGTERM(K8s 删除 Pod)。
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("收到关闭信号,开始优雅关闭");
}
