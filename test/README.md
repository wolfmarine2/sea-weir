# sea-weir 测试目录

测试用例文档(`cases/`)、测试代码(`src/`)与按环境编排的入口(`env/`)。
编排结构参照 service-auth。

**设计文档**:`doc/test-design.md`(分层与门禁)、
`doc/architecture/TEST-VECTORS.md`(黄金用例)、
`doc/architecture/adr/ADR-010-testing-strategy.md`(策略决策)。

---

## 当前状态:L1 已转绿(2026-09-10 实现推进)

骨架阶段 L1 全红(62 通过 / 103 失败,失败全部为 `panic: not yet implemented`)。
实现补齐 L1 各模块后:

```
# test/ 后端
test result: FAILED. 254 passed; 3 failed; 5 ignored      # 全量
test result: ok.     165 passed; 0 failed                 # cargo test --lib unit_

# code/frontend
Test Files  5 passed (5); Tests  25 passed (25)           # vitest
tsc -b: 0 错误                                            # ADR-009 门禁
```

- **L1 单元层 165/165 全绿**:计费公式、选路加权、重试/禁用判定、格式转换、
  流式改写、常量取值、错误出口、分页、脱敏、适配器共性、注册表覆盖(35+10)
- **L3 契约层全绿**(34):基线为真实录制
- **L2 集成层**:无依赖时按 dev 策略跳过(`require_dep`);test 环境 `STRICT=1` 拒绝跳过
- **仍红 3 条(预期)**:`integration_pipeline` 的 001/002/004 是 **TODO(实现) 占位用例**
  (断言值硬编码为 0),需先落地 `relay::pipeline` 与 Repository mock 才能改写为真实断言;
  这是下一个实现单元,不是缺陷

### 本机验证的构建约束

| 约束 | 说明 |
|---|---|
| Rust ≥ 1.85 | 依赖树中 `clap_lex`、`base64ct` 等已用 edition 2024;骨架原 pin 的 1.82 无法解析 |
| **libssl-dev** | `webauthn-rs 0.5` 硬依赖 openssl,是全栈唯一的非 rustls TLS 依赖 |

因此依赖 `sea-weir-server` 的用例(路由自省、认证中间件)置于 **`server-tests` feature** 之后,
无 libssl-dev 的环境下其余 L1 照常运行:

```bash
cargo test --lib unit_                      # 不需要 libssl-dev
cargo test --lib --features server-tests    # 需要 libssl-dev
```

---

## 编排结构

| 文件 | 用途 |
|---|---|
| `cicd.sh` | CD test 阶段统一入口,按 `ENV` 分发到 `env/<env>.sh` |
| `env/common.sh` | 公共阶段执行器(审查/L1/L2/L3/L4/性能/覆盖率)+ 依赖参数推导 |
| `env/dev.sh` / `test.sh` / `prod.sh` | 分环境策略 |
| `env/record-baseline.sh` | 从 new-api 实例录制契约基线 |
| `cases/` | 用例文档 + fixtures |
| `src/` | 测试代码(独立 Rust crate) |

## 分环境策略

| 环境 | 策略 |
|---|---|
| **dev** | 审查 + L1 必跑;L2/L3 依赖不可达时**告警跳过**;L4 跳过 |
| **test** | **全量门禁**:`DATABASE_URL` / `REDIS_URL` / `SEAWEIR_HTTP_BASE_URL` 必填,缺一即失败 |
| **prod** | 部署后冒烟:审查 + L1 + L4;性能基准默认禁用(需 `PROD_ALLOW_LOAD=1`) |

**test 环境为什么不允许跳过**:允许跳过就会出现"依赖没配好 → 套件跳过 → CI 全绿"的
假绿。这在 service-auth 上有过教训,直接沿用其结论。

## 使用

```bash
# 流水线
NAMESPACE=loong-service-sea-weir ENV=dev  bash test/cicd.sh
NAMESPACE=loong-service-sea-weir ENV=test bash test/cicd.sh
NAMESPACE=loong-service-sea-weir ENV=prod bash test/cicd.sh
```

```bash
# 本地:只跑 L1(零依赖,秒级)
cd test && cargo test --lib unit_
```

```bash
# 本地:跑 L2(需 Docker,testcontainers 自动起容器)
cd test && cargo test --lib integration_ -- --test-threads=1
```

```bash
# 录制契约基线(需一个运行中的 new-api)
NEWAPI_BASE_URL=http://localhost:3000 NEWAPI_ADMIN_TOKEN=xxx \
  bash test/env/record-baseline.sh
```

调试开关:`SKIP_REVIEW` / `SKIP_FUNC` / `SKIP_PERF` / `SKIP_COVERAGE` `=1`。

---

## 用例与代码对应

`cases/` 的每个 `TC-*` 编号在 `src/` 有同名测试函数(编号小写下划线 + 语义后缀),
便于从失败的测试反查用例背景。

| 用例文档 | 测试代码 | 层 |
|---|---|---|
| `01-unit-types.md` | `unit_constants.rs`、`unit_mask.rs`、`unit_pagination.rs`、`unit_error_exit.rs` | L1 |
| `02-unit-billing.md` | `unit_billing.rs` | L1 |
| `03-unit-select-retry.md` | `unit_select.rs`、`unit_retry.rs` | L1 |
| `04-unit-adaptors.md` | `unit_adaptor_common.rs` | L1 |
| `05-integration-repository.md` | `integration_repository.rs`、`integration_idempotency.rs` | L2 |
| `06-unit-convert-stream.md` | `unit_convert.rs`、`unit_stream.rs` | L1 |
| `07-integration-auth.md` | `integration_auth.rs`、`integration_cache.rs` | L2 |
| `08-integration-pipeline.md` | `integration_pipeline.rs`、`integration_task_polling.rs` | L2 |
| `09-contract-admin.md` | `contract_admin_api.rs` | L3 |
| `10-contract-relay.md` | `contract_relay_api.rs` | L3 |
| `11-route-completeness.md` | `unit_route_completeness.rs` | L1 |
| `12-live-smoke.md` | `live_smoke.rs` | L4 |
| `13-performance.md` | `perf.rs` | — |
| `14-frontend.md` | `code/frontend/src/**/*.test.ts(x)` | L1/L3 |

---

## 溯源纪律

> **凡涉及「与 new-api 一致」的断言,期望值必须可溯源到 new-api 的源码行或录制的响应,
> 不接受「设计文档这么写」作为依据。**

理由见 `doc/test-design.md` §1:v1.0 的设计文档带着 4 处契约偏差
(camelCase 字段名、0 起分页、错误的计费公式、断链的端点清单)通过了首轮人工审查。
按文档写测试会把偏差固化成"正确行为"。

计价的 4 组核心向量直接移植自 new-api 自带的 Go 单测断言,可信度最高。

---

## 红绿进度表

随实现推进更新。`—` = 未开始,`红` = 测试已写待实现,`绿` = 通过。

| 模块 | 用例数 | 状态 | 阻塞项 |
|---|---|---|---|
| `types::constants` / `error` | 22 | 红 | — |
| `relay::billing::calculate_quota` | 21 | 红 | — |
| `relay::select` | 7 | 红 | — |
| `relay::autoban` | 10 | 红 | — |
| `dto::common`(分页) | 3 | 红 | — |
| `token::mask_token_key` | 3 | 红 | — |
| `adaptors::sync::common` | 10 | 红 | — |
| `relay::convert` | 8 | 红 | — |
| `relay::stream` | 9 | 红 | 需先录制 SSE 基线 |
| `router` 完整性 | 8 | 红 | 需 `--features server-tests`(libssl-dev) |
| Repository 额度/事务 | 10 | 红 | 需 testcontainers 编排 |
| 幂等不变量 | 9 | 红 | 同上 |
| 认证与缓存 | 22 | 红 | 同上 + `server-tests` feature |
| 中继管线 | 10 | 红 | — |
| 任务轮询 | 8 | 红 | — |
| 契约(管理面) | 13 | 红 | **需先录制基线** |
| 契约(中继面) | 8 | 红 | **需先录制基线** |
| 冒烟 | 8 | 红 | 需已部署实例 |

---

## 测试驱动出的设计缺口

TDD 的预期产出之一:测试先行会暴露骨架设计的不足。**实际发现 4 处**(其中两处是
真实编译/运行才暴露的),均已补进 `code/backend` 骨架:

### 1. `Usage` 缺 Claude 分档缓存字段(已补)

TC-UNI-BIL-002 / 003 / 004 要求,骨架原本无法表达 Anthropic 的 5m/1h 分档计价。已补:

- `Usage.claude_cache_creation_5m_tokens` / `_1h_tokens`
- `Usage.usage_semantic: Option<UsageSemantic>` + `effective_semantic()`
- `Usage::is_legacy_claude_derived()` —— TC-UNI-BIL-004 的隐蔽分支
- `PriceData.cache_creation_5m_ratio` / `_1h_ratio` / `audio_input_price` / `other_ratios`

### 2. `router` 缺路由自省能力(已补)

TC-UNI-RTE-001~008 要求。axum 不暴露路由枚举 API,必须注册时主动登记。
已补 `router/manifest.rs` 的 `RouteManifest` / `RouteEntry`。
顺带收益:路由数可与 CONTRACTS 附录 A 自动对表;可作为 `/api/performance` 的诊断输出。

### 3. `sea-weir-server` 缺 lib 目标(已补)

原骨架只有 `[[bin]]`,测试 crate 无法 import。已改为 lib + bin 双目标 ——
这本就是需要被测试的服务端该有的结构。

### 4. `sea-weir-types` 漏声明 `bytes` 依赖(已补)

`dto/relay.rs` 的 `UpstreamResponse.body` 用了 `bytes::Bytes` 但 Cargo.toml 未声明。
真实编译才暴露。

---

## 覆盖率门禁

| 范围 | 阈值 |
|---|---|
| `core::relay::billing`、`repository` 额度方法 | ≥ 90% |
| 其余 | ≥ 70% |

账务模块单独设高门槛的理由:计价错误是**持续性资金损失**,且往往在对账时才发现。

---

## 文档历史

| 版本 | 日期 | 作者 | 变更 |
|---|---|---|---|
| v1.0 | 2026-09-10 | Architecture Team | 初始编写;14 份用例文档 + 23 个测试模块 / 320 个测试点,基准取自 new-api 实际行为;L1 实跑验证红灯状态 |
