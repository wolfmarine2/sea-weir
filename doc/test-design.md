# sea-weir 测试设计(TDD)

**文档版本**:v1.0
**最后更新**:2026-09-10
**范围**:`test/` 全目录 + `code/backend` 各 crate 内联单测 + `code/frontend` vitest
**相关文档**:`architecture/adr/ADR-010-testing-strategy.md`、`architecture/TEST-VECTORS.md`、`architecture/CONTRACTS.md`

---

## 1. 测试的特殊定位

sea-weir 是**重构项目**,首要目标是契约兼容(§1.2 目标 1)。这决定了测试的判定基准
不在我们的设计文档里,而在 new-api 的**实际运行行为**里。

这不是理论顾虑。本项目的文档审查已经证明:v1.0 的设计文档带着 4 处 P0/P1 契约偏差
(camelCase 字段名、0 起分页、错误的计费公式、断链的端点清单)通过了首轮人工审查。
**如果测试按文档写,就会把这些偏差固化成"正确行为"。**

因此本项目的测试遵循一条原则:

> **凡涉及"与 new-api 一致"的断言,期望值必须可溯源到 new-api 的源码行或录制的响应,
> 不接受"设计文档这么写"作为依据。**

`architecture/TEST-VECTORS.md` 中每条向量都标注了来源(`源码` / `单测` / `录制`)。

---

## 2. 分层与目录

### 2.1 四层

| 层 | 代码位置 | 外部依赖 | 判定基准 | 典型耗时 |
|---|---|---|---|---|
| **L1 单元** | `code/backend/**` 内联 `#[cfg(test)]`;`test/src/unit_*.rs` | 无 | TEST-VECTORS 黄金用例 | < 1s |
| **L2 集成** | `test/src/integration_*.rs` | openGauss + Valkey | 不变量断言 | 10s ~ 1min |
| **L3 契约** | `test/src/contract_*.rs` | 录制 fixture | new-api 实际响应 | < 5s |
| **L4 端到端** | `test/src/live_*.rs`;前端 vitest + msw | 已部署实例 | 冒烟断言 | 1 ~ 5min |

**L1 与 L2 的分工**:纯逻辑(计价公式、选路加权、重试判定、格式转换)在 L1;
凡涉及事务、行锁、唯一约束、并发的一律在 L2 —— 这些语义 mock 验证不了。

### 2.2 目录结构

```
test/
├── README.md
├── Cargo.toml              # 独立 crate,依赖 code/backend/crates/*
├── cicd.sh                 # CD test 阶段入口,按 ENV 分发
├── env/
│   ├── common.sh           # 公共阶段执行器 + 依赖参数推导
│   ├── dev.sh / test.sh / prod.sh
│   └── record-baseline.sh  # 从 new-api 实例录制契约基线
├── cases/                  # 测试用例文档(本设计的展开)
│   ├── 01-unit-types.md          ~  12-security.md
│   └── fixtures/           # 黄金数据 + 录制基线
└── src/                    # 测试代码
    ├── lib.rs / common.rs
    ├── unit_*.rs
    ├── integration_*.rs
    ├── contract_*.rs
    ├── live_*.rs
    └── perf.rs
```

用例文档(`cases/`)与测试代码(`src/`)**一一对应**:每个 `TC-*` 编号在代码里有一个
同名测试函数,便于从失败的测试反查用例背景。

---

## 3. TDD 执行顺序

按「正确性风险 × 依赖深度」排。每一步都是**先写测试(红)→ 补实现(绿)→ 重构**。

| # | 目标模块 | 层 | 用例文档 | 为什么排这个位置 |
|---|---|---|---|---|
| 1 | `types::constants` / `error` | L1 | `01-unit-types.md` | 零依赖,锁死契约取值;后续所有测试都依赖这些常量 |
| 2 | `relay::billing::calculate_quota` | L1 | `02-unit-billing.md` | **正确性风险最高**;有 4 组 new-api 单测可直接移植 |
| 3 | `relay::select` / `autoban` | L1 | `03-unit-select-retry.md` | 纯函数 + 表驱动,覆盖成本最低、收益高 |
| 4 | `dto::common`(分页)/ `mask_token_key` | L1 | `01-unit-types.md` | 审查发现过的偏差,必须有测试守住 |
| 5 | Repository 额度/幂等 | L2 | `05-integration-repository.md` | 需真库;账务不变量的最后防线 |
| 6 | `adaptors::sync::common` | L1 | `04-unit-adaptors.md` | 一次覆盖 35 个适配器的共性 |
| 7 | `relay::convert` / `stream` | L1 | `06-unit-convert-stream.md` | 回放录制的上游响应 |
| 8 | 认证中间件 | L2 | `07-integration-auth.md` | 安全边界;fail-close 语义需真实缓存 |
| 9 | `relay::pipeline` | L2 | `08-integration-pipeline.md` | mock repo + wiremock 上游,验账目守恒 |
| 10 | 管理面 / 中继面契约 | L3 | `09-contract-admin.md`、`10-contract-relay.md` | 逐字段比对 new-api 录制响应 |
| 11 | 路由注册完整性 | L1 | `11-route-completeness.md` | 与 CONTRACTS 附录 A 对表 |
| 12 | 前端 | L1/L4 | `12-frontend.md` | 三条契约 + 表格地基 |

---

## 4. 覆盖矩阵

行 = 设计文档中的关注点,列 = 测试层。`●` 主要覆盖,`○` 辅助覆盖。

| 关注点 | 来源 | L1 | L2 | L3 | L4 |
|---|---|---|---|---|---|
| 计价公式正确性 | ADR-005 | ● | ○ | ○ | |
| 三段式状态机与幂等 | ADR-005 | ● | ● | | |
| 额度扣减原子性与并发 | ADR-005 | | ● | | |
| 崩溃预扣对账 | ADR-005 | | ● | | |
| 渠道选路与降级 | ADR-006 | ● | ○ | | |
| 重试与自动禁用 | ADR-006/008 | ● | ○ | | |
| 格式转换 | ADR-006 | ● | | ○ | |
| 流式转发与 usage 注入 | SEQ-004 | ● | ○ | ○ | |
| 异步任务轮询与 CAS | SEQ-006 | ○ | ● | | |
| 认证与角色闸门 | ADR-004 | ● | ● | ○ | |
| 敏感操作凭证一次性 | ADR-004 | | ● | | |
| 缓存降级 fail-close | ADR-007 | | ● | | |
| 限流 | ADR-007 | ○ | ● | | |
| 错误分类与出口形状 | ADR-008 | ● | | ● | |
| 管理面字段命名(snake_case) | CONTRACTS | ○ | | ● | |
| 分页基数与形状 | CONTRACTS | ● | | ● | |
| 路由完整性(305 条) | 附录 A | ● | | | ○ |
| 前端三条契约 | ADR-009 | ● | | ● | ○ |
| SPA 路由与守卫 | ADR-009 | ● | | | ○ |

**空白即风险**:表中某关注点若整行无 `●`,说明该设计决策没有测试保障,应补用例。

---

## 5. 测试环境

### 5.1 依赖矩阵

| 层 | openGauss | Valkey | new-api 实例 | 已部署 sea-weir |
|---|---|---|---|---|
| L1 | | | | |
| L2 | ✔ testcontainers | ✔ testcontainers | | |
| L3 | | | 录制时需要,运行时不需要 | |
| L4 | | | | ✔ |

L3 的设计要点:**录制与运行解耦**。录制一次落成 fixture 入库,之后 CI 不再需要 new-api。

### 5.2 分环境策略(沿用 service-auth 约定)

| 环境 | 策略 |
|---|---|
| **dev** | 审查 + L1 必跑;L2/L3 按依赖可达性启用,不可达仅**告警跳过**;L4 跳过 |
| **test** | **全量门禁**:`DATABASE_URL` / `REDIS_URL` / `SEAWEIR_HTTP_BASE_URL` 必填,缺一即失败 |
| **prod** | 部署后冒烟:审查 + L1 + HTTP 连通性;性能基准默认禁用(需 `PROD_ALLOW_LOAD=1`) |

**为什么 test 环境不允许跳过**:允许跳过就会出现"依赖没配好 → 套件跳过 → CI 全绿"的
假绿。这在 service-auth 上有过教训,直接沿用其结论。

### 5.3 基线录制

```bash
NEWAPI_BASE_URL=http://localhost:3000 \
NEWAPI_ADMIN_TOKEN=xxx \
bash test/env/record-baseline.sh
```

产出 `test/cases/fixtures/baseline/*.json`,每个文件带元信息:

```json
{
  "recorded_from": "new-api",
  "recorded_at": "2026-09-10T10:00:00Z",
  "newapi_version": "<VERSION 文件内容>",
  "endpoint": "GET /api/user/self",
  "request": { "...": "..." },
  "response": { "status": 200, "headers": {}, "body": {} }
}
```

契约测试失败时,先看 `recorded_at`:**是我方回归,还是上游演进?**

---

## 6. 门禁

| 门禁 | 阈值 | dev | test | prod |
|---|---|---|---|---|
| `cargo fmt --check` | 零差异 | ✔ | ✔ | ✔ |
| `cargo clippy -- -D warnings` | 零告警 | ✔ | ✔ | ✔ |
| `tsc -b` | 零错误 | ✔ | ✔ | ✔ |
| L1 全绿 | 100% | ✔ | ✔ | ✔ |
| L2 全绿 | 100% | 可跳过 | ✔ | — |
| L3 全绿 | 100% | 可跳过 | ✔ | — |
| L4 冒烟 | 100% | — | ✔ | ✔ |
| 覆盖率:`core::relay::billing`、`repository` 额度方法 | ≥ 90% | 告警 | ✔ | — |
| 覆盖率:其余 | ≥ 70% | 告警 | ✔ | — |
| 路由数 == 305 | 相等 | ✔ | ✔ | ✔ |

---

## 7. 命名与编号

### 7.1 用例编号

```
TC-<层>-<域>-<序号>-<类型>
```

| 段 | 取值 |
|---|---|
| 层 | `UNI`(单元)/ `INT`(集成)/ `CTR`(契约)/ `E2E` |
| 域 | `TYP` `BIL` `SEL` `RTY` `ADP` `CVT` `STM` `REP` `AUT` `PIP` `API` `RLY` `RTE` `FE` |
| 序号 | 三位,域内递增 |
| 类型 | `POS`(正向)/ `NEG`(负向)/ `BND`(边界)/ `CNC`(并发)/ `IDM`(幂等) |

例:`TC-UNI-BIL-001-POS`、`TC-INT-REP-003-CNC`。

### 7.2 测试函数命名

用例编号转小写下划线,加语义后缀:

```rust
#[test]
fn tc_uni_bil_001_pos_openai_semantic_cache_read() { ... }
```

这样测试失败时能直接定位到 `cases/` 里的用例背景。

---

## 8. 断言风格约定

1. **具体值,不要范围**。`assert_eq!(quota, 798)`,不写 `assert!(quota > 700)`。
   契约测试的价值就在于精确。
2. **失败信息带上下文**。用 `pretty_assertions::assert_eq!` 输出结构化 diff。
3. **契约类断言注明溯源**。
   ```rust
   // 溯源:new-api service/text_quota_test.go
   //       TestCalculateTextQuotaSummaryUnifiedForClaudeSemantic
   assert_eq!(quota, 1488);
   ```
4. **并发断言要断言不变量而非时序**。不断言"第 3 个请求失败",而断言
   "成功数 == 3 且余额 == 100 且任何时刻余额 ≥ 0"。
5. **负向测试要断言错误类型,不只是 `is_err()`**。
   `assert!(matches!(e, AppError::QuotaExceeded))`。

---

## 9. 当前状态与红灯预期

骨架阶段 `code/backend` 全部实现为 `todo!()`。因此:

> **本目录的测试在实现补齐前应当全部失败(panic: not yet implemented)。这是 TDD 的
> 预期状态,不是缺陷。**

CI 在骨架阶段应将测试步骤标记为 `allow_failure`,待实现推进后逐步收紧。
`test/README.md` 记录了各模块的红→绿进度表。

---

## 文档历史

| 版本 | 日期 | 作者 | 变更 |
|------|------|------|------|
| v1.0 | 2026-09-10 | Architecture Team | 初始编写;补齐 doc 目录此前缺失的测试策略与用例基准 |
