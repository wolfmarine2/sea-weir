use async_trait::async_trait;
use sea_weir_types::{domain::Ability, AppResult};

#[cfg_attr(feature = "mock", mockall::automock)]
#[async_trait]
pub trait AbilityRepository: Send + Sync {
    /// 选路主查询:按 (group, model) 取可用渠道,按 priority 降序。
    async fn find_candidates(&self, group: &str, model: &str) -> AppResult<Vec<Ability>>;
    /// 分组下全部可用模型(用于 `/api/user/models`、`/v1/models`)。
    async fn list_models_by_group(&self, group: &str) -> AppResult<Vec<String>>;
    async fn list_all_groups(&self) -> AppResult<Vec<String>>;
    /// 缺失模型检测:abilities 有而 models 表无。
    async fn find_missing_models(&self) -> AppResult<Vec<String>>;
}
