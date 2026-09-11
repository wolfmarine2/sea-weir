# 08. 中继管线与任务轮询集成测试用例

**版本**:v1.0 | **层**:L2(mock Repository + wiremock 上游)
**测试代码**:`test/src/integration_pipeline.rs`、`integration_task_polling.rs`
**基准来源**:SEQ-003、SEQ-005、SEQ-006;ADR-005

## 定位

管线是把选路、计费、适配器、流式、禁用串起来的编排层。
单看各模块都对,串起来仍可能出错 —— 典型是**重试时漏退款**。

本组的核心断言是**账目守恒**:

> 用户额度的净变化 == 实际成功那次请求的 quota,与重试了几次无关。

不需要真实数据库(用 mockall 的 Repository 替身即可断言调用次数),
也不需要真实上游(wiremock 打桩)。

## 用例清单

### 中继管线

| 编号 | 名称 | 类型 | 优先级 | 核心断言 |
|---|---|---|---|---|
| TC-INT-PIP-001-POS | 首次成功 | 正向 | P0 | 预扣 1、退款 0 |
| TC-INT-PIP-002-POS | 重试后成功,账目守恒 ★ | 正向 | P0 | 预扣 2、退款 1、净扣 1 份 |
| TC-INT-PIP-003-NEG | 重试耗尽全部退款 | 负向 | P0 | 净变化 0 |
| TC-INT-PIP-004-NEG | 不可重试立即返回 | 负向 | P0 | 尝试 1 次 |
| TC-INT-PIP-005-NEG | 无可用渠道不扣费 ★ | 负向 | P0 | 预扣 0 次 |
| TC-INT-PIP-006-NEG | 预扣失败不调上游 ★ | 负向 | P0 | 上游收到 0 请求 |
| TC-INT-PIP-007-POS | retry_count 递增影响选路 | 正向 | P0 | 档位正确 |
| TC-INT-PIP-008-POS | 401 触发自动禁用 | 正向 | P0 | 渠道被摘除 |
| TC-INT-PIP-009-POS | 资金源失败回滚令牌 ★ | 正向 | P0 | 令牌额度不变 |
| TC-INT-PIP-010-POS | 结算补退差额 | 正向 | P0 | 净扣实际值 |

### 任务轮询

| 编号 | 名称 | 类型 | 优先级 |
|---|---|---|---|
| TC-INT-TSK-001-POS | 提交全额预扣(无信任旁路) | 正向 | P0 |
| TC-INT-TSK-002-POS | remix 锁定原渠道 | 正向 | P0 |
| TC-INT-TSK-003-POS | 完成按适配器补差 | 正向 | P0 |
| TC-INT-TSK-004-POS | 返回 None 时按 tokens 重算 | 正向 | P1 |
| TC-INT-TSK-005-POS | 失败退款 | 正向 | P0 |
| TC-INT-TSK-006-POS | 超时清扫退款 ★ | 正向 | P0 |
| TC-INT-TSK-007-CNC | 多节点轮询退款一次 ★ | 并发 | P0 |
| TC-INT-TSK-008-POS | 轮询周期 15s | 正向 | P2 |

## 关键用例详述

### TC-INT-PIP-002-POS:重试后成功,账目守恒 ★

```yaml
背景: >
  每次尝试都会预扣。若失败路径漏了退款,用户会因为一次自动重试被扣两份钱,
  而且**看起来一切正常**(请求成功了),只有对账时才会发现总额对不上。
  这是最隐蔽的一类计费缺陷。

setup:
  渠道 A: "wiremock 返回 500"
  渠道 B: "wiremock 返回正常响应"
  retry_times: 1

expected:
  try_decrease_quota 调用次数: 2      # 两次尝试各预扣
  increase_quota(退款) 调用次数: 1    # 失败的那次必须退
  用户额度净变化: "== 一次成功请求的 quota"

失败含义: >
  退款 0 次 → 漏退,用户被多扣
  预扣 1 次 → 重试时没重新预扣,可能用了第一次的预扣额度(渠道切换后价格可能不同)
```

### TC-INT-PIP-005/006:扣费与调用的顺序 ★

```yaml
背景: >
  这两条测的是同一件事的两面:**扣费必须在调用上游之前,选路必须在扣费之前**。

  - 选路在扣费前:无可用渠道时,用户不该为一个从未发出的请求付费
  - 扣费在调用前:余额不足时,我们不该先付了上游的钱再发现收不到用户的钱

  顺序写反不会报错,只会在特定场景下产生资损或用户投诉。

TC-INT-PIP-005:
  setup:    "abilities 表为空"
  expected: { status: 404, error_code: model_not_found, pre_consume_calls: 0 }

TC-INT-PIP-006:
  setup:    "用户余额 0"
  expected: { status: 403, error_code: insufficient_quota, upstream_requests: 0 }
```

### TC-INT-PIP-009-POS:资金源失败回滚令牌 ★

```yaml
背景: >
  预扣是两步:先扣令牌 remain_quota,再扣资金源(钱包或订阅)。
  第二步失败时若不回滚第一步,令牌额度就凭空少了一笔 ——
  用户会发现"令牌余额少了但钱包没动,也没有对应的消费记录"。

setup:
  token.remain_quota: 1000    # 充足
  user.quota: 0               # 不足
expected:
  - "try_decrease_quota(token) 被调 1 次"
  - "increase_quota(token) 回滚被调 1 次"
  - "令牌 remain_quota 最终仍为 1000"
  - "返回 insufficient_quota"
```

### TC-INT-TSK-007-CNC:多节点轮询退款一次 ★

```yaml
背景: >
  ≥2 副本部署下,多个节点会同时轮询同一批任务。
  终态迁移必须用 CAS,CAS 失败的节点要跳过后续的退款/结算 ——
  否则一个失败任务会被退 N 次款(N = 副本数)。

setup:
  "同一失败任务,两个 poll_loop 并发处理"
expected:
  cas_success_count: 1
  refund_count: 1
  用户额度增量: "== 一次退款金额"
```
