use async_trait::async_trait;
use sea_weir_types::{domain::PrefillGroup, AppResult};

/// 预填分组仓储(soft delete;名称在未删除记录内唯一)。
#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait PrefillGroupRepository: Send + Sync {
    /// 全部未删除记录,按 id 倒序(管理面列表)。
    async fn list(&self) -> AppResult<Vec<PrefillGroup>>;
    async fn find_by_id(&self, id: i64) -> AppResult<Option<PrefillGroup>>;
    /// 名称是否已被占用(未删除记录内唯一)。
    async fn exists_name(&self, name: &str) -> AppResult<bool>;
    async fn create(&self, group: &PrefillGroup) -> AppResult<i64>;
    async fn update(&self, group: &PrefillGroup) -> AppResult<()>;
    /// 软删除(置 `deleted_at`),不物理删除。
    async fn delete(&self, id: i64) -> AppResult<()>;
}
