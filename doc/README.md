# 文档目录说明

本目录为 **sea-weir(AI 网关与大模型 API 管理中台)** 的系统设计文档。sea-weir 是对开源项目 [new-api](https://github.com/QuantumNous/new-api)(Go + Gin + GORM)的 **React + Rust 整体重构**。文档结构沿用微服务文档模板(`service/doc`,提炼自 service-auth / service-hr),并针对本项目特点做了如下调整:

| 模板约定(微服务) | 本项目(sea-weir) | 原因 |
|---|---|---|
| 对外提供 gRPC 服务端(tonic server) | 对外仅提供 **HTTP**(axum):管理面 REST(`/api/*`)+ 中继面 OpenAI/Claude/Gemini 兼容 API(`/v1` 等) | 原 new-api 即纯 HTTP 网关,无 gRPC 面 |
| 单一 Rust 服务 | **前端(React SPA)+ 后端(Rust 服务)两个可部署单元** | Web 项目前后端分离 |
| ER 图 + SQL DDL(openGauss) | 保留;表结构与 new-api 对齐以便数据迁移,DDL 为 openGauss 语法 | 重构项目需数据兼容(ADR-003) |
| Repository Trait 契约 | REST 路由契约 + 中继面协议契约 + Adaptor Trait 契约 + Repository Trait 契约 | 网关项目契约对象更多 |
| C4 L3 按业务引擎组件拆分 | C4 L3 分 **api_server(入口+中间件)**、**relay_engine(中继管线)**、**web_frontend(页面/组件层)** 三份 | 网关的核心复杂度在中继管线 |
| input-standard/ 需求输入表单 | **不收本目录** —— 属需求输入物,非设计产出 | 与 web-auth 约定一致 |

## 目录结构

```
doc/
├── README.md                          # 本文件
├── system-design.md                   # 主系统设计文档
├── test-design.md                     # 测试设计(TDD 分层、覆盖矩阵、环境、门禁)
└── architecture/
    ├── CONTRACTS.md                   # 接口契约(REST 路由 / 中继协议 / Adaptor Trait / Repository Trait)
    ├── TEST-VECTORS.md                # 黄金用例与判定表(TDD 红灯依据)
    ├── adr/
    │   ├── README.md                  # ADR 索引(含依赖关系图、决策分类、关键摘要)
    │   ├── ADR-001-c4-architecture-layering.md
    │   ├── ADR-002-tech-stack-selection.md
    │   ├── ADR-003-database-selection-and-migration.md
    │   ├── ADR-004-dual-api-surface-and-auth.md
    │   ├── ADR-005-billing-three-phase-and-idempotency.md
    │   ├── ADR-006-channel-adaptor-and-routing.md
    │   ├── ADR-007-cache-and-rate-limit.md
    │   ├── ADR-008-error-classification-and-exit.md
    │   ├── ADR-009-frontend-architecture.md
    │   └── ADR-010-testing-strategy.md
    ├── adr-review-report.md           # ADR 完整性与一致性审查报告
    ├── sequence-diagram-review.md     # 时序图完整性与一致性审查报告
    ├── c4/
    │   ├── c4-l1-system-context.puml       # C4 L1 系统上下文图
    │   ├── c4-l2-container.puml            # C4 L2 容器图
    │   ├── c4-l3-component-api-server.puml     # C4 L3 api_server 组件图
    │   ├── c4-l3-component-relay-engine.puml   # C4 L3 relay_engine 组件图
    │   └── c4-l3-component-web-frontend.puml   # C4 L3 web_frontend 组件图
    ├── sequence-diagram/
    │   ├── SEQ-001-user-login-and-admin-auth.puml   # 用户登录与管理面认证
    │   ├── SEQ-002-token-auth-and-distribution.puml # 中继面令牌认证与渠道分发
    │   ├── SEQ-003-text-relay-billing.puml          # 文本中继计费全流程(预扣-调用-结算)
    │   ├── SEQ-004-sse-stream-relay.puml            # 流式 SSE 转发与 usage 注入
    │   ├── SEQ-005-channel-retry-autoban.puml       # 渠道重试与自动禁用
    │   └── SEQ-006-async-task-polling.puml          # 异步任务提交与轮询结算
    ├── er-diagram.puml                # ER 图(openGauss)
    ├── module-dependency.puml         # 模块依赖图(后端 workspace crate + 前端 src 分层)
    └── deployment-architecture.puml   # 部署架构图(C4 Deployment,K8s 拓扑)
```

## 文档产出约定

1. **system-design.md 是唯一主文档**,architecture/ 下的图与 ADR 是其展开;两者内容必须一致
2. **ADR**:每个关键架构决策一份,编号 ADR-001 起;索引、依赖关系图、状态维护在 `adr/README.md`
3. **C4 图**:L1/L2 各一份;L3 按核心容器拆分为 api_server / relay_engine / web_frontend 三份
4. **时序图**:核心业务流程每个一份,命名 `SEQ-NNN-<流程名>.puml`,参与者名称必须来自 C4 L2 容器定义
4.1 **图源格式**:全部图(C4 / 时序 / ER / 模块依赖 / 部署)统一 PlantUML,扩展名一律 `.puml`;C4 类图使用 C4-PlantUML stdlib(`!include <C4/...>`),不使用 Structurizr DSL
5. **审查报告**:ADR 与时序图完成后各产出一份审查报告,记录完整性/一致性检查结果
6. **代码对应**:文档描述的实现位于 `code/backend`(Rust workspace)与 `code/frontend`(React + TS);数据库 DDL/初始化/种子脚本位于 `data/`(权威 DDL:`data/ddl.sql`);原实现 new-api(Go + React/JS,本地路径 `/home/zhangzc/project/new-api`)仅作迁移参照
6.1 **兼容性声明可溯源**:任何「与 new-api 一致」的表述都必须能定位到具体源码文件与符号,审查报告按此加严校验(见 `adr-review-report.md` C4 项)
7. 所有文档结尾维护「文档历史」表(版本/日期/作者/变更)
8. **测试**:`test-design.md` 定义分层与门禁,`architecture/TEST-VECTORS.md` 提供可直接
   转写为断言的黄金用例;测试代码与用例文档在 `test/`(`cases/` 与 `src/` 一一对应)
9. **溯源纪律**:凡「与 new-api 一致」的断言,期望值必须溯源到 new-api 源码行或录制响应,
   不接受设计文档作为依据(v1.0 文档曾带 4 处契约偏差通过人工审查)

## 文档历史

| 版本 | 日期 | 作者 | 变更 |
|------|------|------|------|
| v1.0 | 2026-09-10 | Architecture Team | 基于 new-api 调研初始化,目标架构 React + Rust |
| v1.1 | 2026-09-10 | Architecture Team | 源码级复核修订:管理面字段命名改 snake_case(P0);统一表数量 26、适配器 35+10、分页 1 起;新增 CONTRACTS 附录 A 完整端点清单(305 条);图源统一 `.puml` 并重写部署架构图;补齐 24 个前端页面域与 7 种 i18n 语言;两份审查报告升 v2.0 |
| v1.2 | 2026-09-10 | Architecture Team | 补齐 TDD 所需产出:新增 `test-design.md`、`architecture/TEST-VECTORS.md`、ADR-010;修正 CONTRACTS §6 计费公式(audio 计价位置、OtherRatios 连乘、下限规则、Claude 语义分支、取整方式)|
