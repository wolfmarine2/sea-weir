//! 管理面各业务域的请求/响应 DTO。
//!
//! **字段命名铁律**:一律 `snake_case`,与 new-api 逐字段一致。
//! 建议在 crate 根统一 `#[serde(rename_all = "snake_case")]`,并由契约测试比对
//! new-api 实际响应的字段集(见 doc/architecture/adr-review-report.md 后续检查项 1)。
//!
//! 骨架阶段仅给出分域占位,具体字段在 TDD 时按 CONTRACTS.md §2~§9 逐域补全。

// TODO(TDD): 按域拆分文件 —— user / token / channel / log / pricing /
//            topup / subscription / model / option / task / data / group 等。
//
// 每个 DTO 落地时的验收标准:
//   1. 字段名集合 == new-api 对应端点响应的字段名集合(允许新增,不允许改名/缺失)
//   2. 可选字段的缺省行为与 Go 侧 omitempty 语义一致
//   3. 敏感字段(password / key / secret)不可序列化
