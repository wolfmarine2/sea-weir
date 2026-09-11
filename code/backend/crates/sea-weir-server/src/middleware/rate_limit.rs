//! 限流。见 ADR-007 限流维度表。
//!
//! 维度(与 new-api 对齐):
//! - IP 维度:GlobalWeb / GlobalAPI / Critical
//! - IP 或用户维度:Search / Download / Upload
//! - 用户维度:ModelRequestRateLimit(总次数令牌桶 + 成功次数滑动窗口,分组可覆盖)
//!
//! 实现:Valkey 滑动窗口;Valkey 不可用时降级到进程内内存窗口。

#[derive(Debug, Clone, Copy)]
pub enum LimitDimension {
    GlobalWeb,
    GlobalApi,
    Critical,
    Search,
    Download,
    Upload,
    ModelRequest,
}

pub fn layer(_dimension: LimitDimension) -> impl Clone {
    todo!("axum layer;超限返回 429 + Retry-After")
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 窗口内第 N+1 次请求返回 429 且带 Retry-After
    // - [ ] 窗口滑出后恢复放行
    // - [ ] Valkey 不可用 → 降级内存窗口,不中断服务
    // - [ ] ModelRequestRateLimit 按用户隔离,不同用户互不影响
    // - [ ] 分组配置能覆盖默认限额
}
