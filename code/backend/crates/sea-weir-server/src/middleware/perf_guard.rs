//! 系统性能护栏。对应 new-api 的 `SystemPerformanceCheck`。
//!
//! 负载过高时快速拒绝新的中继请求,保护在途长连接。

pub fn layer() -> impl Clone {
    todo!("检查负载指标;超阈值返回 503")
}
