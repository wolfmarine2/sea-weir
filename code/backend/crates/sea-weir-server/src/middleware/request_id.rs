//! request-id 生成与贯穿。
//!
//! 每个请求生成一个 id,注入 tracing span、写入中继日志与计费日志,
//! 并在 panic 兜底与错误响应中回显(便于线上排障)。

pub fn layer() -> impl Clone {
    todo!("生成/透传 request-id;注入 tracing span 与响应头")
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 未带 request-id 时自动生成
    // - [ ] 客户端带了则透传同一个值
    // - [ ] 错误响应体中包含该 id
}
