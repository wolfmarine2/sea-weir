# 黄金用例与判定表(Test Vectors)

**文档版本**:v1.0
**最后更新**:2026-09-10
**用途**:TDD 的**红灯依据**。每条向量给出具体输入与具体期望值,可直接转写为断言。
**相关文档**:`adr/ADR-010-testing-strategy.md`、`CONTRACTS.md`、`../test-design.md`

---

## 关于基准来源

本文档的期望值**不是设计推导出来的**,而是从 new-api 的实际实现与其自带单元测试中提取的。
每条向量标注 `来源`,以便上游演进时复核。

| 标记 | 含义 |
|---|---|
| `源码` | 从 new-api 源码逻辑推导,已人工验算 |
| `单测` | 直接取自 new-api 自带的 Go 单元测试断言,可信度最高 |
| `录制` | 需从运行中的 new-api 实例录制,fixture 落在 `test/cases/fixtures/` |

---

## 1. 计费(TV-BILL)

公式见 `CONTRACTS.md` §6。以下向量覆盖两种 usage 语义、缓存分档、取整与边界。

### TV-BILL-001:OpenAI 语义 + 缓存读

**来源**:单测 `service/text_quota_test.go:TestCalculateTextQuotaSummarySeparatesOpenRouterCacheReadFromPromptBilling`

| 输入 | 值 |
|---|---|
| prompt_tokens | 2604 |
| completion_tokens | 383 |
| cached_tokens | 2432 |
| usage 语义 | OpenAI(prompt 含 cache,须扣减) |
| ModelRatio / GroupRatio / CompletionRatio / CacheRatio | 1 / 1 / 1 / 0.1 |

```text
base     = 2604 − 2432 = 172
cache_q  = 2432 × 0.1  = 243.2
prompt_q = 172 + 243.2 = 415.2
compl_q  = 383 × 1     = 383
quota    = (415.2 + 383) × (1 × 1) = 798.2 → round → 798
```

**期望**:`quota == 798`

### TV-BILL-002:Claude 语义 + 缓存写入分档

**来源**:单测 `TestCalculateTextQuotaSummaryUnifiedForClaudeSemantic`

| 输入 | 值 |
|---|---|
| prompt / completion | 1000 / 200 |
| cached_tokens | 100 |
| cache_creation_tokens | 50(其中 5m=10、1h=20) |
| usage 语义 | Claude(prompt **不含** cache,不扣减) |
| ModelRatio / GroupRatio / CompletionRatio | 1 / 1 / 2 |
| CacheRatio | 0.1 |
| CacheCreationRatio / 5m / 1h | 1.25 / 1.25 / 2 |

```text
base     = 1000                        # Claude 语义不扣减
cache_q  = 100 × 0.1 = 10
create_q = (50 − 10 − 20) × 1.25 + 10 × 1.25 + 20 × 2
         = 25 + 12.5 + 40 = 77.5
prompt_q = 1000 + 10 + 77.5 = 1087.5
compl_q  = 200 × 2 = 400
quota    = (1087.5 + 400) × 1 = 1487.5 → round → 1488
```

**期望**:`quota == 1488`

**附加断言**:入口为 OpenAI 格式但**最终请求格式**为 Claude 时,计费结果必须与
入口即 Claude 时**完全相同**(new-api 对这两条路径有 `require.Equal` 断言)。

### TV-BILL-003:Claude 语义 + 仅缓存写入

**来源**:单测 `TestCalculateTextQuotaSummaryUsesSplitClaudeCacheCreationRatios`

| 输入 | 值 |
|---|---|
| prompt / completion | 100 / 0 |
| cache_creation_tokens | 10(其中 5m=2、1h=3) |
| ModelRatio / GroupRatio / CompletionRatio / CacheRatio | 1 / 1 / 1 / 0 |
| CacheCreationRatio / 5m / 1h | 1 / 2 / 3 |

```text
create_q = (10 − 2 − 3) × 1 + 2 × 2 + 3 × 3 = 5 + 4 + 9 = 18
quota    = (100 + 18 + 0) × 1 = 118
```

**期望**:`quota == 118`

### TV-BILL-004:遗留 Claude 派生的 OpenAI usage

**来源**:单测 `TestCalculateTextQuotaSummaryHandlesLegacyClaudeDerivedOpenAIUsage`

上游以 OpenAI 格式上报,但带了 `claude_cache_creation_5m_tokens` 字段且无 usage 语义标记 ——
此时应**按 Claude 语义处理**(不扣减 prompt)。

| 输入 | 值 |
|---|---|
| prompt / completion | 62 / 95 |
| cached_tokens | 3544 |
| cache_creation 5m | 586 |
| ModelRatio / GroupRatio / CompletionRatio / CacheRatio / CacheCreation5mRatio | 1 / 1 / 5 / 0.1 / 1.25 |

```text
quota = 62 + 3544×0.1 + 586×1.25 + 95×5
      = 62 + 354.4 + 732.5 + 475 = 1623.9 → round → 1624
```

**期望**:`quota == 1624`

> 注:new-api 该测试的注释写的是 `1624.9`,系笔误;断言值 1624 与实算 1623.9 一致。

### TV-BILL-005:取整方式(banker's vs half-away)★

**来源**:源码(`shopspring/decimal.Round` 语义)

这是 Rust 移植的**必踩坑**:`rust_decimal::Decimal::round()` 默认银行家舍入。

| 原始值 | half away from zero(正确) | 银行家舍入(错误) |
|---|---|---|
| 798.5 | **799** | 798 |
| 1487.5 | 1488 | 1488(此例两者相同,不具判别力) |
| 2.5 | **3** | 2 |
| 3.5 | 4 | 4 |
| −0.5 | **−1** | 0 |

**期望**:实现必须使用
`round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)`;
用 `.round()` 时 TV-BILL-005 必红。

### TV-BILL-006:按次计费

**来源**:源码 `relay/helper/price.go:197`

```text
quota = ModelPrice × QuotaPerUnit × GroupRatio
```

| ModelPrice | GroupRatio | 期望 quota |
|---|---|---|
| 0.02 | 1 | 10000 |
| 0.02 | 0.5 | 5000 |
| 1 | 1 | 500000 |

### TV-BILL-007:边界与下限

**来源**:源码 `service/text_quota.go:286-305`

| 场景 | 期望 |
|---|---|
| `total_tokens == 0` | `quota == 0`(即使倍率非零) |
| 倍率非零且算得 `quota ≤ 0` | `quota == 1`(下限保护,防止零额度请求) |
| 倍率为 0(免费模型) | `quota == 0`,**不触发下限保护** |
| OtherRatios = [2, 1.5] | 主体结果连乘 3 倍 |

### TV-BILL-008:额度单位换算

| quota | USD |
|---|---|
| 500000 | $1.00 |
| 250000 | $0.50 |
| 1 | $0.000002 |
| 0 | $0.00 |

---

## 2. 令牌脱敏(TV-MASK)

**来源**:源码 `model/token.go:MaskTokenKey`

| 输入 | 长度 | 期望输出 |
|---|---|---|
| `""` | 0 | `""` |
| `"ab"` | 2 | `"**"` |
| `"abcd"` | 4 | `"****"` |
| `"abcde"` | 5 | `"ab****de"` |
| `"abcdefgh"` | 8 | `"ab****gh"` |
| `"abcdefghi"` | 9 | `"abcd**********fghi"` |
| 48 位随机 key | 48 | `前4 + "**********" + 后4`,总长 18 |

规则:`len ≤ 4` → 全掩码(长度等于原长);`len ≤ 8` → `前2+"****"+后2`;
否则 → `前4+"**********"+后4`。

---

## 3. 分页(TV-PAGE)

**来源**:源码 `common/page_info.go:41-71`

| `p` 入参 | `page_size` | 期望 page | 期望 start_idx |
|---|---|---|---|
| 缺省 | 10 | 1 | 0 |
| 0 | 10 | 1 | 0 |
| 1 | 10 | 1 | 0 |
| 2 | 10 | 2 | 10 |
| 3 | 20 | 3 | 40 |
| −5 | 10 | 1 | 0 |

**别名**:`page_size` 缺省时依次回落 `ps` → `size`。
**响应形状**:恒为 `{items, total, page, page_size}`,包在 `data` 内。

---

## 4. 重试与自动禁用判定表(TV-RETRY)

**来源**:源码 `controller/relay.go:319-349` + `setting/operation_setting/status_code_ranges.go`

### 4.1 `should_retry` 判定顺序

判定**有严格短路顺序**,顺序错了会得到不同结果:

```text
1. err == nil                      → false
2. 亲和性失败跳过重试               → false
3. 是渠道错误(channel:*)          → true   ★ 优先于次数判定
4. skip_retry 标记                 → false
5. retry_times_left ≤ 0            → false
6. 指定了 specific_channel_id      → false
7. 2xx                             → false
8. status < 100 或 > 599           → true
9. IsAlwaysSkipRetryCode(错误码)   → false
10. 按可重试区间判定
```

### 4.2 状态码判定矩阵(retry_times_left > 0、非渠道错误、无特殊标记)

| 状态码 | should_retry | 理由 |
|---|---|---|
| 100 | ✔ | 1xx 区间内 |
| 200 / 201 / 204 | ✘ | 2xx 一律不重试 |
| 301 / 399 | ✔ | 3xx 区间内 |
| **400** | ✘ | 明确排除 |
| 401 | ✔ | 401–407 区间内(但会触发自动禁用) |
| 407 | ✔ | 区间上界 |
| **408** | ✘ | 明确排除 |
| 409 / 429 / 499 | ✔ | 409–499 区间内 |
| 500 / 503 | ✔ | 500–503 区间内 |
| **504** | ✘ | `alwaysSkipRetryStatusCodes` |
| 505 / 523 | ✔ | 区间内 |
| **524** | ✘ | `alwaysSkipRetryStatusCodes` |
| 525 / 599 | ✔ | 区间内 |
| 0 / 99 / 600 | ✔ | 超出 100..=599,视为网络异常 |

### 4.3 `should_disable` 判定

| 场景 | 期望 |
|---|---|
| status 401、auto_ban 开启 | ✔ 禁用 |
| status 401、auto_ban 关闭 | ✘ |
| status 403 / 429 / 500 | ✘(默认区间仅 401) |
| 命中配置的关键词 | ✔ |
| `local_error == true` | ✘(适配器自身错误,不能怪渠道) |

### 4.4 禁用粒度

| 渠道形态 | 期望 |
|---|---|
| 单 key | 整渠道置 `status = 3`(自动禁用)并从索引摘除 |
| 多 key | 仅禁用出错 key 的下标,其余 key 仍可被选中 |
| 自动禁用后渠道测试通过 + 开启自动恢复 | 重新启用 |
| 手动禁用(`status = 2`)后测试通过 | **不**自动恢复 |

---

## 5. 渠道选路(TV-SELECT)

**来源**:源码 `model/ability.go` + ADR-006

### 5.1 优先级分档

输入渠道集(priority, weight, id):

```
A(100, 5, 1)  B(100, 0, 2)  C(50, 10, 3)  D(50, 1, 4)  E(10, 0, 5)
```

| retry | 期望档位 | 候选 |
|---|---|---|
| 0 | priority=100 | {A, B} |
| 1 | priority=50 | {C, D} |
| 2 | priority=10 | {E} |
| 3 | 无 | `None`(不 panic) |

### 5.2 加权随机(权重平滑 `weight + 10`)

档 `{A(weight=5), B(weight=0)}` → 有效权重 `{15, 10}`,期望选中比例 `3:2`。

**断言方式**:固定随机种子跑 10000 次,A 的占比落在 `[0.57, 0.63]`。
**关键**:`weight = 0` 的渠道**必须有机会被选中**(平滑的意义),`B` 的选中次数 > 0。

### 5.3 auto 分组降级

分组列表 `["vip", "default"]`,模型 `gpt-4`:

| 场景 | 期望 |
|---|---|
| vip 组有可用渠道 | 选 vip 组 |
| vip 组无可用、default 有 | 降级到 default |
| 两组都无 | `None` → 出口 `model_not_found` 404 |
| 令牌 `cross_group_retry == false` | 不降级,vip 无可用即失败 |

### 5.4 指定渠道

| 场景 | 期望 |
|---|---|
| `sk-xxx-123`,用户 role ≥ 10 | 直接返回 channel_id=123,不走选路 |
| `sk-xxx-123`,用户 role == 1 | 403「普通用户不支持指定渠道」 |
| 指定渠道请求失败 | **不重试**(判定顺序第 6 条) |

---

## 6. 错误出口形状(TV-ERR)

**来源**:源码 `types/error.go` + ADR-008;完整形状需**录制**确认

> **以下取值经 2026-09-10 录制实测校正**,与首版基于源码推导的内容有出入之处已标 ★。
> 基线:`test/cases/fixtures/baseline/`。

| AppError | 管理面 HTTP | 管理面体 | 中继面 HTTP | error.code |
|---|---|---|---|---|
| `Biz(msg)` | **200** | `{success:false, message:msg}` | — | — |
| `NotFound`(记录不存在)★ | **200** | `{success:false,"record not found"}` | — | — |
| `Unauthorized` | 401 | `{success:false, message}` | 401 | **`""`(空)** ★ |
| `Forbidden`(角色不足)★ | **200** | `{success:false,"Unauthorized, insufficient privileges"}` | 403 | — |
| `QuotaExceeded` | 200 | `{success:false,"余额不足"}` | 403 | `insufficient_quota` |
| `RateLimited` | 429 | 同上 + `Retry-After` 头 | 429 | `rate_limit_exceeded` |
| `BadRequest` | 200 | 同上 | 400 | `""`(空)★ |
| 无可用渠道 ★ | — | — | **503** | `model_not_found` |
| 占位端点(POST)★ | — | — | **501** | `api_not_implemented` |
| 占位端点(GET,无 body)★ | — | — | **400** | `""` —— 被 Distribute 的 body 解析拦在 handler 之前 |
| panic | 500 | — | 500 | `new_api_panic`,message 含 request id |

**★ F1:无可用渠道是 503 不是 404**。这条差别很大:客户端 SDK 对 503 会自动重试、
对 404 不会。按 404 实现会让上游故障时客户端不重试。

**★ F2:中继面 `error.type` 恒为 `new_api_error`**,不是 OpenAI 标准的
`authentication_error` / `invalid_request_error`。`error.code` 在鉴权与参数错误时为**空串**,
只有业务可识别的错误(如 `model_not_found`、`api_not_implemented`)才填值。
OpenAI 错误体还带一个 `param` 字段(占位端点响应中可见)。

**★ F5:管理面角色不足返回 200,不是 403**。源码 `middleware/auth.go` 的 `authHelper`:
`if role.(int) < minRole { c.JSON(http.StatusOK, gin.H{"success": false, ...}) }`。
只有**未认证**(无 session 且无 access token)才 401。403 仅用于中继面。
这条若实现错,前端 axios 拦截器会把越权当网络错误处理,丢失真实提示。

**★ F3:request id 嵌在 `message` 文本尾部**,格式 `... (request id: <id>)`,
不是独立字段也不在响应头。客户端要拿 request id 只能从文案里正则抠 ——
sea-weir 若"改良"成独立字段,会让依赖该文案的排障脚本失效。

### 6.1 中继面错误体形状(按入口格式)

| 格式 | 形状 |
|---|---|
| OpenAI | `{"error":{"message":…,"type":…,"code":…}}` |
| Claude | `{"type":"error","error":{"type":…,"message":…}}` |
| Gemini | Gemini 原生错误体 |
| Midjourney | `{"code":…,"description":…,"result":…}`,`code == 30` 对应 HTTP 429 |

**关键断言**:管理面业务错误是 **HTTP 200**。若实现返回 400/500,客户端与前端会走错分支。

---

## 7. 认证与鉴权(TV-AUTH)

### 7.1 sk-token 提取优先级

**来源**:源码 `middleware/auth.go`

| # | 来源 | 适用路径 |
|---|---|---|
| 1 | `Authorization: Bearer sk-…` | 通用 |
| 2 | `x-api-key` | Claude `/v1/messages` |
| 3 | `?key=` / `x-goog-api-key` | Gemini `/v1beta/*` |
| 4 | `Sec-WebSocket-Protocol` | `/v1/realtime` |
| 5 | `mj-api-secret` | `/mj/**` |

**断言**:同时提供多个来源时,按上表**顺序取第一个命中**。

### 7.2 令牌校验

| 场景 | 期望 |
|---|---|
| `status != 1` | 401 |
| `expired_time != -1` 且已过期 | 401 |
| `expired_time == -1` | 永不过期,放行 |
| `unlimited_quota == false` 且 `remain_quota ≤ 0` | 403 `insufficient_quota` |
| `unlimited_quota == true` 且 `remain_quota == 0` | 放行 |
| `allow_ips` 非空且客户端 IP 未命中 CIDR | 403 |
| 归属用户 `status != 1` | 401「用户已被封禁」 |
| 请求模型不在 `model_limits` 内(且已启用) | 403 `permission_error` |

### 7.3 管理面会话

| 场景 | 期望 |
|---|---|
| 缺 `New-Api-User` 头 | 401 |
| `New-Api-User` 与会话 user id 不一致 | 401(防串号) |
| jti 在 Valkey 吊销列表 | 401 |
| 无会话但有合法 `Authorization: Bearer <access_token>` | 放行 |
| role < 端点要求 ★ | **200 + `success:false`**(不是 403,见 §6 F5) |
| **缓存不可达** | 401(fail-close,**不得**放行) |

### 7.4 角色闸门边界

| role | UserAuth(≥1) | AdminAuth(≥10) | RootAuth(≥100) |
|---|---|---|---|
| 0 | ✘ | ✘ | ✘ |
| 1 | ✔ | ✘ | ✘ |
| 9 | ✔ | ✘ | ✘ |
| 10 | ✔ | ✔ | ✘ |
| 99 | ✔ | ✔ | ✘ |
| 100 | ✔ | ✔ | ✔ |

---

## 8. 额度扣减不变量(TV-QUOTA)

**来源**:ADR-005;需**真实数据库**验证

| 场景 | 初始 quota | 扣减额 | 期望返回 | 期望余额 |
|---|---|---|---|---|
| 余额充足 | 1000 | 300 | `true` | 700 |
| 余额恰好等于扣减额 | 300 | 300 | `true` | 0 |
| 余额不足 | 299 | 300 | `false` | **299(不变)** |
| 余额为 0 | 0 | 1 | `false` | 0 |
| 扣减 0 | 100 | 0 | `true` | 100 |

**并发不变量**:初始 1000,10 个并发各扣 300 → 成功 3 次、失败 7 次、终值 100,
**任何时刻余额不为负**。

> 这是相对 new-api 的实质改进:Go 版 `model/user.go:928` 是无条件的
> `UPDATE users SET quota = quota - ?`,并发下可扣成负值。

---

## 9. 幂等不变量(TV-IDEM)

| 对象 | 幂等键 | 场景 | 期望 |
|---|---|---|---|
| 兑换码 | 状态 + 行锁 | 同码 10 并发兑换 | 恰好 1 成功,用户额度只加一次 |
| 支付回调 | `trade_no` 唯一 | 同 trade_no 重复回调 | 第 2 次返回"已处理",额度只加一次 |
| 订阅预扣 | `request_id` 唯一 | 同 request_id 重复预扣 | 只扣一次,返回首次结果 |
| 任务终态 | `status` CAS | 两节点同时迁移终态 | 恰好 1 个 `true`,退款只发生一次 |
| 结算 | 会话 `settled` 标志 | 重复 settle | 额度只变一次 |
| 退款 | 会话 `refunded` 标志 | 已 settle 后 refund | **不退款** |

---

## 10. 流式转发(TV-STREAM)

**来源**:需**录制**真实上游 SSE 流

| 场景 | 期望 |
|---|---|
| 上游流带 usage | 原样透传,不重复注入 |
| 上游无 usage + `stream_options.include_usage = true` | 在 `[DONE]` 前注入一个 usage chunk |
| 上游无 usage + `include_usage` **缺省** ★ | **仍然注入** —— 见下 |
| 入口 OpenAI、上游 OpenAI | chunk 逐字节原样转发 |
| 入口 Claude、上游 OpenAI | 重组为 `message_start` / `content_block_delta` / `message_delta` / `message_stop` |
| 空闲超过 `stream_idle_timeout` | 关闭连接并记录流状态 |
| 客户端中途断开 | 停止拉取上游;**已消费部分仍进入结算** |
| 上游流中途报错 | 按入口格式输出错误;触发退款 |
| ping 事件 | 不污染业务数据流(客户端解析时应被忽略) |

**★ F4:usage 帧是无条件注入的**。录制实测(`sse_openai_basic`,请求体不含
`stream_options`)显示:假上游发 4 帧,new-api 输出 6 帧 —— 在 `finish_reason` 帧之后、
`[DONE]` 之前多注入了一帧 `{"choices":[],"usage":{...}}`。

三份基线的对照:

| fixture | include_usage | 输出帧数 | 含 usage 帧 |
|---|---|---|---|
| `sse_openai_basic` | 缺省 | 6 | ✔ |
| `relay_openai_chat_stream` | 缺省 | 6 | ✔ |
| `relay_openai_chat_stream_usage` | true | 6 | ✔ |

首版文档写的"未请求则不注入"是**推导错误**。按那个实现,所有依赖流式 usage 做成本
统计的客户端(LangChain / LiteLLM 等)在不显式传 `include_usage` 时会统计为 0。

---

## 11. 路由完整性(TV-ROUTE)

**来源**:`CONTRACTS.md` 附录 A

| 断言 | 期望值 |
|---|---|
| 管理面路由数 | 236 |
| 中继面路由数 | 54 |
| 视频任务面路由数 | 11 |
| 兼容面路由数 | 4 |
| **合计** | **305** |
| `/mj/**` 与 `/:mode/mj/**` 双前缀 | 同一 handler,展开后 MJ 共 32 条 |
| 占位端点 | 11 条。**POST 类返回 501 `api_not_implemented`;GET 类返回 400**(被 Distribute 的 body 解析拦截,到不了 handler)。两者都不消耗额度 |
| 每条受保护路由 | 都挂了对应角色闸门(遍历断言) |

---

## 文档历史

| 版本 | 日期 | 作者 | 变更 |
|------|------|------|------|
| v1.0 | 2026-09-10 | Architecture Team | 初始编写;计费 4 组向量取自 new-api 自带单测,其余从源码推导并人工验算 |

---

## 12. 录制实测发现的其他约束(TV-MISC)

以下均来自 2026-09-10 的基线录制,首版文档未覆盖或表述不准。

### TV-MISC-001:access token 路径同样强制 `New-Api-User` 头 ★

CONTRACTS.md 原表述"无会话时接受 `Authorization: Bearer <access_token>`"容易被读成
"用 token 就不用带 New-Api-User"。实测:

| 请求 | 结果 |
|---|---|
| `Authorization: Bearer <access_token>` 单独 | **401** |
| `Authorization` + `New-Api-User: <id>` | 200 |

`New-Api-User` 在**两条认证路径下都是必需的**,不是 session 专属。

### TV-MISC-002:`/api/ratio_config` 默认关闭

实测返回 `403 {"success":false,"message":"倍率配置接口未启用"}`。
它是**开关控制的可选端点**,不是常开的公开端点。契约测试不能默认它返回 200。

### TV-MISC-003:分页响应可带域特有的额外字段

`GET /api/channel/` 的 `data` 除 `{items,total,page,page_size}` 外还有 `type_counts`
(渠道类型计数)。说明 `PageInfo<T>` 泛型不足以覆盖所有列表端点,
渠道列表需要独立的响应类型。

### TV-MISC-004:`/v1/models` 是混合形状

响应是 `{"object":"list","data":[...],"success":true}` —— 既有 OpenAI 的
`object`/`data`,又混入了管理面的 `success` 字段。移植时两个都要保留。

### TV-MISC-005:`dashboard/billing/subscription` 的完整字段

实测含 `object` / `has_payment_method` / `soft_limit_usd` / `hard_limit_usd` /
`system_hard_limit_usd` / `access_until`。首版文档只列了其中四个。

### TV-MISC-006:new-api 自身的 nil 解引用缺陷(不要复刻)

`POST /api/channel/` 若请求体缺 `channel` 字段,new-api 会 panic:
`controller/channel.go` 的 `validateChannel` 在第 438 行调用 `channel.ValidateSettings()`,
而 nil 检查在第 444 行 —— 顺序反了。

sea-weir 应返回参数错误而非 panic。这条**不是**要对齐的契约,是要修掉的缺陷;
但 panic 兜底的错误体形状(`{"error":{"type":"new_api_panic",...}}`)仍需保持一致。

### TV-MISC-007:snake_case 有既成例外 ★

"管理面全量 snake_case"这条**有例外**。录制实测发现三个 PascalCase 字段:

| 字段 | 出现位置 | 成因 |
|---|---|---|
| `DeletedAt` | `/api/token/` 的 `items[]` | GORM 的 `gorm.DeletedAt` 内嵌字段漏写 json tag,序列化成 Go 字段名 |
| `HeaderNavModules` | `/api/status` 的 `data` | 配置键,未走 snake_case 约定 |
| `SidebarModulesAdmin` | `/api/status` 的 `data` | 同上 |

这些**不是**我们想要的风格,但它们是既成契约 —— 客户端可能已在读这些键,
改名会破坏兼容。sea-weir 必须原样保留。

另需注意:`/api/status` 的 `chats` 是 **map** 而非结构体,键是用户自定义的
聊天客户端名(如 `"Cherry Studio"`)。校验命名约定时必须区分「结构字段名」与
「map 数据键」,否则会误报。校验实现见 `test/src/common.rs` 的
`PASCAL_CASE_EXCEPTIONS` 与 `looks_like_data_key`。
