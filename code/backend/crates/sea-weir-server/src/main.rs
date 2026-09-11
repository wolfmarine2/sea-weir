//! sea-weir 服务入口。
//!
//! 启动装配顺序(doc/architecture/module-dependency.puml 的 main.rs note):
//! 1. clap 解析 `-c/--config`
//! 2. 配置加载(Nacos > env > YAML)
//! 3. tracing 初始化
//! 4. sqlx 双连接池(主库/日志库)+ migrations
//! 5. Valkey 客户端 + pub/sub 订阅
//! 6. 适配器注册表构建(缺失实现立即 panic,不留到线上)
//! 7. `Arc<AppState>` 组装
//! 8. 后台任务 spawn
//! 9. axum 服务启动
//! 10. `signal::ctrl_c()` 优雅关闭

// 模块定义在 lib.rs(见该文件说明):app_state / background / config / handlers /
// middleware / response / router。main() 装配时经 `sea_weir_server::...` 引用。

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "sea-weir-server", version)]
struct Cli {
    /// 配置文件路径。
    #[arg(short, long, default_value = "/etc/sea-weir/config.yaml")]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    todo!("按上方 10 步装配")
}
