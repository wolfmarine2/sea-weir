# ADR-001: C4 架构分层设计

## Metadata

| 项 | 内容 |
|----|------|
| 编号 | ADR-001 |
| 标题 | C4 架构分层设计 |
| 状态 | Proposed |
| 日期 | 2026-09-10 |
| 决策者 | Architecture Team |
| 影响范围 | 全部文档(`doc/`)、全部 crate |
| 依赖 | 无 |

## Status History

| 日期 | 状态 | 说明 |
|------|------|------|
| 2026-09-10 | Proposed | 初始提议 |

## Context(上下文)

### 问题陈述

sea-weir 是含前端 SPA、HTTP 服务、中继引擎、数据访问层与多种外部依赖(35 个上游渠道适配器、支付、OAuth)的中等复杂度系统,需要一套从宏观到微观、能同时服务评审与编码的架构表达方法。

### 约束条件

- 文档须与代码结构可交叉验证(crate、容器、组件一一对应)
- 沿用公司 service-auth / service-hr 的文档模板与工具链(PlantUML;C4 图用 C4-PlantUML stdlib)

### 驱动力

- 渐进式抽象:评审看 L1/L2,编码看 L3 与时序图
- 一致性:时序图参与者必须可追溯到容器定义
- 可维护性:图源以文本入库,随代码演进 diff

### 目标状态

架构图、时序图、契约文档三者交叉引用一致;新增功能时组件归属明确。

### 备选方案

#### 方案 A:C4 模型(L1 上下文 / L2 容器 / L3 组件)

**描述**: 采用 C4 四层中的前三层,PlantUML 源文件入库,system-design.md 嵌 ASCII 简图保持一致。
**优点**:
- 渐进抽象,评审/编码各取所需
- C4-PlantUML stdlib 与现有 PlantUML 工具链同源,公司模板已有先例
- 容器命名为时序图参与者提供唯一合法来源
**缺点**:
- 学习成本(团队成员需理解 C4 语义)

#### 方案 B:传统 UML 分层架构图 + 自由时序图

**描述**: 单张分层图 + 自由绘制的时序图。
**优点**:
- 无学习成本
**缺点**:
- 抽象层级单一,中继引擎等复杂容器无法展开
- 时序图参与者无命名约束,文档间易漂移

### 决策依据

| 维度 | 权重 | A(C4) | B(UML) |
|---|---|---|---|
| 渐进抽象 | 高 | ✔ | ✘ |
| 公司模板一致 | 高 | ✔ | ✘ |
| 交叉可验证 | 中 | ✔ | ✘ |

## Decision(决策)

### 选择的方案

在 sea-weir 重构中,面对「架构表达需同时服务评审与编码」的关注点,我们选择 **C4 模型(L1/L2/L3)**,而非传统 UML 分层图,以获得渐进式抽象与跨文档一致性,接受团队学习 C4 语义的成本。

### 决策理由

1. 公司文档模板(service-auth 提炼)已采用 C4,工具链与审查清单现成
2. L2 容器定义为时序图参与者提供唯一命名来源,消除文档漂移
3. L3 能对 relay_engine(本系统最复杂的容器)单独展开

### 技术架构

- `c4/c4-l1-system-context.puml`:用户/AI 客户端/管理员 + sea-weir + openGauss/Valkey/Nacos/上游 LLM/OAuth/支付
- `c4/c4-l2-container.puml`:web_frontend、api_server、admin_engine、relay_engine、repository_layer + 外部容器
- `c4/c4-l3-component-{api-server,relay-engine,web-frontend}.puml`:三份组件图

### 实施计划

1. 完成 L1/L2 PlantUML 图并在 system-design.md §3 嵌 ASCII 简图
2. 三个核心容器各出一份 L3
3. 时序图审查报告校验参与者命名一致性

## Consequences(后果)

### 正面影响

1. 文档结构与公司模板一致,可复用审查清单
2. 组件职责边界在编码前冻结,减少返工
3. 新成员按 L1→L2→L3 渐进理解系统

### 负面影响

1. 图表维护成本:结构变更需同步 `.puml` 图源与 ASCII 简图

### 缓解措施

1. README 约定「system-design.md 是唯一主文档,图是其展开」,评审时强制一致性检查

### 长期影响

C4 容器边界即为未来服务拆分的天然边界(如 relay_engine 独立为中继服务)。

## Notes

无。

## References

- `doc/system-design.md` §3
- `doc/architecture/c4/*.puml`
- `service/doc/architecture/c4/`(模板)

---

**文档版本**: v1.0
**最后更新**: 2026-09-10
**下次评审**: 2026-12-10(3个月后)
**评审责任人**: Architecture Team Lead
