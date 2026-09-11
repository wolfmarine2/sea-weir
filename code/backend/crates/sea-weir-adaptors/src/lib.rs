//! # sea-weir-adaptors
//!
//! 渠道适配层:`Adaptor` / `TaskAdaptor` trait 与注册表。
//! 35 个同步渠道适配器(`ApiType`)+ 10 家异步任务平台。
//!
//! **约束**:本 crate 不依赖 `sea-weir-repository` —— 渠道数据由 core 层注入
//! (`RelayInfo` 已携带 base_url / key / model_mapping 等全部上下文)。
//! 这条边界保证适配器可脱离数据库单测,是 TDD 的前提。

#![forbid(unsafe_code)]

pub mod api_type;
pub mod registry;
pub mod sync;
pub mod task;

pub use api_type::{ApiType, TaskPlatform};
pub use registry::AdaptorRegistry;

use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use sea_weir_types::{
    dto::{relay::UpstreamResponse, RelayInfo, RelayRequest, Usage},
    AppError, NewApiError,
};

/// 同步渠道适配器。
///
/// 契约见 doc/architecture/CONTRACTS.md §11。
/// 调用顺序:`init` → `get_request_url` → `setup_request_header` → `convert_request`
///          → `do_request` → `do_response`。
///
/// - **前置**:`info` 已完成渠道上下文注入与模型映射。
/// - **后置**:`do_response` 返回的 `Usage` 进入结算。
/// - **错误**:适配器内部错误须标 `local_error = true`(不触发渠道禁用/重试);
///   上游错误经归一化后可参与重试与自动禁用判定。
#[async_trait]
pub trait Adaptor: Send + Sync {
    fn init(&self, info: &RelayInfo) -> Result<(), AppError>;

    fn get_request_url(&self, info: &RelayInfo) -> Result<String, AppError>;

    fn setup_request_header(
        &self,
        headers: &mut HeaderMap,
        info: &RelayInfo,
    ) -> Result<(), AppError>;

    /// 入口协议 → 上游协议的请求体转换。
    fn convert_request(&self, req: RelayRequest, info: &RelayInfo) -> Result<Bytes, AppError>;

    async fn do_request(&self, info: &RelayInfo, body: Bytes)
        -> Result<UpstreamResponse, AppError>;

    /// 非流式:解析 usage。流式:驱动 `stream_pipe`,消费完后返回累计 usage。
    async fn do_response(
        &self,
        resp: UpstreamResponse,
        info: &mut RelayInfo,
    ) -> Result<Usage, NewApiError>;

    fn get_model_list(&self) -> &[String];

    fn channel_name(&self) -> &'static str;
}

/// 异步任务平台适配器(视频 / 音乐 / MJ)。
///
/// 与 `Adaptor` 的关键差异:**全额预扣、无信任旁路**,完成时按
/// `adjust_billing_on_complete` 补差或按 tokens 重算(见 SEQ-006)。
#[async_trait]
pub trait TaskAdaptor: Send + Sync {
    fn validate_request_and_set_action(&self, req: &mut TaskRequest) -> Result<(), AppError>;

    /// 按时长 / 分辨率等给出计费修正因子。
    fn estimate_billing(&self, req: &TaskRequest) -> Vec<OtherRatio>;

    /// 提交成功后的额度修正。
    fn adjust_billing_on_submit(&self, req: &TaskRequest, pre_consumed: i64) -> i64;

    /// 完成时的额度补差。返回 `None` 表示「按 tokens 重算」。
    fn adjust_billing_on_complete(&self, task: &sea_weir_types::domain::Task) -> Option<i64>;

    fn build_request(&self, info: &RelayInfo) -> Result<(String, HeaderMap, Bytes), AppError>;

    async fn fetch_task(&self, task_id: &str, info: &RelayInfo) -> Result<TaskResult, AppError>;

    fn parse_task_result(&self, raw: serde_json::Value) -> Result<TaskResult, AppError>;

    fn platform_name(&self) -> &'static str;
}

/// 任务提交请求的统一内部表示。
#[derive(Debug, Clone)]
pub struct TaskRequest {
    pub action: String,
    pub model: String,
    pub raw: serde_json::Value,
    /// remix / 变换类任务锁定的原任务渠道。
    pub origin_task_id: Option<String>,
}

/// 计费修正因子。
#[derive(Debug, Clone)]
pub struct OtherRatio {
    pub name: String,
    pub ratio: f64,
}

/// 任务回源结果。
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub status: sea_weir_types::domain::task::TaskStatus,
    pub progress: Option<String>,
    pub fail_reason: Option<String>,
    pub usage: Option<Usage>,
    pub raw: serde_json::Value,
}

#[cfg(test)]
mod tests {
    // TDD 入口(适配器是本项目最适合 TDD 的部分 —— 纯函数 + wiremock):
    // 每个适配器的标准测试套:
    // - [ ] get_request_url 在 base_url 为空/自定义/带路径三种情况下的拼接
    // - [ ] setup_request_header 注入正确的鉴权头,且 header_override 能覆盖
    // - [ ] convert_request 的黄金用例:固定输入 → 固定上游请求体(快照测试)
    // - [ ] do_response 非流式解析 usage;缺失 usage 时的兜底
    // - [ ] 上游 4xx/5xx → NewApiError 的 status/error_code 映射
    // - [ ] 适配器自身解析失败 → local_error=true(不误禁用渠道)
}
