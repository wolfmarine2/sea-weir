//! 通用任务平台适配器(10 个平台的共性)。
//!
//! 各平台的差异集中在:提交路径、JSON 字段路径、进度/终态映射。
//! 骨架阶段实现通用形态,`parse_task_result` 按平台路径提取;
//! 平台专有逻辑在各自模块中覆盖。

use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use sea_weir_types::{domain::task::TaskStatus, AppError};

use crate::{TaskAdaptor, TaskPlatform, TaskRequest, TaskResult};

/// 通用异步任务适配器。
pub struct CompatTaskAdaptor {
    pub platform: TaskPlatform,
    pub name: &'static str,
    /// 提交/查询路径(各平台不同)。
    pub submit_path: &'static str,
}

impl CompatTaskAdaptor {
    fn usage_from(raw: &serde_json::Value) -> Option<sea_weir_types::dto::Usage> {
        raw.get("usage")
            .and_then(|u| serde_json::from_value(u.clone()).ok())
    }
}

#[async_trait]
impl TaskAdaptor for CompatTaskAdaptor {
    fn validate_request_and_set_action(&self, req: &mut TaskRequest) -> Result<(), AppError> {
        if req.action.is_empty() {
            return Err(AppError::BadRequest("任务缺少 action".into()));
        }
        Ok(())
    }

    fn estimate_billing(&self, _req: &TaskRequest) -> Vec<crate::OtherRatio> {
        // 时长/分辨率修正因子由平台专有实现提供。
        Vec::new()
    }

    fn adjust_billing_on_submit(&self, _req: &TaskRequest, pre_consumed: i64) -> i64 {
        pre_consumed
    }

    fn adjust_billing_on_complete(&self, _task: &sea_weir_types::domain::Task) -> Option<i64> {
        // None = 按 tokens 重算。
        None
    }

    fn build_request(
        &self,
        info: &sea_weir_types::dto::RelayInfo,
    ) -> Result<(String, HeaderMap, Bytes), AppError> {
        let url =
            super::super::sync::common::join_url(info.base_url.as_deref(), "", self.submit_path);
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
        if let Ok(value) = http::HeaderValue::from_str(&format!("Bearer {}", info.key)) {
            headers.insert(http::header::AUTHORIZATION, value);
        }
        let body = serde_json::to_vec(&serde_json::json!({
            "model": info.upstream_model,
        }))
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
        Ok((url, headers, Bytes::from(body)))
    }

    async fn fetch_task(
        &self,
        task_id: &str,
        info: &sea_weir_types::dto::RelayInfo,
    ) -> Result<TaskResult, AppError> {
        let (base_url, headers, _) = self.build_request(info)?;
        let url = format!("{base_url}/{}", task_id);
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| AppError::Upstream(format!("任务回源失败: {e}")))?;
        let raw: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::Upstream(format!("任务回源解析失败: {e}")))?;
        self.parse_task_result(raw)
    }

    fn parse_task_result(&self, raw: serde_json::Value) -> Result<TaskResult, AppError> {
        // 通用映射:各平台的 status 字段取值不同,骨架阶段按常见键提取。
        let status_text = raw
            .get("status")
            .or_else(|| raw.get("state"))
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN");
        let status = match status_text.to_ascii_uppercase().as_str() {
            "SUCCESS" | "SUCCEEDED" | "SUCCEED" => TaskStatus::Success,
            "FAILURE" | "FAILED" | "FAIL" => TaskStatus::Failure,
            "IN_PROGRESS" | "RUNNING" | "PROCESSING" => TaskStatus::InProgress,
            "QUEUED" | "PENDING" | "WAITING" => TaskStatus::Queued,
            "SUBMITTED" => TaskStatus::Submitted,
            "NOT_START" | "CREATED" => TaskStatus::NotStart,
            _ => TaskStatus::Unknown,
        };
        let task_id = raw
            .get("task_id")
            .or_else(|| raw.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        Ok(TaskResult {
            task_id,
            status,
            progress: raw
                .get("progress")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            fail_reason: raw
                .get("fail_reason")
                .or_else(|| raw.get("error"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
            usage: Self::usage_from(&raw),
            raw,
        })
    }

    fn platform_name(&self) -> &'static str {
        self.name
    }
}
