//! 统一响应出口。对应 C4 组件 `response`。
//!
//! 两个面的出口形状完全不同,**不可混用**:
//! - 管理面:`{success, message, data}`,业务错误也是 HTTP 200
//! - 中继面:上游原生格式;错误按 `RelayFormat` 分派

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use sea_weir_types::dto::common::ApiResponse;
use sea_weir_types::{AppError, NewApiError, RelayFormat};

/// 管理面成功出口。字段恒为 `success/message/data`,`message` 为空串。
pub fn ok<T: serde::Serialize>(data: T) -> Response {
    (StatusCode::OK, Json(ApiResponse::ok(data))).into_response()
}

/// 管理面错误出口。
///
/// 契约:业务错误也返回 HTTP 200 + `success:false`;仅未认证(401)、
/// 限流(429)与基础设施错误走真实状态码(见 `AppError::admin_status`)。
pub fn err(e: AppError) -> Response {
    let status = e.admin_status();
    let retry_after = match &e {
        AppError::RateLimited { retry_after_secs } => Some(*retry_after_secs),
        _ => None,
    };
    let body = ApiResponse::<()>::fail(e.to_string());
    let mut response = (status, Json(body)).into_response();
    if let Some(secs) = retry_after {
        if let Ok(value) = HeaderValue::from_str(&secs.to_string()) {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
    }
    response
}

/// 中继面错误出口。按入口协议生成错误体;429 附 `Retry-After`。
pub fn relay_err(e: NewApiError, format: RelayFormat) -> Response {
    let status = StatusCode::from_u16(e.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = e.to_body(format);
    let mut response = (status, Json(body)).into_response();
    if status == StatusCode::TOO_MANY_REQUESTS {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
    }
    response
}

/// panic 兜底(注册为 tower 的 catch-panic 层)。
pub fn panic_response(request_id: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(sea_weir_types::relay_error::panic_body(request_id)),
    )
        .into_response()
}

impl IntoResponse for crate::response::AppErrorWrapper {
    fn into_response(self) -> Response {
        err(self.0)
    }
}

/// 为 `?` 运算符提供 axum 集成。
pub struct AppErrorWrapper(pub AppError);

impl From<AppError> for AppErrorWrapper {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

#[cfg(test)]
mod tests {
    // TDD 入口(出口形状是契约,必须与 new-api 逐字节对齐):
    // - [ ] ok(): 字段恰为 success/message/data,message 为空串而非 null
    // - [ ] err(Biz): HTTP 200 + success:false
    // - [ ] err(Unauthorized): HTTP 401;err(Forbidden): 403;err(RateLimited): 429 + Retry-After
    // - [ ] relay_err 在 OpenAI/Claude/Gemini/MJ 四种格式下的错误体形状
    // - [ ] panic_response 含 request id
}
