# 05. Repository 集成测试用例

**版本**:v1.0 | **层**:L2(需 openGauss)
**测试代码**:`test/src/integration_repository.rs`、`integration_idempotency.rs`
**基准来源**:TEST-VECTORS.md §8/§9;ADR-005

## 为什么必须用真实数据库

本组用例验证的三条不变量,**mock 一条都验证不了**:

| 不变量 | 依赖的数据库语义 |
|---|---|
| 条件原子扣减 | `UPDATE ... WHERE quota >= $1` 的**影响行数** |
| 兑换/订阅预扣 | `SELECT ... FOR UPDATE` 的**行锁** |
| 支付/预扣幂等 | 唯一索引的**冲突检测** |

用 mock 写的"并发测试"只是在测 mock 自己的实现,毫无价值。

## 环境

```bash
# testcontainers 自动起容器(推荐)
cargo test integration_

# 或指向已有实例
DATABASE_URL=postgres://sea_weir:pass@localhost:5432/sea_weir_test cargo test integration_
```

dev 环境缺 `DATABASE_URL` 时告警跳过;**test 环境(`STRICT=1`)直接失败**。

## 用例清单

### 额度不变量

| 编号 | 名称 | 类型 | 优先级 |
|---|---|---|---|
| TC-INT-REP-001-BND | 条件扣减边界(5 组)★ | 边界 | P0 |
| TC-INT-REP-002-CNC | 并发扣减不为负 ★★ | 并发 | P0 |
| TC-INT-REP-003-CNC | 增减交叉守恒 | 并发 | P0 |
| TC-INT-REP-004-POS | unlimited_quota 不扣库 | 正向 | P1 |

### 事务一致性

| 编号 | 名称 | 类型 | 优先级 |
|---|---|---|---|
| TC-INT-REP-010-POS | sync_abilities 失败整体回滚 ★ | 正向 | P0 |
| TC-INT-REP-011-POS | 禁用渠道不出现在 list_enabled | 正向 | P0 |
| TC-INT-REP-012-POS | 多 key 按粒度禁用 | 正向 | P1 |

### 软删除与索引

| 编号 | 名称 | 类型 | 优先级 |
|---|---|---|---|
| TC-INT-REP-020-POS | 软删除后同名可重建 ★ | 正向 | P1 |
| TC-INT-REP-021-NEG | 未删除时同名冲突 | 负向 | P1 |
| TC-INT-REP-030-POS | log_dsn 为空回落主库 | 正向 | P2 |

### 幂等(`integration_idempotency.rs`)

| 编号 | 名称 | 类型 | 优先级 |
|---|---|---|---|
| TC-INT-IDM-001-CNC | 兑换码并发只成功一次 ★★ | 并发 | P0 |
| TC-INT-IDM-002-IDM | 支付回调重复投递幂等 ★ | 幂等 | P0 |
| TC-INT-IDM-003-CNC | 支付回调并发只入账一次 | 并发 | P0 |
| TC-INT-IDM-004-IDM | 订阅预扣 request_id 幂等 | 幂等 | P0 |
| TC-INT-IDM-005-POS | 多订阅按 end_time 升序消耗 | 正向 | P1 |
| TC-INT-IDM-006-NEG | 已结算不得退款 ★ | 负向 | P0 |
| TC-INT-IDM-007-CNC | 任务终态 CAS 只成功一次 ★ | 并发 | P0 |
| TC-INT-IDM-008-NEG | CAS from 不匹配不改库 | 负向 | P1 |
| TC-INT-IDM-009-POS | 崩溃对账捞出悬挂预扣 ★ | 正向 | P0 |

## 关键用例详述

### TC-INT-REP-002-CNC:并发扣减不为负 ★★

```yaml
背景: >
  本项目账务正确性的核心断言,也是相对 new-api 的实质改进点。
  new-api 的 model/user.go:928 是无条件的 `UPDATE users SET quota = quota - ?`,
  在并发下会把余额扣成负数(用户可以透支)。
  sea-weir 用条件守卫 `AND quota >= $1`,靠影响行数判断是否成功。

setup:
  user.quota: 1000
action:
  "10 个并发 try_decrease_quota(user_id, 300)"
expected:
  success_count: 3          # floor(1000/300)
  final_quota:   100
  invariant:     "任何时刻 quota >= 0"

失败含义: >
  success_count == 10 且 final_quota == -2000 → 守卫没加,是资金漏洞
  success_count < 3 → 可能有不必要的悲观锁,影响吞吐
```

### TC-INT-IDM-001-CNC:兑换码并发只成功一次 ★★

```yaml
背景: >
  典型的资金损失漏洞场景。若实现是"先 SELECT 查状态,再 UPDATE 置已用",
  10 个并发会全部通过检查,用户额度被加 10 次。
  必须用事务 + FOR UPDATE 行锁,或依赖状态字段的 CAS。

setup:
  redemption: { key: "R001", quota: 1000, status: UNUSED }
action:
  "10 个并发 redeem('R001', user_id)"
expected:
  success_count:  1
  quota_delta:    1000        # 不是 10000
  topup_log_count: 1
```

### TC-INT-IDM-009-POS:崩溃对账 ★

```yaml
背景: >
  预扣与结算之间进程崩溃,会留下"钱扣了但服务没提供"的悬挂记录。
  没有对账任务,这笔钱就永久扣在用户头上,只能靠客服工单发现。
  ADR-005 要求启动时 + 每小时执行对账。

setup:
  "构造一条 pre_consume 后既未 settle 也未 refund 的记录,时间戳设为 1 小时前"
action:
  "find_dangling(now - 3600) → 执行对账"
expected:
  - "悬挂记录被捞出"
  - "额度已退回用户"
  - "记录被标记为已处理(不会重复退款)"
```
