//! 路由清单(自省)。
//!
//! **由测试驱动出的设计**:axum 不暴露路由枚举 API,因此注册时必须主动登记。
//! 见 test/cases/11-route-completeness.md。
//!
//! 三个用途:
//! 1. 与 CONTRACTS.md 附录 A 的 305 条清单**自动对表**,防漏注册防重复注册
//! 2. 遍历断言"受保护路由都挂了角色闸门",防越权漏洞
//! 3. 作为 `/api/performance` 的诊断输出

use std::sync::OnceLock;

/// 单条路由的登记信息。
#[derive(Debug, Clone)]
pub struct RouteEntry {
    pub method: String,
    pub path: String,
    /// 所属面:`api` / `relay` / `video` / `dashboard`。
    pub surface: String,
    /// 该路由是否应受鉴权保护(公开端点为 false)。
    pub requires_auth: bool,
    /// 实际挂载的角色闸门:`UserAuth` / `AdminAuth` / `RootAuth` / `TokenAuth` 等。
    pub auth_layer: Option<String>,
    /// 附加中间件:`SecureVerificationRequired` / `CriticalRateLimit` 等。
    pub extra_layers: Vec<String>,
    /// 是否为 `RelayNotImplemented` 占位端点(共 11 条)。
    pub is_placeholder: bool,
}

/// 全量路由清单。进程启动时构建一次。
#[derive(Debug, Default)]
pub struct RouteManifest {
    routes: Vec<RouteEntry>,
}

impl RouteManifest {
    pub fn routes(&self) -> &[RouteEntry] {
        &self.routes
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn count_by_surface(&self, surface: &str) -> usize {
        self.routes.iter().filter(|r| r.surface == surface).count()
    }

    pub fn find(&self, method: &str, path: &str) -> Option<&RouteEntry> {
        self.routes
            .iter()
            .find(|r| r.method.eq_ignore_ascii_case(method) && r.path == path)
    }

    pub fn paths_with_prefix(&self, prefix: &str) -> Vec<String> {
        self.routes
            .iter()
            .filter(|r| r.path.starts_with(prefix))
            .map(|r| r.path.clone())
            .collect()
    }

    /// 登记一条路由。由各 `router/*.rs` 在注册时调用。
    pub fn register(&mut self, entry: RouteEntry) {
        self.routes.push(entry);
    }
}

static MANIFEST: OnceLock<RouteManifest> = OnceLock::new();

/// 取全局路由清单。首次调用时构建。
pub fn route_manifest() -> &'static RouteManifest {
    MANIFEST.get_or_init(|| {
        todo!("在 router::build() 中同步登记,或此处独立构建一份与注册逻辑同源的清单")
    })
}

pub fn registered_route_count() -> usize {
    route_manifest().len()
}
