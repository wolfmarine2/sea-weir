//! 性能基线。用例文档:`cases/13-performance.md`
//!
//! 本层**不做门禁**(除非显式开启),目的是发现数量级退化,不是卡毫秒。
//! 运行:`cargo test --release perf -- --ignored --nocapture`

use std::time::Instant;

/// 计价是每个请求都走的热路径,不能有意外开销。
#[test]
#[ignore = "性能基线,按需运行"]
fn perf_calculate_quota_throughput() {
    // TODO(实现): 10 万次 calculate_quota,断言 p99 < 10µs
    //   Decimal 运算比 f64 慢,但不应慢到成为瓶颈。
    //   若超标,考虑对整数倍率走快路径。
    let start = Instant::now();
    // ...
    println!("calculate_quota x100k: {:?}", start.elapsed());
}

/// 选路在每次重试都会执行,渠道多时不能退化成 O(n²)。
#[test]
#[ignore = "性能基线,按需运行"]
fn perf_channel_select_with_many_channels() {
    // TODO(实现): 1000 个渠道下选路 1 万次,断言平均 < 50µs
}

/// 渠道索引重建在渠道变更时触发,不能阻塞请求太久。
#[test]
#[ignore = "性能基线,按需运行"]
fn perf_channel_index_rebuild() {
    // TODO(实现): 1000 渠道 × 50 模型 × 10 分组 的索引重建耗时 < 500ms
}

/// SSE 转发的单 chunk 开销决定了流式吞吐上限。
#[test]
#[ignore = "性能基线,按需运行"]
fn perf_sse_chunk_rewrite() {
    // TODO(实现): 10 万次 rewrite_chunk,断言 p99 < 5µs(同格式透传路径)
}
