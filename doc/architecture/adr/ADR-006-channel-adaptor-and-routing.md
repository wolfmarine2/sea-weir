# ADR-006: 渠道适配器与智能路由

## Metadata

| 项 | 内容 |
|----|------|
| 编号 | ADR-006 |
| 标题 | 渠道适配器与智能路由(Adaptor trait + 分组/优先级/权重/亲和性) |
| 状态 | Proposed |
| 日期 | 2026-09-10 |
| 决策者 | Architecture Team |
| 影响范围 | `sea-weir-adaptors`、`sea-weir-core/relay`(select / channel_cache / autoban / convert) |
| 依赖 | ADR-002, ADR-003 |

## Status History

| 日期 | 状态 | 说明 |
|------|------|------|
| 2026-09-10 | Proposed | 初始提议 |

## Context(上下文)

### 问题陈述

中继面需要把统一的入口协议(OpenAI/Claude/Gemini)转发给 35 个协议各异的上游渠道适配器(`ApiType`),并在多渠道间做负载均衡、失败重试、自动禁用与亲和性保持。需要在 Rust 中重建这套「适配器 + 智能路由」核心。

### 约束条件

- 行为兼容 new-api:选路算法(优先级档 + 加权随机)、auto 分组降级、multi-key、亲和性、自动禁用规则
- 格式转换矩阵:入口格式 × 渠道格式(OpenAI⇄Claude⇄Gemini)
- 适配器数量大,新增渠道成本必须低

### 驱动力

- 可扩展:新增渠道 = 实现 trait + 注册,不改动引擎
- 性能:选路走进程内索引,不进 DB
- 韧性:失败重试、禁用、恢复形成闭环

### 目标状态

Adaptor/TaskAdaptor trait 稳定;选路 P99 < 1ms(内存命中);渠道行为有契约测试样本。

### 备选方案

#### 方案 A:trait 注册表 + 进程内渠道索引(moka)+ 引擎集中重试/禁用

**描述**: `sea-weir-adaptors` 提供 trait 与注册表;`relay_engine` 持有渠道索引缓存与重试状态机;格式转换集中 convert 模块。
**优点**:
- 新增渠道边际成本低;引擎策略集中可测
- 选路内存命中;失效广播多节点一致
**缺点**:
- trait 对象安全设计需谨慎(泛型 vs dyn)

#### 方案 B:宏生成/代码生成渠道代码

**描述**: 用声明式描述生成适配器。
**优点**:
- 样板代码最少
**缺点**:
- 渠道差异(认证/分帧/错误)难以声明式覆盖;调试困难

#### 方案 C:每个渠道独立 handler(无 trait 抽象)

**描述**: 复制 new-api 的松散结构。
**优点**:
- 与原版逐行对应
**缺点**:
- 引擎与渠道耦合,重试/禁用逻辑重复 40 份

### 决策依据

| 维度 | 权重 | A | B | C |
|---|---|---|---|---|
| 新增渠道成本 | 高 | ✔ | △ | ✘ |
| 引擎策略集中 | 高 | ✔ | ✔ | ✘ |
| 行为兼容验证 | 高 | ✔ | △ | ✔ |
| 实现风险 | 中 | △ | ✘ | ✔ |

## Decision(决策)

### 选择的方案

在 sea-weir 中继核心设计中,面对「35 个异构上游适配器 + 负载均衡 + 韧性闭环」的关注点,我们选择 **Adaptor/TaskAdaptor trait 注册表 + 进程内渠道索引 + 引擎集中重试/禁用**,而非代码生成或无抽象复制,以获得低扩展成本与集中可测的路由策略,接受 trait 对象安全设计的谨慎成本。

### 决策理由

1. trait 抽象使重试/禁用/计费/流式逻辑只写一遍,渠道只关心协议转换
2. 选路索引进程化(moka)+ 失效广播,满足高并发选路时延
3. 与原算法逐项对齐(优先级档、weight+10 平滑、auto 分组二维降级),行为可比对

### 技术架构

- 选路:`(group, model)` → abilities/渠道索引 → 第 retry 高优先级档 → 档内加权随机;`auto` 分组 = 分组列表 × 优先级二维降级;令牌 `cross_group_retry` 控制跨组
- 亲和性:规则(模型/路径/UA/键源)匹配 → HybridCache(内存 LRU + Valkey)命中直连;成功回写;失败按规则跳过重试
- multi-key:渠道 key 列表 random/polling 两种模式;按 key 粒度禁用
- 自动禁用:渠道错误(`channel:*`)/禁用状态码(默认 401)/关键词命中 → 单 key 整渠道禁用或多 key 按 key 禁用 → 通知 root;测试通过 + 开关 → 自动恢复
- 格式转换:convert 模块集中(OpenAI⇄Claude⇄Gemini 请求/响应/流),`RequestConversionChain` 记录链路并决定计费语义

### 实施计划

1. 定义 trait 与注册表;实现 openai/claude/gemini 三个核心适配器
2. 渠道索引缓存 + 失效广播;选路与重试状态机
3. 按流量排序迁移其余适配器;每个适配器配录制样本契约测试
4. 自动禁用/恢复与通知联调

## Consequences(后果)

### 正面影响

1. 新增渠道成本降为「实现 trait + 注册 + 样本测试」
2. 路由策略集中,单测覆盖重试/降级/禁用全矩阵
3. 选路热路径无 DB 访问

### 负面影响

1. 动态分发(`Arc<dyn Adaptor>`)有少量虚调用开销
2. trait 演进(新增能力)需全量适配器跟进

### 缓解措施

1. 热路径开销可忽略(IO 主导);提供默认实现降低 trait 演进成本

### 长期影响

适配器 crate 可独立发布;未来支持 WASM 插件式渠道(见 system-design.md §11 演进方向)。

## Notes

- `sk-xxx-{channelId}` 指定渠道绕过选路(仅 admin/root)
- 亲和性命中失败时 SkipRetryOnFailure 规则保证会话一致性

## References

- `doc/system-design.md` §3.3、§6.6
- `doc/architecture/CONTRACTS.md` §10、§11
- `doc/architecture/sequence-diagram/SEQ-002、SEQ-005`
- 相关 ADR:ADR-007(渠道索引缓存)、ADR-008(错误驱动禁用)

---

**文档版本**: v1.0
**最后更新**: 2026-09-10
**下次评审**: 2026-12-10(3个月后)
**评审责任人**: Architecture Team Lead
