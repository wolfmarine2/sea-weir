use async_trait::async_trait;
use sea_weir_types::AppResult;

#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait OptionRepository: Send + Sync {
    async fn load_all(&self) -> AppResult<Vec<(String, String)>>;
    /// 后置:落库 → 进程内配置即时生效 → pub/sub 广播多节点失效。
    /// 带 `.` 的键(`xxx_setting.yyy`)走注册式分层配置组。
    async fn upsert(&self, key: &str, value: &str) -> AppResult<()>;
    async fn get(&self, key: &str) -> AppResult<Option<String>>;
}
