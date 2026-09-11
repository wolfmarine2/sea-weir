# Architecture Decision Records (ADR)

## 概述

本目录包含 sea-weir(AI 网关与大模型 API 管理中台,new-api 的 React + Rust 重构)的架构决策记录。每个 ADR 记录一个关键架构决策的背景、选择、理由和影响。

**项目**: sea-weir
**范围**: AI 网关(中继面 OpenAI/Claude/Gemini 兼容转发)+ 管理中台(用户/令牌/渠道/计费/订阅/设置);Rust workspace 五 crate(`sea-weir-server / -core / -adaptors / -repository / -types`)+ React SPA
**总文档**: 9 个 ADR
**最后更新**: 2026-09-10

---

## ADR 索引

| ADR编号 | 标题 | 状态 | 决策摘要 | 文件 |
|---------|------|------|----------|------|
| ADR-001 | C4 架构分层设计 | Proposed | 采用 C4 模型(L1 上下文→L2 容器→L3 组件)描述系统,统一用 PlantUML + C4-PlantUML stdlib 绘制(`.puml`) | [ADR-001-c4-architecture-layering.md](ADR-001-c4-architecture-layering.md) |
| ADR-002 | 技术栈选型 | Proposed | 后端 Rust(axum + tokio + sqlx + reqwest),前端 React 18 + TypeScript + antd 5 + zustand | [ADR-002-tech-stack-selection.md](ADR-002-tech-stack-selection.md) |
| ADR-003 | 数据库选型与迁移 | Proposed | 定型 openGauss 单方言;表结构与 new-api 同名同语义;JSONB/部分索引;MySQL→openGauss 类型映射 | [ADR-003-database-selection-and-migration.md](ADR-003-database-selection-and-migration.md) |
| ADR-004 | 双 API 面与认证模型 | Proposed | 管理面(签名会话 Cookie + access token + New-Api-User)与中继面(sk-token)分离;角色四级;会话吊销走 Valkey | [ADR-004-dual-api-surface-and-auth.md](ADR-004-dual-api-surface-and-auth.md) |
| ADR-005 | 计费三段式与并发幂等 | Proposed | 预扣→结算→退款显式状态机;额度原子扣减;幂等键唯一约束;账务同步落库、统计批量合并 | [ADR-005-billing-three-phase-and-idempotency.md](ADR-005-billing-three-phase-and-idempotency.md) |
| ADR-006 | 渠道适配器与智能路由 | Proposed | Adaptor/TaskAdaptor trait 注册表;(group,model)→优先级档→加权随机;auto 分组;亲和性缓存 | [ADR-006-channel-adaptor-and-routing.md](ADR-006-channel-adaptor-and-routing.md) |
| ADR-007 | 缓存与限流策略 | Proposed | Valkey + 进程内两级;失效用 pub/sub 广播 + 60s TTL 兜底;限流滑动窗口(Valkey/内存降级) | [ADR-007-cache-and-rate-limit.md](ADR-007-cache-and-rate-limit.md) |
| ADR-008 | 错误分类与统一错误出口 | Proposed | AppError 四级分类;管理面 {success,message,data} 与中继面 OpenAI/Claude 错误格式双出口 | [ADR-008-error-classification-and-exit.md](ADR-008-error-classification-and-exit.md) |
| ADR-009 | 前端架构 | Proposed | React 18 + TS strict + antd 5(替换 Semi)+ zustand(收敛 localStorage)+ 服务端鉴权为准 | [ADR-009-frontend-architecture.md](ADR-009-frontend-architecture.md) |
| ADR-010 | 测试策略与质量门禁 | Proposed | 四层测试(单元/集成/契约/端到端);判定基准取自 new-api 实际行为而非设计文档;test 环境不允许因依赖不可达而跳过 | [ADR-010-testing-strategy.md](ADR-010-testing-strategy.md) |

---

## ADR 依赖关系图

```
Wave 1 (无依赖):
├── ADR-001: C4 架构分层设计
└── ADR-002: 技术栈选型

Wave 2 (依赖 Wave 1):
├── ADR-003: 数据库选型与迁移 (depends: 001, 002)
├── ADR-004: 双 API 面与认证模型 (depends: 001, 002)
└── ADR-009: 前端架构 (depends: 002)

Wave 3 (依赖 Wave 2):
├── ADR-005: 计费三段式与并发幂等 (depends: 003, 004)
├── ADR-006: 渠道适配器与智能路由 (depends: 002, 003)
├── ADR-007: 缓存与限流策略 (depends: 002, 003)
└── ADR-008: 错误分类与统一错误出口 (depends: 002, 004)

Wave 4 (依赖 Wave 3):
└── ADR-010: 测试策略与质量门禁 (depends: 002, 005, 008)
```

---

## 决策分类

### 架构设计类
- **ADR-001**: 系统架构可视化方法论
- **ADR-004**: API 面划分与认证体系
- **ADR-006**: 中继核心抽象(适配器/路由)

### 技术选型类
- **ADR-002**: 开发语言、框架、前端栈
- **ADR-003**: 数据库与迁移策略
- **ADR-009**: 前端组件库与状态管理

### 业务策略类
- **ADR-005**: 计费正确性与并发控制
- **ADR-007**: 缓存一致性与限流
- **ADR-008**: 错误处理与恢复策略

### 工程实践类
- **ADR-010**: 测试分层、契约基线与 CI 门禁

---

## 关键决策摘要

### C4 架构分层(ADR-001)

采用 C4 模型而非传统 UML 分层图。L2 明确 web_frontend / api_server / admin_engine / relay_engine / repository_layer 五个内部容器与 openGauss、Valkey、上游 LLM 的边界;时序图参与者一律取自 L2 容器名。

### 技术栈选型(ADR-002)

后端选 Rust(axum 0.8 + tokio + sqlx 0.8 + reqwest),不选继续 Go 或 Node;前端选 React 18 + TypeScript strict + antd 5 + zustand + vite,与公司 web 栈(web-auth)统一。收益:类型安全、长连接性能、团队栈收敛;代价:35 个同步渠道适配器 + 10 家任务平台适配器需逐一重写。

### 数据库选型与迁移(ADR-003)

定型 openGauss,放弃 SQLite/MySQL 方言分支;表结构与 new-api 同名同语义以支持存量数据迁移;额度统一 BIGINT、JSON 列统一 JSONB、软删除唯一约束用部分索引。提供 dbswitch/自研迁移工具完成 MySQL→openGauss 全量+校验。

### 双 API 面与认证(ADR-004)

管理面与中继面在同进程同端口按路径分流,中间件链独立。管理面用签名会话 Cookie(自承载,吊销走 Valkey jti 黑名单)+ `New-Api-User` 防串号;中继面用 sk-token(HMAC 后作缓存键)。保留原契约的所有认证入口(x-api-key / ?key= / WS 子协议 / mj-api-secret)。

### 计费三段式与幂等(ADR-005)

预扣(信任旁路 + 令牌先行 + 资金源,失败回滚)→ 结算(差额补退,rust_decimal 精确计算)→ 失败退款(异步幂等);settled/fundingSettled 标志保证 Settle 与 Refund 互斥;账务路径同步落库,仅统计口径批量合并,消除原批量更新的崩溃丢账窗口;订阅预扣以 request_id 幂等,支付/兑换以 trade_no / key 唯一约束 + FOR UPDATE。

### 渠道适配器与智能路由(ADR-006)

`Adaptor` / `TaskAdaptor` trait + 注册表(ApiType 35 个 / 任务平台 10 个);选路 = (group, model) → 第 retry 高优先级档 → 档内加权随机(weight+10);auto 分组为「分组列表 × 优先级」二维降级;亲和性缓存(HybridCache)提供粘性路由;格式转换集中 convert 模块并记录转换链。

### 缓存与限流(ADR-007)

用户/令牌/亲和性缓存走 Valkey(hash,60s TTL,读穿 + 异步回填);渠道索引与定价视图进程内缓存(moka)+ Valkey pub/sub 失效广播;Valkey 不可用时限流降级进程内滑动窗口、缓存回源 DB;会话吊销 fail-close。

### 错误分类与出口(ADR-008)

AppError 四级分类(Transient/Permanent/Recoverable/Unrecoverable);管理面 `{success:false,message}`,中继面按 RelayFormat 转 OpenAI/Claude/Gemini/MJ 错误格式;敏感信息打码;渠道错误前缀 `channel:` 驱动重试与自动禁用;panic 兜底为 `new_api_panic`。

### 前端架构(ADR-009)

antd 5 替换 Semi Design(公司统一),zustand 替换 Context+useReducer;服务端配置收敛进 status store(不再平铺 20+ localStorage 键);路由守卫为体验层,权限以服务端逐端点鉴权为准;表格三件套模式沿用并类型化。

---

## 相关设计文档

### C4 架构设计
- `../c4/c4-l1-system-context.puml`
- `../c4/c4-l2-container.puml`
- `../c4/c4-l3-component-*.puml`(api-server / relay-engine / web-frontend)

### 时序图设计
- `../sequence-diagram/SEQ-*.puml`(SEQ-001 ~ SEQ-006)

### 接口契约定义
- `../CONTRACTS.md`

---

## ADR 模板格式

本目录 ADR 采用 **Michael Nygard 模板 + Y-Statements 混合格式**(模板沿用 `service/doc/architecture/adr/ADR-000-template.md`):

1. **Metadata**: 编号、标题、状态、日期、决策者、影响范围
2. **Status History**: 状态变更历史
3. **Context**: 问题陈述、约束条件、驱动力、备选方案(≥2个)
4. **Decision**: 选择方案、决策理由(≥3个)、技术架构、实施计划
5. **Consequences**: 正面影响(≥3个)、负面影响、缓解措施、长期影响
6. **Notes** / **References**

---

## 状态定义

| 状态 | 说明 |
|------|------|
| Proposed | 已提议,待审查 |
| Accepted | 已接受,正在实施 |
| Deprecated | 已废弃,不再推荐 |
| Superseded | 已被新ADR取代 |

---

## 文档历史

| 版本 | 日期 | 作者 | 变更 |
|------|------|------|------|
| v1.0 | 2026-09-10 | Architecture Team | 初始 9 份 ADR |

---

**End of Document**

### ADR-010: 测试策略与质量门禁

四层测试(L1 单元 / L2 集成 / L3 契约 / L4 端到端)。核心决策:**判定基准取自 new-api 的
实际运行行为而非设计文档** —— v1.0 设计文档曾带 4 处契约偏差通过人工审查,按文档写测试
会把偏差固化。契约层从 new-api 录制响应做 fixture,录制与运行解耦。test 环境**不允许**
因依赖不可达而跳过套件(杜绝假绿)。黄金用例见 `../TEST-VECTORS.md`。
