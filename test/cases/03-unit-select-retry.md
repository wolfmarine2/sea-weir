# 03. 选路与重试判定单元测试用例

**版本**:v1.0 | **层**:L1 | **测试代码**:`test/src/unit_select.rs`、`unit_retry.rs`
**基准来源**:TEST-VECTORS.md §4/§5;溯源 `controller/relay.go:319-349`、`status_code_ranges.go`

## 定位

纯函数 + 表驱动,**覆盖成本最低而收益很高**的一层。
选路与重试的缺陷不会导致报错,而是表现为"某些渠道从不被使用"或
"故障渠道一直被重试"——这类问题在生产上很难通过日志发现。

## 用例清单

### 重试判定(`unit_retry.rs`)

| 编号 | 名称 | 类型 | 优先级 |
|---|---|---|---|
| TC-UNI-RTY-001-BND | 状态码判定矩阵(24 组)★ | 边界 | P0 |
| TC-UNI-RTY-002-POS | 渠道错误优先于次数判定 ★ | 正向 | P0 |
| TC-UNI-RTY-003-NEG | 亲和性跳过优先级最高 | 负向 | P0 |
| TC-UNI-RTY-004-NEG | 指定渠道不重试 | 负向 | P0 |
| TC-UNI-RTY-005-NEG | 次数耗尽不重试 | 负向 | P1 |
| TC-UNI-RTY-006-NEG | skip_retry 标记生效 | 负向 | P1 |
| TC-UNI-RTY-007-NEG | 本地错误不重试也不禁用 ★ | 负向 | P0 |
| TC-UNI-RTY-010-BND | 禁用状态码矩阵(6 组) | 边界 | P0 |
| TC-UNI-RTY-011-NEG | auto_ban 关闭时不禁用 | 负向 | P0 |
| TC-UNI-RTY-012-POS | 渠道错误判别(5 组) | 正向 | P1 |

### 选路(`unit_select.rs`)

| 编号 | 名称 | 类型 | 优先级 |
|---|---|---|---|
| TC-UNI-SEL-001-POS | 优先级分档降序 | 正向 | P0 |
| TC-UNI-SEL-002-BND | retry 超档数返回 None | 边界 | P0 |
| TC-UNI-SEL-003-POS | weight=0 仍可被选中 ★ | 正向 | P0 |
| TC-UNI-SEL-004-POS | 加权分布符合 (w+10) 占比 | 正向 | P1 |
| TC-UNI-SEL-005-BND | 空候选集返回 None | 边界 | P1 |
| TC-UNI-SEL-006-BND | 单候选恒返回 | 边界 | P2 |
| TC-UNI-SEL-007-POS | 同档无饥饿 | 正向 | P1 |

## 关键用例详述

### TC-UNI-RTY-001-BND:状态码判定矩阵 ★

```yaml
背景: >
  默认可重试区间是「1xx / 3xx / 4xx(除 400、408)/ 5xx(除 504、524)」,2xx 不重试。
  504 与 524 走独立的 alwaysSkipRetryStatusCodes,优先级高于区间配置。
  这四个排除项在审查中已核实,是本项目与 new-api 行为一致性的关键点之一。

排除项的理由:
  400: 参数错误,换渠道也是一样的错,重试纯属浪费
  408: 请求超时,通常是请求本身过大,重试会加剧拥塞
  504/524: 网关超时,上游可能已经在处理,重试会产生重复计费的实际请求

matrix: |
  100 ✔ | 200 ✘ | 201 ✘ | 204 ✘ | 301 ✔ | 399 ✔
  400 ✘ | 401 ✔ | 407 ✔ | 408 ✘ | 409 ✔ | 429 ✔ | 499 ✔
  500 ✔ | 503 ✔ | 504 ✘ | 505 ✔ | 523 ✔ | 524 ✘ | 525 ✔ | 599 ✔
  0 ✔   | 99 ✔  | 600 ✔    (超出 100..=599 视为网络异常)
```

### TC-UNI-RTY-002-POS:渠道错误优先于次数判定 ★

```yaml
背景: >
  判定顺序中「是否渠道错误」在第 3 位,「次数是否耗尽」在第 5 位。
  即使 retry_times_left == 0,渠道类错误仍返回 true。
  若把次数判定提前,渠道故障时就不会切换渠道 —— 所有请求都会打在坏渠道上。

input:
  error: { status: 500, code: "channel:invalid_key" }
  ctx:   { retry_times_left: 0 }
expected: true
```

### TC-UNI-RTY-007-NEG:本地错误既不重试也不禁用 ★

```yaml
背景: >
  local_error = true 表示是**我方适配器**解析出错(如上游返回了预期外的结构),
  不是渠道的问题。若误判为渠道错误,会导致:
  1. 无谓重试,把同样的解析错误在每个渠道上重演一遍
  2. 更糟:把一批健康渠道全部自动禁用

expected:
  should_retry:   false
  should_disable: false
```

### TC-UNI-SEL-003-POS:权重平滑 ★

```yaml
背景: >
  加权随机用 (weight + 10) 而非 weight。平滑的意义在于:
  运维新建渠道时 weight 默认为 0,若直接用 weight 做权重,
  这些渠道**永远不会被选中** —— 表现为"新加的渠道没流量",
  且不会有任何报错,极难排查。

input:  tier = [A(weight=5), B(weight=0)]
assert: 1000 次选择中 B 被选中次数 > 0
延伸:  有效权重 15:10,B 的期望占比 40%
```
