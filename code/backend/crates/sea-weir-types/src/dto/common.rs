//! 管理面统一响应包裹与分页约定。
//!
//! 见 doc/architecture/CONTRACTS.md §统一响应包裹、§通用约定。

use serde::{Deserialize, Serialize};

/// 管理面统一响应体。
///
/// 契约:**业务错误也返回 HTTP 200**,由 `success:false` 表达;
/// 仅鉴权失败(401/403)与限流(429)使用真实状态码。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            message: String::new(),
            data: Some(data),
        }
    }
    pub fn fail(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }
}

/// 分页查询参数。
///
/// **页码 1 起**(new-api `common/page_info.go`:`start_idx = (page-1)*page_size`,
/// 且 `p < 1` 一律归一为 1)。兼容别名:`ps`、`size`。
#[derive(Debug, Clone, Deserialize)]
pub struct PageQuery {
    #[serde(rename = "p", default)]
    pub page: i64,
    #[serde(default, alias = "ps", alias = "size")]
    pub page_size: i64,
}

impl PageQuery {
    pub fn start_idx(&self) -> i64 {
        (self.normalized_page() - 1) * self.page_size
    }
    pub fn normalized_page(&self) -> i64 {
        if self.page < 1 {
            1
        } else {
            self.page
        }
    }
}

/// 分页响应体。恒为 `{items, total, page, page_size}`,包在 `data` 内。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[cfg(test)]
mod tests {
    // TDD 入口(分页基数曾是审查发现的契约偏差,必须锁死):
    // - [ ] p=0 → normalized_page()==1 → start_idx()==0
    // - [ ] p=1 → start_idx()==0;p=2,page_size=10 → start_idx()==10
    // - [ ] p=-5 归一为 1
    // - [ ] `ps` 与 `size` 别名均能反序列化到 page_size
    // - [ ] ApiResponse 序列化字段名恰为 success/message/data
    // - [ ] data 为 None 时字段被省略(与 new-api 行为比对)
}
