# ADR 完整性与一致性审查报告

**审查日期**: 2026-09-10(第 1 轮)/ 2026-09-10(第 2 轮,含对 new-api 源码的逐项复核)
**审查员**: Architecture Team (ADR Consistency & Completeness Checker)
**审查材料**: 9 个 ADR 文件 + README.md 索引 + 系统设计文档 + CONTRACTS.md + new-api 源码交叉引用(`/home/zhangzc/project/new-api`)
**项目**: sea-weir(AI 网关与大模型 API 管理中台)

> **第 2 轮说明**:第 1 轮审查仅做文档内部交叉核对,未对照 new-api 源码逐条验证「兼容性声明」,结论为「0 问题、全部 PASS」。第 2 轮补做了源码级复核,发现 **6 项问题(其中 1 项 P0)**,均已修复。第 1 轮的 C4「可溯源」结论应视为**覆盖不足**而非成立 —— 检查项 C4 已按下表重新定义并加严。

---

## 检查结果(第 2 轮)

### A. ADR 文件完整性检查

| 项 | 结论 | 证据 |
|---|---|---|
| A1: ADR 文件数量与 README 索引一致,无孤儿 ADR 或缺失引用 | PASS | 索引 9 份(ADR-001~009),`adr/` 目录实有 9 份,文件名与索引链接一致 |
| A2: README.md 索引完整(索引表、依赖关系图、决策分类、关键决策摘要、模板格式说明、状态定义表) | PASS | `adr/README.md` 六节齐全 |
| A3: 索引与实际文件一一对应 | PASS | 逐份核对通过 |

### B. 模板合规性检查

| 项 | 结论 | 证据 |
|---|---|---|
| B1: 所有 ADR 均包含 Metadata(编号/标题/状态/日期/决策者/影响范围) | PASS | 9/9 含完整 Metadata 表 |
| B2: 所有 ADR 均包含 Status History,日期格式统一 | PASS | 均为 2026-09-10 Proposed 初始提议 |
| B3: 所有 ADR 的 Context 结构完整(问题陈述/约束条件/驱动力/备选方案 ≥2 个) | PASS | ADR-001~006、009 各 2~3 个备选方案;ADR-008 2 个 |
| B4: 所有 ADR 的 Decision 决策理由 ≥3 个 | PASS | ADR-002/003/005/006/007 为 3~4 条;ADR-001/004/008/009 为 3 条 |
| B5: 所有 ADR 的 Consequences 正面影响 ≥3 个,含负面影响与缓解措施 | PASS | 9/9 满足 |

### C. 与系统设计文档一致性检查

| 项 | 结论 | 证据 |
|---|---|---|
| C1: ADR 决策与 system-design.md 对应章节一致 | PASS(修复后) | ADR-002↔§4 依赖表;ADR-003↔§7 DDL/类型映射;ADR-004↔§6.6 角色与认证;ADR-005↔§6.6/§8.3;ADR-006↔§3.3/§8.5;ADR-007↔§3.3;ADR-008↔§10 错误分类表;ADR-009↔§3.3/§4.2 前端栈。**第 1 轮曾把 ADR-007 标注为「↔§10.1 降级矩阵」,实际 §10.1 是错误分类表,已更正** |
| C2: ADR 引用的 crate 名 / Trait 名 / 表名与设计一致 | PASS(修复后) | crate 名、Adaptor/TaskAdaptor、表名一致。**修复:适配器数量口径在 ADR-001/002/006/README 与 system-design、CONTRACTS §11 之间不一致(40 vs 35),现统一为「35 个同步渠道适配器(ApiType)+ 10 家任务平台」** |
| C3: ADR 之间无相互矛盾的决策 | PASS | ADR-005 账务同步落库与 ADR-007「预扣直查 DB」一致;ADR-004 吊销走 Valkey 与 ADR-007 降级 fail-close 一致;ADR-003 openGauss 与 ADR-002 sqlx 编译期校验一致 |
| C4(加严): 每条「与 new-api 兼容」的声明都必须能定位到源码文件与符号 | PASS(修复后) | 见下方「源码复核明细」 |

### D. 依赖关系检查

| 项 | 结论 | 证据 |
|---|---|---|
| D1: 依赖关系图与各 ADR Metadata 中的依赖声明一致 | PASS | ADR-003/004 依赖 001+002;ADR-005 依赖 003+004;ADR-006/007 依赖 002+003;ADR-008 依赖 002+004;ADR-009 依赖 002;与 README 依赖图一致 |
| D2: 无循环依赖 | PASS | Wave 1→2→3 单向 |

---

## 源码复核明细(C4 加严项)

### 复核通过(15 项)

| 声明 | 出处 | new-api 证据 |
|---|---|---|
| QuotaPerUnit = 500000 | ADR-005、§6.6 | `common/constants.go:22` |
| 角色 0 / 1 / 10 / 100 | ADR-004、§6.6 | `common/constants.go:148-151` |
| 日志类型 0~6 | §6.6 | `model/log.go:46-52` |
| 令牌 key 48 位随机 | §7.2 DDL | `common/utils.go:251-253`(`GenerateRandomCharsKey(48)`) |
| 重试默认排除 400 / 408 / 504 / 524 | ADR-006/008、§8.5 | `setting/operation_setting/status_code_ranges.go:20-33` |
| 自动禁用默认状态码 401 | ADR-006、§8.5 | 同上 `AutomaticDisableStatusCodeRanges` |
| 异步任务轮询 15s | ADR-006、§8.6 | `service/task_polling.go:93` |
| SSE ping 默认 10s | §8.4 | `relay/helper/stream_scanner.go:27` |
| 渠道缓存 / Option 同步 60s | ADR-007、§9.2 | `common/init.go:102`(`SYNC_FREQUENCY` 默认 60) |
| `sk-xxx-{channelId}` 仅 admin/root | §6.6、CONTRACTS §1 | `middleware/auth.go:429-437`(`model.IsAdmin`) |
| `New-Api-User` 防串号头 | ADR-004、CONTRACTS §1 | `middleware/auth.go:95-96` |
| ClickHouse 仅占位变量 | §1.3 Out of Scope | `common/database.go:13`(`UsingClickHouse = false`,无引用) |
| 日志分库 `LOG_SQL_DSN` | ADR-003、§7.2 | `model/main.go:41,51,214` |
| 计费偏好四值 | ADR-005、CONTRACTS §7 | `common/str.go:112`、`service/billing_session.go:341,406` |
| 系统设置 12 个配置组 | §1.3、ADR-009 | `web/src/pages/Setting/index.jsx`(12 个 TabPane) |

### 第 2 轮发现并修复的问题(6 项)

| # | 级别 | 问题 | 位置 | 修复 |
|---|---|---|---|---|
| 1 | **P0** | 规定管理面 DTO 用 camelCase 并声称「与 new-api 逐字段一致」,实际 new-api 全量 snake_case(`model/user.go`、`model/token.go` 的 struct tag),直接推翻 §1.2 目标 1 | CONTRACTS.md §通用约定、system-design.md §4.2 | 改为强制 `snake_case` + serde `rename_all`,并在 §6.1 增加「字段命名铁律」与契约测试要求 |
| 2 | P1 | 表数量口径三处不一致:§7.1「24 张表」、§7.2「其余 16 表」(实列 18 个)、er-diagram.puml 26 实体 | system-design.md §7 | 统一为 **26 张**(8 核心 + 18),与 `model/main.go` AutoMigrate 清单一一对应 |
| 3 | P1 | 分页页码基数写反(「`p` 0 起」),实际 `start_idx=(page-1)*page_size` 且 `p<1` 归一为 1 | CONTRACTS.md §通用约定 | 改为 **1 起**,补记兼容别名 `ps` / `size` 与响应形状 |
| 4 | P1 | 交叉引用断链:「完整 200+ 端点清单见 CONTRACTS.md §3」,而 §3 是令牌域(7 行),全文无完整清单 | system-design.md §6.2 | 在 CONTRACTS.md 新增**附录 A:完整端点清单(305 条)**,逐条含鉴权与限流中间件标注;§6.2 改指附录 A |
| 5 | P1 | 适配器数量口径矛盾:多处「约 40 家渠道」,而 CONTRACTS §11 写「ApiType 35 个常量」 | ADR-001/002/006、README、system-design、两张 C4 图、module-dependency | 全量统一为「35 个同步渠道适配器(`relay/channel/` 36 个目录,部分共用 OpenAI 兼容 ApiType)+ 10 家任务平台」 |
| 6 | P2 | ADR-005 对 Go 版现状描述过时/失衡:未提及已有 `service/billing_session.go`;把默认关闭的 `BATCH_UPDATE_ENABLED` 表述为既有故障 | ADR-005 Context、system-design §1.1/§2.4 | 改为「已有 BillingSession 但耦合 `*gin.Context`、底层扣减缺 `AND quota >= ?` 守卫(`model/user.go:928`)、批量更新开关默认 false 但一旦开启即引入丢账窗口」,并在 §2.4 新增「无条件扣减」条目 |

---

## 迭代收敛状态

| 轮次 | 日期 | 审查范围 | 发现问题 | 已修复 | 状态 |
|------|------|----------|----------|--------|------|
| 1 | 2026-09-10 | 仅文档内部交叉核对 | 0 | 0 | 结论不可靠(覆盖不足) |
| 2 | 2026-09-10 | 文档交叉核对 + new-api 源码逐条复核 | 6(P0×1、P1×4、P2×1) | 6 | 全部 PASS |

---

## 附录:ADR 质量评分(第 2 轮)

| ADR | 完整性 | 一致性 | 可追溯性 | 总分 |
|-----|--------|--------|----------|------|
| ADR-001 C4 架构分层 | 5 | 5 | 5 | 15 |
| ADR-002 技术栈选型 | 5 | 5 | 5 | 15 |
| ADR-003 数据库选型与迁移 | 5 | 5 | 5 | 15 |
| ADR-004 双 API 面与认证 | 5 | 5 | 5 | 15 |
| ADR-005 计费三段式与幂等 | 5 | 5 | 5 | 15 |
| ADR-006 渠道适配器与路由 | 5 | 5 | 5 | 15 |
| ADR-007 缓存与限流 | 5 | 5 | 4 | 14 |
| ADR-008 错误分类与出口 | 5 | 5 | 5 | 15 |
| ADR-009 前端架构 | 5 | 5 | 5 | 15 |

(评分:5=优,4=良。ADR-007 可追溯性 4 分:限流窗口算法与 Valkey 降级策略为新增设计,无 new-api 对应实现可溯源,须在实施阶段补压测数据。)

## 后续检查项(实施阶段)

1. **契约回归测试**:对 new-api 与 sea-weir 同一端点做响应字段集 diff,守住 snake_case 与分页形状(防 P0/P1 回归)
2. 附录 A 端点清单纳入 CI:路由注册数与清单数不符即失败
3. 各 ADR 补代码交叉引用行号(实现落地后)

---

## 文档历史

| 版本 | 日期 | 作者 | 变更 |
|------|------|------|------|
| v1.0 | 2026-09-10 | Architecture Team | 首轮审查,全部 PASS |
| v2.0 | 2026-09-10 | Architecture Team | 补做 new-api 源码级复核;发现 6 项问题(P0×1)并全部修复;加严 C4 检查项;推翻 v1.0「0 问题」结论 |
