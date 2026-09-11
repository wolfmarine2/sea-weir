# ADR-008: 错误分类与统一错误出口

## Metadata

| 项 | 内容 |
|----|------|
| 编号 | ADR-008 |
| 标题 | 错误分类与统一错误出口(四级分类 + 双协议面格式化) |
| 状态 | Proposed |
| 日期 | 2026-09-10 |
| 决策者 | Architecture Team |
| 影响范围 | `sea-weir-types/AppError`、`sea-weir-server/response.rs`、relay_engine 错误处理 |
| 依赖 | ADR-002, ADR-004 |

## Status History

| 日期 | 状态 | 说明 |
|------|------|------|
| 2026-09-10 | Proposed | 初始提议 |

## Context(上下文)

### 问题陈述

系统有双协议面(管理面 `{success,message,data}`、中继面 OpenAI/Claude/Gemini/MJ 各自错误格式),且中继面错误还要驱动重试与渠道禁用决策。需要一个统一内部错误类型承载分类与元数据,并在出口按面格式化。

### 约束条件

- 中继面错误格式必须 OpenAI 兼容(Claude/Gemini/MJ 路径各有原生错误结构)
- 渠道错误需携带:HTTP 状态、error_code(`channel:*` 前缀)、是否可重试、是否记日志
- 敏感信息(密钥/邮箱)出口前必须打码

### 驱动力

- 正确性:重试/禁用决策依赖结构化错误而非字符串匹配
- 可观测:request-id 贯穿,错误日志含渠道链与请求快照
- 兼容:客户端按 OpenAI 错误语义工作

### 目标状态

内部单一错误枚举;出口格式化集中;重试/禁用由错误元数据驱动;panic 有兜底格式。

### 备选方案

#### 方案 A:AppError 枚举(四级分类)+ NewApiError 中继扩展 + 出口集中格式化

**描述**: 基础层 `AppError`(Transient/Permanent/Recoverable/Unrecoverable);中继层 `NewApiError{status_code, error_code, relay_format, skip_retry}`;出口由 response 模块按面转换。
**优点**:
- 决策元数据显式;出口格式单点维护
- panic-hook 统一兜底
**缺点**:
- 错误类型设计需一次性覆盖各面语义

#### 方案 B:thiserror 细粒度错误 + 各 handler 自行格式化

**描述**: 错误格式化分散在 handler。
**优点**:
- 局部灵活
**缺点**:
- 出口格式漂移风险;重试/禁用决策分散

### 决策依据

| 维度 | 权重 | A | B |
|---|---|---|---|
| 出口一致性 | 高 | ✔ | ✘ |
| 决策驱动(重试/禁用) | 高 | ✔ | ✘ |
| 局部灵活性 | 低 | △ | ✔ |

## Decision(决策)

### 选择的方案

在 sea-weir 错误处理设计中,面对「双协议面出口 + 错误驱动路由决策」的关注点,我们选择 **AppError 四级分类 + NewApiError 中继扩展 + 出口集中格式化**,而非 handler 分散格式化,以获得出口一致性与显式的重试/禁用决策,接受错误类型前期设计成本。

### 决策理由

1. `channel:*` 错误码与 skip_retry 标志使重试/禁用成为数据驱动而非字符串匹配
2. 出口集中保证管理面/中继面/Claude/Gemini/MJ 各格式不漂移
3. 四级分类直接映射恢复策略(重试/拒绝/降级/对账)

### 技术架构

```
AppError(基础层):Database(Transient) / Biz(Permanent) / Unauthorized / Forbidden /
                 RateLimited(Recoverable) / QuotaExceeded(Recoverable) / Internal
NewApiError(中继层):{ status_code, error_code, error_type, skip_retry, record_error_log }
出口:
  管理面 → HTTP 200 {success:false,message} / 401 / 403 / 429
  中继面 → 按 RelayFormat:OpenAI {"error":{…}} / Claude {"type":"error",…} / MJ {"code",…}
  panic → panic-hook → {"error":{"type":"new_api_panic",…(request id)}}
打码:出口前 MaskSensitiveInfo(key/邮箱/密钥模式)
```

### 实施计划

1. 定义 AppError/NewApiError 与 From 映射
2. response 模块实现双面出口 + 打码 + panic-hook
3. 重试/禁用判定迁移到错误元数据;补契约测试(各格式错误样本)

## Consequences(后果)

### 正面影响

1. 重试/禁用决策可单测(错误构造 → 断言行为)
2. 出口格式集中,新增协议面只加格式化分支
3. request-id 贯穿错误与日志,排障成本下降

### 负面影响

1. 错误类型初期设计不全时需补枚举(编译期可发现)

### 缓解措施

1. 保留 `Internal(String)` 逃生舱;评审时收敛

### 长期影响

错误元数据可演进为渠道 SLA 统计的数据源。

## Notes

- 渠道状态码映射(status_code_mapping)在出口前应用,200 不重映射
- 管理面业务错误保持 HTTP 200 + success=false(new-api 兼容)

## References

- `doc/system-design.md` §10
- `doc/architecture/CONTRACTS.md` §13
- `doc/architecture/sequence-diagram/SEQ-005`
- 相关 ADR:ADR-004、ADR-005、ADR-006

---

**文档版本**: v1.0
**最后更新**: 2026-09-10
**下次评审**: 2026-12-10(3个月后)
**评审责任人**: Architecture Team Lead
