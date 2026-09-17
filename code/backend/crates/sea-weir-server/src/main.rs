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

use sea_weir_repository::pg::{
    PgChannelRepository, PgLogRepository, PgOptionRepository, PgPrefillGroupRepository,
    PgTokenRepository, PgUserRepository,
};
use sea_weir_repository::{
    ChannelRepository, DbPools, LogRepository, OptionRepository, PrefillGroupRepository,
    RepositoryContext, TokenRepository, UserRepository,
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
    let cache = connect_cache(&config).await;
    let repos = connect_repositories(&config, cache.clone()).await;
    if let Some(err) = repos.error.as_ref() {
        tracing::warn!(error = %err, "数据库不可用,以降级模式启动");
    }

    // 5. Valkey 客户端(不可用时降级:限流走内存窗口、会话吊销不可用)
    // (连接已在第 4 步前完成,见 connect_cache)

    // 9. HTTP 服务
    let state = Arc::new(ServerState {
        sessions: SessionSigner::new(&config.session),
        config,
        users: repos.users,
        options: repos.options,
        tokens: repos.tokens,
        channels: repos.channels,
        logs: repos.logs,
        prefill_groups: repos.prefill_groups,
        db_error: repos.error,
        pricing: sea_weir_server::pricing::PricingCache::new(),
        relay_limiter: sea_weir_server::middleware::rate_limit::SlidingWindowLimiter::new(
            relay_rpm(),
            60,
        ),
        cache,
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .unwrap_or_default(),
        started,
    });

    let app = build_router(state.clone());
    // 订阅缓存失效广播(多节点即时失效;未配置缓存时为空操作)。
    sea_weir_server::cache_listener::spawn(state.clone());
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "HTTP 监听中");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("已优雅关闭");
    Ok(())
}

/// 连接 Valkey;失败返回 None(限流降级内存窗口,会话吊销不可用)。
async fn connect_cache(
    config: &AppConfig,
) -> Option<sea_weir_repository::pool::cache_client::CacheClient> {
    let url = config.cache.url.trim();
    if url.is_empty() {
        tracing::warn!("cache.url 未配置,缓存不可用");
        return None;
    }
    match sea_weir_repository::pool::cache_client::CacheClient::connect(url).await {
        Ok(client) => {
            tracing::info!(cache_url = %url, "缓存(Valkey)连接就绪");
            Some(client)
        }
        Err(e) => {
            tracing::warn!(error = %e, cache_url = %url, "缓存不可用(降级:限流走内存窗口)");
            None
        }
    }
}

/// 数据层装配结果。任一 Repository 为 `None` 表示数据库不可用;
/// `error` 保留失败原因,经 `/api/status` 下发,便于直接看到"为什么连不上"。
struct RepoBundle {
    users: Option<Arc<dyn UserRepository>>,
    options: Option<Arc<dyn OptionRepository>>,
    tokens: Option<Arc<dyn TokenRepository>>,
    channels: Option<Arc<dyn ChannelRepository>>,
    logs: Option<Arc<dyn LogRepository>>,
    prefill_groups: Option<Arc<dyn PrefillGroupRepository>>,
    error: Option<String>,
}

impl RepoBundle {
    /// 全部为 `None` 并带上失败原因。
    fn unavailable(error: impl Into<String>) -> Self {
        Self {
            users: None,
            options: None,
            tokens: None,
            channels: None,
            logs: None,
            prefill_groups: None,
            error: Some(error.into()),
        }
    }
}

/// 建立连接池并装配已实现的 Repository;不可用或未配置时返回全 `None` + 原因。
async fn connect_repositories(
    config: &AppConfig,
    cache: Option<sea_weir_repository::pool::cache_client::CacheClient>,
) -> RepoBundle {
    let dsn = config.database.dsn.trim();
    if dsn.is_empty() || dsn.contains("CHANGE_ME") {
        return RepoBundle::unavailable("database.dsn 未配置(仍为占位值)");
    }

    let pools = match DbPools::connect(&config.database).await {
        Ok(pools) => pools,
        Err(e) => return RepoBundle::unavailable(e.to_string()),
    };

    if let Err(e) = pools.migrate().await {
        // 建表失败会让后续所有端点不可用(如 relation "users" does not exist),
        // 因此这里按 error 级别提示,并给出排查方向。
        tracing::error!(
            error = %e,
            "sqlx migrations 执行失败:数据库可能不可写或权限不足;请检查 DATABASE_DSN 与 openGauss 权限"
        );
    }
    tracing::info!("数据库连接池就绪");

    let ctx = Arc::new(RepositoryContext {
        pools,
        cache,
        cache_secret: config.session.crypto_secret.clone(),
    });
    RepoBundle {
        users: Some(Arc::new(PgUserRepository::new(ctx.clone()))),
        options: Some(Arc::new(PgOptionRepository::new(ctx.clone()))),
        tokens: Some(Arc::new(PgTokenRepository::new(ctx.clone()))),
        channels: Some(Arc::new(PgChannelRepository::new(ctx.clone()))),
        logs: Some(Arc::new(PgLogRepository::new(ctx.clone()))),
        prefill_groups: Some(Arc::new(PgPrefillGroupRepository::new(ctx))),
        error: None,
    }
}

/// 中继面每 IP 每分钟限额,可用 `SEA_WEIR_RELAY_RPM` 覆盖(默认 600)。
fn relay_rpm() -> usize {
    std::env::var("SEA_WEIR_RELAY_RPM")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|v| *v > 0)
        .unwrap_or(600)
}

/// 路由装配。已落地端点 + 健康探针;其余返回 501。
fn build_router(state: Arc<ServerState>) -> Router {
    // 中继面:挂 IP 限流中间件(超限 429 + Retry-After)。
    let relay = Router::new()
        .route(
            "/v1/chat/completions",
            post(handlers::relay::chat_completions),
        )
        .route("/v1/messages", post(handlers::relay::claude_messages))
        .route(
            "/v1/messages/count_tokens",
            post(handlers::relay::claude_count_tokens),
        )
        .route("/v1/models", get(handlers::relay::list_models))
        .route("/v1/models/{model}", get(handlers::relay::get_model))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            sea_weir_server::middleware::rate_limit::relay_limit,
        ));

    Router::new()
        // 公开:全站状态 / 首装向导
        .route("/api/status", get(handlers::system::status))
        .route("/api/uptime/status", get(uptime_status))
        .route(
            "/api/setup",
            get(handlers::system::get_setup).post(handlers::system::post_setup),
        )
        // 运营文案(公开)
        .route("/api/notice", get(handlers::system::notice))
        .route("/api/about", get(handlers::system::about))
        .route("/api/user-agreement", get(handlers::system::user_agreement))
        .route("/api/privacy-policy", get(handlers::system::privacy_policy))
        .route(
            "/api/home_page_content",
            get(handlers::system::home_page_content),
        )
        // 认证
        .route("/api/user/login", post(handlers::user::login))
        .route("/api/user/logout", get(handlers::user::logout))
        // 受保护:本人信息(UserAuth + New-Api-User 防串号)
        .route("/api/user/self", get(handlers::user::self_info))
        // 用户管理(AdminAuth;访问控制,不含充值)
        .route(
            "/api/user/",
            get(handlers::user::admin_list)
                .post(handlers::user::admin_create)
                .put(handlers::user::admin_update),
        )
        .route("/api/user/batch", post(handlers::user::admin_batch_create))
        .route(
            "/api/user",
            get(handlers::user::admin_list).put(handlers::user::admin_update),
        )
        .route(
            "/api/user/{id}",
            get(handlers::user::admin_get).delete(handlers::user::admin_delete),
        )
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
            "/api/channel/update_balance",
            get(handlers::channel::update_balance_all),
        )
        .route(
            "/api/channel/update_balance/{id}",
            get(handlers::channel::update_balance_by_id),
        )
        .route("/api/channel/fetch_models/{id}", get(handlers::channel::fetch_models_by_id))
        .route("/api/channel/types", get(handlers::channel::types))
        .route("/api/channel/test", get(handlers::channel::test_all))
        .route("/api/channel/test/{id}", get(handlers::channel::test_by_id))
        .route(
            "/api/channel/{id}",
            get(handlers::channel::get).delete(handlers::channel::delete),
        )
        // 消费日志
        .route("/api/log/self", get(handlers::log::self_logs))
        .route("/api/log/self/stat", get(handlers::log::self_stat))
        .route("/api/log/stat", get(handlers::log::stat))
        .route("/api/log/", get(handlers::log::all_logs))
        .route("/api/log", get(handlers::log::all_logs))
        .route("/api/log/search", get(handlers::log::all_logs))
        // 概览(AdminAuth)
        .route("/api/data/", get(handlers::dashboard::overview))
        .route("/api/data", get(handlers::dashboard::overview))
        // 定价:倍率配置快照 + 模型广场(公开)
        .route("/api/ratio_config", get(handlers::pricing::ratio_config))
        .route("/api/pricing", get(handlers::pricing::pricing))
        // 系统选项 / 倍率配置(RootAuth)
        .route(
            "/api/option/",
            get(handlers::option::list).put(handlers::option::update),
        )
        .route(
            "/api/option",
            get(handlers::option::list).put(handlers::option::update),
        )
        // 分组管理(AdminAuth):列表 / 新增 / 修改 / 删除
        .route(
            "/api/group/",
            get(handlers::group::list)
                .post(handlers::group::create)
                .put(handlers::group::update),
        )
        .route(
            "/api/group",
            get(handlers::group::list)
                .post(handlers::group::create)
                .put(handlers::group::update),
        )
        .route("/api/group/{name}", delete(handlers::group::delete))
        // 预填分组(model / tag / endpoint)(AdminAuth)
        .route(
            "/api/prefill_group",
            get(handlers::prefill_group::list)
                .post(handlers::prefill_group::create)
                .put(handlers::prefill_group::update),
        )
        .route(
            "/api/prefill_group/",
            get(handlers::prefill_group::list)
                .post(handlers::prefill_group::create)
                .put(handlers::prefill_group::update),
        )
        .route(
            "/api/prefill_group/{id}",
            delete(handlers::prefill_group::delete),
        )
        // 中继面(TokenAuth / sk-token;已挂 IP 限流)
        .merge(relay)
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
///
/// 消息里带上**方法与路径**:客户端报"连接失败"时,一眼能看出它打的是哪个端点,
/// 不必再靠猜(此前 Anthropic 客户端探 `/v1/messages/count_tokens` 就吃过这个亏)。
async fn not_implemented(method: axum::http::Method, uri: axum::http::Uri) -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiResponse::<()>::fail(format!(
            "endpoint not implemented yet (skeleton stage): {method} {}",
            uri.path()
        ))),
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
