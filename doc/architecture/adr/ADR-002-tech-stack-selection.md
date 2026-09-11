# ADR-002: 技术栈选型

## Metadata

| 项 | 内容 |
|----|------|
| 编号 | ADR-002 |
| 标题 | 技术栈选型(Rust 后端 + React 前端) |
| 状态 | Proposed |
| 日期 | 2026-09-10 |
| 决策者 | Architecture Team |
| 影响范围 | `code/backend` 全部 crate、`code/frontend` |
| 依赖 | 无 |

## Status History

| 日期 | 状态 | 说明 |
|------|------|------|
| 2026-09-10 | Proposed | 初始提议 |

## Context(上下文)

### 问题陈述

new-api(Go + Gin + GORM,前端 React 纯 JSX + Semi Design)需要整体重构。需选定后端语言/框架、数据库访问、HTTP 客户端、前端语言/组件库/状态管理。

### 约束条件

- 公司标准:Rust 微服务栈(service-auth 系)、openGauss、Valkey、Nacos、K8s
- 中继面负载特征:万级并发 SSE/WebSocket 长连接、IO 密集
- 计费链路要求强一致、可审计
- 契约必须与 new-api 兼容(灰度替换)

### 驱动力

- 性能:无 GC、零成本异步,长连接场景 P99 与内存优势
- 正确性:编译期 SQL 校验、强类型状态机(计费)
- 团队栈收敛:前端与公司统一 antd 5 + TS
- 生态成熟度:适配器需大量 HTTP/WS/流式处理,生态必须完整

### 目标状态

后端单二进制、单端口;前端 TS strict 零错误;构建免 protoc 等外部依赖。

### 备选方案

#### 方案 A:Rust(axum + tokio + sqlx + reqwest)+ React 18/TS/antd 5

**描述**: 后端全面 Rust 化;前端保留 React 但 TS 化并换 antd。
**优点**:
- tokio 长连接性能与内存效率;sqlx 编译期校验
- 强类型计费状态机;axum/tower 中间件组合与 Gin 语义对应,迁移心智成本低
- antd 5 与公司 web 栈统一
**缺点**:
- 35 个同步渠道适配器 + 10 家任务平台需逐一重写,工作量大
- Rust 编译速度慢,CI 需缓存

#### 方案 B:继续 Go,仅治理代码(重构而非重写)

**描述**: 保留 Go,拆分 module、补测试。
**优点**:
- 成本最低;适配器无需重写
**缺点**:
- 与公司 Rust 微服务栈分裂;Gin 动态类型 context 无法根治计费链路的隐式依赖
- 长连接内存与 GC 尾延迟问题保留

#### 方案 C:Node.js(NestJS)+ React

**描述**: 前后端同语言。
**优点**:
- 前后端类型共享
**缺点**:
- 单线程事件循环不适合 CPU 段(token 计数);公司无 Node 服务运维基线

### 决策依据

| 维度 | 权重 | A(Rust) | B(Go 治理) | C(Node) |
|---|---|---|---|---|
| 长连接性能/内存 | 高 | ✔ | △ | △ |
| 计费正确性(类型/状态机) | 高 | ✔ | △ | △ |
| 公司栈统一 | 高 | ✔ | ✘ | ✘ |
| 重写成本 | 中 | ✘ | ✔ | △ |
| 生态完整度(WS/SSE/tiktoken/webauthn) | 中 | ✔ | ✔ | ✔ |

## Decision(决策)

### 选择的方案

在 sea-weir 重构中,面对「高并发长连接网关 + 强一致计费 + 公司栈收敛」的关注点,我们选择 **Rust(axum + tokio + sqlx + reqwest)+ React 18 + TypeScript + antd 5 + zustand**,而非继续 Go 或转 Node,以获得类型安全与长连接性能、统一公司技术栈,接受 35 + 10 个适配器的重写成本。

### 决策理由

1. tokio + axum 在 SSE/WebSocket 长连接场景的吞吐与内存优势直接命中中继面负载
2. sqlx 编译期 SQL 校验 + rust_decimal 精确计费 + 显式状态机,根治 Go 版账务隐患
3. 前端 antd 5 + TS strict 与公司 web-auth 一致,组件/主题/权限模式可复用

### 技术架构

- 后端依赖:tokio 1、axum 0.8、sqlx 0.8(PG 特性)、reqwest 0.12(rustls + stream)、tokio-tungstenite 0.26、fred(Valkey)、jsonwebtoken 9、bcrypt、totp-rs、webauthn-rs、tiktoken-rs、rust-embed、moka、rust_decimal、tracing
- 前端依赖:react 18、react-router-dom 6、antd 5、zustand 4、axios 1、@ant-design/charts 2、i18next 23、vite 5、typescript 5.5 strict
- 明细见 system-design.md §4.2

### 实施计划

1. 搭建 workspace 五 crate 骨架 + CI(sqlx offline、cargo cache、前端 tsc 门禁)
2. 先实现中继面最小闭环(OpenAI chat 同步/流式 + 计费)+ 管理面登录/令牌/渠道
3. 适配器按渠道流量排序分批迁移,每批以契约测试(录制 new-api 请求/响应)验证
4. 前端按页面域迁移,表格三件套先行

## Consequences(后果)

### 正面影响

1. 编译期消除整类运行时错误(SQL、空指针、类型错)
2. 长连接资源占用显著下降,单节点容量提升
3. 前后端类型可从契约生成对齐(后续可引入 OpenAPI/TS 生成)

### 负面影响

1. 适配器重写量大,渠道行为回归风险高
2. Rust 招聘/上手成本高于 Go

### 缓解措施

1. 适配器契约测试:以 new-api 录制流量为黄金样本,逐渠道比对请求/响应
2. 灰度发布:按渠道/按用户分组切流,可回滚至 Go 版

### 长期影响

Rust 资产(tiktoken 计数、计费状态机、SSE 管道)可沉淀为公司中继基础设施;前端组件库与公司统一后主题/权限组件可跨项目复用。

## Notes

- reqwest 选 rustls 避免 OpenSSL 交叉编译问题
- Valkey 客户端选 fred(连接复用与 pub/sub 支持好)

## References

- `doc/system-design.md` §4
- 相关 ADR:ADR-003、ADR-005、ADR-009

---

**文档版本**: v1.0
**最后更新**: 2026-09-10
**下次评审**: 2026-12-10(3个月后)
**评审责任人**: Architecture Team Lead
