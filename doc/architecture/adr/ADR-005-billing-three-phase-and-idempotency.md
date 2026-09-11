# ADR-005: 计费三段式与并发幂等

## Metadata

| 项 | 内容 |
|----|------|
| 编号 | ADR-005 |
| 标题 | 计费三段式与并发幂等(预扣-结算-退款状态机) |
| 状态 | Proposed |
| 日期 | 2026-09-10 |
| 决策者 | Architecture Team |
| 影响范围 | `sea-weir-core/relay/billing.rs`、`sea-weir-repository`(额度方法)、中继面全部端点 |
| 依赖 | ADR-003, ADR-004 |

## Status History

| 日期 | 状态 | 说明 |
|------|------|------|
| 2026-09-10 | Proposed | 初始提议 |

## Context(上下文)

### 问题陈述

中继请求的实际 token 用量在上游响应后才知道,必须先预扣防止超支、后结算多退少补、失败退款。Go 版已有 `service/billing_session.go` 做了一层收敛(`Reserve`/`Settle`/`Refund` + `settled`/`refunded` 幂等标志),但仍存在三处结构性问题:① 会话对象以 `*gin.Context` 贯穿,业务逻辑与 HTTP 层耦合,无法在后台对账任务中复用;② 底层额度扣减是无条件表达式(`model/user.go`:`UPDATE users SET quota = quota - ?`),缺少 `AND quota >= ?` 守卫,并发下可扣成负值;③ 批量异步更新开关(`BATCH_UPDATE_ENABLED`,默认 false)一旦开启即引入崩溃丢账窗口。重构需要把计费收敛为不依赖 HTTP 上下文的显式状态机,并保证并发与崩溃安全。

### 约束条件

- 兼容 new-api 计费语义:QuotaPerUnit=500000、按量/按次/阶梯三种计费、缓存/图片/音频倍率、分组倍率(GroupRatio × GroupGroupRatio)、信任额度旁路
- 资金来源两种:钱包(users.quota)与订阅(user_subscriptions),用户可选四种计费偏好
- 崩溃恢复:预扣未结算的记录必须可对账退款

### 驱动力

- 正确性:任何失败路径不多扣、不少退;并发请求不超支
- 可审计:每次扣减可追溯(消费日志含倍率明细)
- 性能:热路径不引入分布式事务

### 目标状态

计费状态机在类型层禁止非法迁移(如 Settle 后 Refund);账务路径同步落库;崩溃对账任务兜底;全链路幂等。

### 备选方案

#### 方案 A:三段式状态机 + DB 原子扣减 + 幂等键(账务同步落库)

**描述**: `BillingSession{PreConsumed → Settled | Refunded}`;额度扣减用条件原子 UPDATE;订阅预扣 request_id 幂等;统计口径(used_quota/request_count/quota_data)才批量合并。
**优点**:
- 语义显式,类型层防误用;无崩溃丢账窗口
- 原子 UPDATE 单语句完成,热路径无额外开销
**缺点**:
- 高频写直击 DB(以 openGauss 性能与连接池可承受)

#### 方案 B:沿用 Go 版批量异步更新

**描述**: 额度增量攒内存 map,5s 合并落库(即 new-api 的 `BATCH_UPDATE_ENABLED` 路径,默认关闭)。
**优点**:
- DB 写压力最低
**缺点**:
- 崩溃丢最近周期账务(已识别缺陷);余额视图滞后,超支风险

#### 方案 C:分布式事务/ saga 框架

**描述**: 引入事务协调。
**优点**:
- 形式化最强
**缺点**:
- 复杂度与延迟不可接受;本场景原子 UPDATE + 幂等已足够

### 决策依据

| 维度 | 权重 | A | B | C |
|---|---|---|---|---|
| 账务正确性 | 高 | ✔ | ✘ | ✔ |
| 热路径性能 | 高 | ✔ | ✔ | ✘ |
| 实现复杂度 | 中 | ✔ | ✔ | ✘ |
| 兼容原语义 | 高 | ✔ | ✔ | △ |

## Decision(决策)

### 选择的方案

在 sea-weir 计费设计中,面对「并发超支防控 + 崩溃可恢复 + 热路径性能」的关注点,我们选择 **三段式显式状态机 + DB 原子扣减 + 幂等键(账务同步落库,统计批量)**,而非批量异步更新或分布式事务,以获得正确性与性能的平衡,接受高频写直击数据库。

### 决策理由

1. 原子条件 UPDATE(`quota >= $1`)单语句完成校验+扣减,并发不超支
2. settled/fundingSettled/refunded 标志 + 类型化状态机,Settle 与 Refund 互斥,消除误退
3. request_id / trade_no / redemption.key 唯一约束兜底幂等;崩溃对账任务扫描「预扣未结算」退款
4. 统计口径(used_quota、quota_data)允许批量合并,丢统计不丢账

### 技术架构

```
BillingSession 状态机:
  Created → PreConsumed ──Settle(delta)──▶ Settled(终态)
                    └──────Refund()──────▶ Refunded(终态)
  不变量:Settled 后不可 Refund;delta = actual − pre_consumed(正补扣/负退还)

预扣顺序:信任旁路判定(钱包模式且余额>TrustQuota 且非强制 → 预扣 0)
        → 令牌额度(try_decrease_quota)
        → 资金源(钱包原子扣减 / 订阅 request_id 幂等 + FOR UPDATE)
        → 任一步失败回滚已扣部分

结算:rust_decimal 精确计算 → 差额补退 → 消费日志(倍率明细 JSONB)
```

### 实施计划

1. 实现 `BillingSession` 状态机与资金源抽象(Wallet/Subscription)
2. repository 提供条件原子扣减与幂等方法;唯一约束 + migrations
3. 对账任务:启动时 + 每小时扫描预扣未结算记录退款
4. 并发测试:同用户并发请求验证不超支;故障注入(上游失败/进程 kill)验证退款

## Consequences(后果)

### 正面影响

1. 崩溃丢账窗口消除;超支被原子条件杜绝
2. 计费逻辑集中,可单测(资金源 mock)
3. 与原语义逐点兼容,灰度期账务可对账

### 负面影响

1. 每次中继请求多 1-2 次 DB 写(预扣+结算)

### 缓解措施

1. 信任额度旁路减少小额请求预扣;统计口径批量;连接池与批量日志写优化

### 长期影响

billing_journal 台账可演进为完整对账/财务报表能力(见 system-design.md §11)。

## Notes

- 流式中断已结算部分不退(与 new-api 一致)
- 违规费(内容审查命中)在退款后按比例另计

## References

- `doc/system-design.md` §6.6、§8.3
- `doc/architecture/CONTRACTS.md` §6、§12
- `doc/architecture/sequence-diagram/SEQ-003`
- 相关 ADR:ADR-003、ADR-008

---

**文档版本**: v1.0
**最后更新**: 2026-09-10
**下次评审**: 2026-12-10(3个月后)
**评审责任人**: Architecture Team Lead
