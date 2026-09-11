# 02. 计价公式单元测试用例

**版本**:v1.0
**日期**:2026-09-10
**层**:L1 单元(零外部依赖)
**模块**:`sea_weir_core::relay::billing::calculate_quota`
**测试代码**:`test/src/unit_billing.rs`
**基准来源**:`doc/architecture/TEST-VECTORS.md` §1;4 组核心向量直接移植自
new-api `service/text_quota_test.go` 的 Go 单测断言
**公式**:`doc/architecture/CONTRACTS.md` §6

---

## 为什么这组用例优先级最高

计价错误是**持续性资金损失**:每个请求都错一点,且往往在对账时才被发现。
与之相比,功能性 bug 会被用户立刻报告。

new-api 的计价逻辑有若干非直觉分支(usage 语义决定是否扣减 prompt、
缓存写入分 5m/1h 档、audio 计价在倍率之外、取整方式),移植时任一处理解偏差都会
产生系统性偏账。因此本组用例的期望值**全部来自 new-api 的实际断言**,不从公式推导。

---

## 用例清单

| 编号 | 名称 | 类型 | 优先级 | 基准来源 |
|---|---|---|---|---|
| TC-UNI-BIL-001-POS | OpenAI 语义 + 缓存读 | 正向 | P0 | 单测 |
| TC-UNI-BIL-002-POS | Claude 语义 + 缓存写入分档 | 正向 | P0 | 单测 |
| TC-UNI-BIL-002-POS-B | 入口格式不影响计费 | 正向 | P0 | 单测 |
| TC-UNI-BIL-003-POS | Claude 语义 + 仅缓存写入 | 正向 | P0 | 单测 |
| TC-UNI-BIL-004-POS | 遗留 Claude 派生 OpenAI usage | 正向 | P0 | 单测 |
| TC-UNI-BIL-005-BND | 取整必须 half-away-from-zero ★ | 边界 | P0 | 源码 |
| TC-UNI-BIL-005-BND-B | 小数中点取整(2.5 → 3) | 边界 | P0 | 源码 |
| TC-UNI-BIL-006-POS | 按次计费(3 组参数) | 正向 | P0 | 源码 |
| TC-UNI-BIL-007-BND | total_tokens=0 → quota=0 | 边界 | P1 | 源码 |
| TC-UNI-BIL-007-BND-B | 倍率非零时下限为 1 | 边界 | P1 | 源码 |
| TC-UNI-BIL-007-BND-C | 倍率为 0 不触发下限 | 边界 | P1 | 源码 |
| TC-UNI-BIL-008-POS | 额度换算(3 组) | 正向 | P2 | 源码 |
| TC-UNI-BIL-009-BND | 无浮点累积误差 | 边界 | P1 | 源码 |

---

## TC-UNI-BIL-001-POS:OpenAI 语义 + 缓存读

```yaml
id: TC-UNI-BIL-001-POS
title: "OpenAI 语义下 prompt 已含 cache,须先扣减再按 CacheRatio 计价"
category: positive
priority: P0
source: "new-api service/text_quota_test.go::TestCalculateTextQuotaSummarySeparatesOpenRouterCacheReadFromPromptBilling"

背景: >
  OpenAI 协议的 prompt_tokens 是**总输入**,已经包含了缓存命中的 token。
  若不先扣减,缓存部分会被按全价 + 缓存价重复计费两次。

input:
  usage:
    prompt_tokens: 2604
    completion_tokens: 383
    cached_tokens: 2432
  price:
    model_ratio: 1
    group_ratio: 1
    completion_ratio: 1
    cache_ratio: 0.1

计算:
  - "base     = 2604 − 2432 = 172"
  - "cache_q  = 2432 × 0.1  = 243.2"
  - "prompt_q = 172 + 243.2 = 415.2"
  - "compl_q  = 383 × 1     = 383"
  - "quota    = (415.2 + 383) × (1 × 1) = 798.2 → round → 798"

expected:
  quota: 798

失败含义: >
  若得到 3036(= 2604 + 383 + 2432×0.1 未扣减),说明漏了 OpenAI 语义的扣减分支,
  所有走缓存的请求都会被多收费。
```

---

## TC-UNI-BIL-002-POS:Claude 语义 + 缓存写入分档

```yaml
id: TC-UNI-BIL-002-POS
title: "Claude 语义下 prompt 不含 cache,不扣减;缓存写入按 5m/1h 分档计价"
category: positive
priority: P0
source: "new-api service/text_quota_test.go::TestCalculateTextQuotaSummaryUnifiedForClaudeSemantic"

背景: >
  Anthropic 把 cache_read / cache_creation 与 input_tokens **分开上报**,
  prompt_tokens 不含它们。若沿用 OpenAI 的扣减逻辑会把 base 扣成负数。
  同时 Anthropic 的缓存写入有 5 分钟与 1 小时两种 TTL,单价不同。

input:
  usage:
    prompt_tokens: 1000
    completion_tokens: 200
    cached_tokens: 100
    cache_creation_tokens: 50
    claude_cache_creation_5m_tokens: 10
    claude_cache_creation_1h_tokens: 20
  price:
    model_ratio: 1
    group_ratio: 1
    completion_ratio: 2
    cache_ratio: 0.1
    cache_creation_ratio: 1.25
    cache_creation_5m_ratio: 1.25
    cache_creation_1h_ratio: 2

计算:
  - "base     = 1000                                   # 不扣减"
  - "cache_q  = 100 × 0.1 = 10"
  - "remaining= 50 − 10 − 20 = 20"
  - "create_q = 20×1.25 + 10×1.25 + 20×2 = 25 + 12.5 + 40 = 77.5"
  - "compl_q  = 200 × 2 = 400"
  - "quota    = (1000 + 10 + 77.5 + 400) × 1 = 1487.5 → round → 1488"

expected:
  quota: 1488
```

### TC-UNI-BIL-002-POS-B:入口格式不影响计费

```yaml
id: TC-UNI-BIL-002-POS-B
title: "入口 OpenAI 但最终上游为 Claude 时,计费结果与入口即 Claude 完全相同"
category: positive
priority: P0
source: "同上测试中的 require.Equal(messageSummary.Quota, chatSummary.Quota)"

背景: >
  用户可以用 OpenAI SDK 调 Claude 模型(走格式转换)。计费必须看**最终上游格式**
  的 usage 语义,而不是入口格式。搞错会让同一个模型经两条入口产生不同账单。

expected:
  两条路径的 quota 完全相等,且均为 1488
```

---

## TC-UNI-BIL-005-BND:取整方式 ★

```yaml
id: TC-UNI-BIL-005-BND
title: "取整必须是 half-away-from-zero,不是 Rust 默认的银行家舍入"
category: boundary
priority: P0
source: "shopspring/decimal Round(0) 语义;TEST-VECTORS.md TV-BILL-005"

背景: >
  这是 Go → Rust 移植的必踩坑。
  - Go 的 shopspring/decimal `Round(0)`:half away from zero
  - Rust 的 rust_decimal `.round()`:MidpointNearestEven(银行家舍入)
  两者在 `x.5` 且整数部分为偶数时结果差 1。单次差 1 quota 看似无害,
  但在高频请求下会累积成可观的对账差异,且极难定位。

input:
  usage: { prompt_tokens: 1597, completion_tokens: 0 }
  price: { model_ratio: 0.5, group_ratio: 1, completion_ratio: 1 }

计算:
  - "quota = 1597 × 0.5 = 798.5"
  - "half-away-from-zero → 799"
  - "banker's rounding   → 798(798 为偶数)"

expected:
  quota: 799

实现要求: |
  quota.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)

失败含义: >
  得到 798 说明用了 .round(),必须改为显式指定舍入策略。
```

---

## TC-UNI-BIL-007:边界与下限

```yaml
- id: TC-UNI-BIL-007-BND
  title: "total_tokens == 0 时 quota 为 0"
  背景: "空请求(如仅探测连通性)不应计费,即使倍率非零"
  input:  { usage: {prompt_tokens: 0, completion_tokens: 0}, price: {model_ratio: 1} }
  expected: { quota: 0 }

- id: TC-UNI-BIL-007-BND-B
  title: "倍率非零但算得 ≤ 0 时,下限保护为 1"
  背景: "防止极小额请求完全免费,被用来白嫖"
  input:  { usage: {prompt_tokens: 1}, price: {model_ratio: 0.0000001} }
  expected: { quota: 1 }

- id: TC-UNI-BIL-007-BND-C
  title: "倍率为 0(免费模型)时不触发下限保护"
  背景: "免费模型必须真的免费,否则运营配置的免费策略失效"
  input:  { usage: {prompt_tokens: 1000, completion_tokens: 1000}, price: {model_ratio: 0} }
  expected: { quota: 0 }
```

---

## 待补充(实现阶段)

骨架的 `Usage` 结构尚未建模 Claude 的分档缓存字段与 usage 语义标记。
实现 TC-UNI-BIL-002 / 003 / 004 前需先补:

- `Usage.claude_cache_creation_5m_tokens` / `_1h_tokens`
- `Usage.usage_semantic`(`"openai"` / `"anthropic"`)
- `PriceData.cache_creation_5m_ratio` / `_1h_ratio`
- `PriceData.other_ratios: Vec<Decimal>`
- 判定"遗留 Claude 派生"的辅助函数(TC-UNI-BIL-004 的分支)

这属于**测试驱动出的设计缺口** —— 正是 TDD 的预期产出。
