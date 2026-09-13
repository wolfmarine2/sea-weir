//! sea-weir 服务入口。
//!
//! 启动装配顺序(doc/architecture/module-dependency.puml 的 main.rs note):
//! 1. clap 解析 `-c/--config`                              ✅
//! 2. 配置加载(env > YAML;Nacos 层已从本地部署移除)         ✅
//! 3. tracing 初始化                                        ✅
//! 4. sqlx 双连接池(主库/日志库)+ migrations                ✅(连接池;migrations 见 pool.rs)
//! 5. Valkey 客户端 + pub/sub 订阅                          ⏳ TODO(TDD)
//! 6. 适配器注册表构建                                     ⏳ TODO(TDD)
//! 7. `Arc<AppState>` 组装                                 ⏳ TODO(TDD;当前用 ServerState 承载已落地端点)
//! 8. 后台任务 spawn                                        ⏳ TODO(TDD)
//! 9. axum 服务启动                                        ✅
//! 10. `signal::ctrl_c()` 优雅关闭                          ✅
//!
//! 已落地端点:状态、首装、登录、登出、本人信息;其余路径返回 501。
//! 四路由面与其余 Repository 实现补齐后,改为 `router::build(state)`。

use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use clap::Parser;

use sea_weir_repository::pg::{PgChannelRepository, PgOptionRepository, PgTokenRepository, PgUserRepository};
use sea_weir_repository::{
    ChannelRepository, DbPools, OptionRepository, RepositoryContext, TokenRepository, UserRepository,
};
use sea_weir_server::app_state::ServerState;
use sea_weir_server::handlers;
use sea_weir_server::response;
use sea_weir_server::session::SessionSigner;
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
    let port = config.server.port;

    // 3. tracing
    sea_weir_server::config::init_tracing(&config);
    let started = *START_TIME.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    });
    tracing::info!(version = env!("CARGO_PKG_VERSION"), started, "sea-weir-server 启动");

    // 4. 主库/日志库连接池 + 已实现的 Repository(数据库不可用时降级启动)
    let (users, options, tokens, channels) = connect_repositories(&config).await;

    // 5. Valkey 客户端 + pub/sub:TODO(TDD)。当前仅记录配置来源,未建立连接。
    tracing::info!(cache_url = %config.cache.url, "缓存配置已加载(Valkey 客户端待接入)");

    // 9. HTTP 服务
    let state = Arc::new(ServerState {
        sessions: SessionSigner::new(&config.session),
        config,
        users,
        options,
        tokens,
        channels,
        started,
    });

    let app = build_router(state);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "HTTP 监听中");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("已优雅关闭");
    Ok(())
}

/// 建立连接池并装配已实现的 Repository;不可用或未配置时返回 `(None, None, None)`。
async fn connect_repositories(
    config: &AppConfig,
) -> (
    Option<Arc<dyn UserRepository>>,
    Option<Arc<dyn OptionRepository>>,
    Option<Arc<dyn TokenRepository>>,
    Option<Arc<dyn ChannelRepository>>,
) {
    let dsn = config.database.dsn.trim();
    if dsn.is_empty() || dsn.contains("CHANGE_ME") {
        tracing::warn!("database.dsn 未配置(仍为占位值),跳过数据库连接");
        return (None, None, None, None);
    }

    let pools = match DbPools::connect(&config.database).await {
        Ok(pools) => pools,
        Err(e) => {
            tracing::warn!(error = %e, "数据库不可用,以降级模式启动");
            return (None, None, None, None);
        }
    };

    if let Err(e) = pools.migrate().await {
        tracing::warn!(error = %e, "migrations 执行失败(继续启动)");
    }
    tracing::info!("数据库连接池就绪");

    let ctx = Arc::new(RepositoryContext {
        pools,
        cache: sea_weir_repository::pool::cache_client::CacheClient,
    });
    (
        Some(Arc::new(PgUserRepository::new(ctx.clone()))),
        Some(Arc::new(PgOptionRepository::new(ctx.clone()))),
        Some(Arc::new(PgTokenRepository::new(ctx.clone()))),
        Some(Arc::new(PgChannelRepository::new(ctx))),
    )
}

/// 路由装配。已落地端点 + 健康探针;其余返回 501。
fn build_router(state: Arc<ServerState>) -> Router {
    Router::new()
        // 公开:全站状态 / 首装向导
        .route("/api/status", get(handlers::system::status))
        .route("/api/uptime/status", get(uptime_status))
        .route(
            "/api/setup",
            get(handlers::system::get_setup).post(handlers::system::post_setup),
        )
        // 认证
        .route("/api/user/login", post(handlers::user::login))
        .route("/api/user/logout", get(handlers::user::logout))
        // 受保护:本人信息(UserAuth + New-Api-User 防串号)
        .route("/api/user/self", get(handlers::user::self_info))
        // 令牌管理(UserAuth)。契约:PUT 用 /api/token/(id 在 body),DELETE 用 /:id
        .route(
            "/api/token/",
            get(handlers::token::list)
                .post(handlers::token::create)
                .put(handlers::token::update),
        )
        .route("/api/token", get(handlers::token::list).put(handlers::token::update))
        .route("/api/token/search", get(handlers::token::list))
        .route("/api/token/{id}", delete(handlers::token::delete))
        .route("/api/token/{id}/key", post(handlers::token::reveal_key))
        // 渠道管理(AdminAuth)
        .route(
            "/api/channel/",
            get(handlers::channel::list)
                .post(handlers::channel::create)
                .put(handlers::channel::update),
        )
        .route(
            "/api/channel",
            get(handlers::channel::list).put(handlers::channel::update),
        )
        .route("/api/channel/search", get(handlers::channel::list))
        .route(
            "/api/channel/{id}",
            get(handlers::channel::get).delete(handlers::channel::delete),
        )
        // K8s 探针
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .fallback(not_implemented)
        .with_state(state)
    // TODO(TDD): 合并 router::build(state) 的四路由面
}

async fn uptime_status() -> Response {
    response::ok(serde_json::json!({ "status": "up" }))
}

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz(State(state): State<Arc<ServerState>>) -> Response {
    if state.db_ready() {
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
