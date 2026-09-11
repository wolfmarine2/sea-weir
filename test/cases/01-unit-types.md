# 01. 基础类型与常量单元测试用例

**版本**:v1.0 | **层**:L1 | **测试代码**:`test/src/unit_constants.rs`、`unit_mask.rs`、`unit_pagination.rs`、`unit_error_exit.rs`
**基准来源**:TEST-VECTORS.md §2/§3/§6;溯源见 `adr-review-report.md`「源码复核明细」

## 定位

这组用例的作用是**锁死契约取值**。它们几乎不会因为业务变更而失败 ——
一旦红了,基本可以断定是有人无意中改动了兼容性常量。

其中三条直接对应**审查阶段发现的契约偏差**,必须永久守住:
- 分页页码 1 起(v1.0 文档误写 0 起)
- 令牌脱敏格式(v1.0 文档误写 `sk-****` 前缀形式)
- 管理面业务错误 HTTP 200(最容易被"改成 RESTful"破坏)

## 用例清单

| 编号 | 名称 | 优先级 | 溯源 |
|---|---|---|---|
| TC-UNI-TYP-001-POS | QuotaPerUnit = 500000 | P0 | `common/constants.go:22` |
| TC-UNI-TYP-002-POS | 角色取值 0/1/10/100 | P0 | `common/constants.go:148-151` |
| TC-UNI-TYP-002-NEG | 非法角色值被拒(9 组) | P1 | 同上 |
| TC-UNI-TYP-003-BND | 角色闸门边界(6 组 × 3 闸门) | P0 | TEST-VECTORS §7.4 |
| TC-UNI-TYP-004-POS | 日志类型 0..=6 | P0 | `model/log.go:46-52` |
| TC-UNI-TYP-005-POS | 令牌 key 48 位 | P1 | `common/utils.go:251` |
| TC-UNI-TYP-006-POS | 默认周期常量 | P1 | 各模块 |
| TC-UNI-TYP-007-POS | 重试区间不含 400/408/504/524 | P0 | `status_code_ranges.go:20-33` |
| TC-UNI-TYP-008-POS | 禁用区间仅 401 | P0 | 同上 |
| TC-UNI-TYP-009-POS | 手动/自动禁用可区分 | P1 | — |
| TC-UNI-TYP-010-BND | 令牌脱敏三段规则(7 组) | P0 | `model/token.go` |
| TC-UNI-TYP-011-NEG | 脱敏不泄漏中间段 | P0 | — |
| TC-UNI-TYP-020-BND | 分页页码 1 起(5 组)★ | P0 | `common/page_info.go:41-71` |
| TC-UNI-TYP-021-POS | page_size 别名 ps/size | P1 | 同上 |
| TC-UNI-TYP-022-POS | 分页响应形状 | P0 | 同上 |
| TC-UNI-TYP-030-POS | 错误分类(12 组) | P1 | ADR-008 |
| TC-UNI-TYP-031-POS | 业务错误 HTTP 200 ★ | P0 | CONTRACTS |
| TC-UNI-TYP-032-POS | 鉴权走真实状态码 | P0 | 同上 |
| TC-UNI-TYP-033-POS | 中继错误体 OpenAI 形状 | P0 | ADR-008 |
| TC-UNI-TYP-034-POS | 中继错误体 Claude 形状 | P0 | ADR-008 |
| TC-UNI-TYP-035-POS | 中继错误体 MJ 形状 + code=30→429 | P0 | ADR-008 |
| TC-UNI-TYP-036-POS | AppError → NewApiError 映射(5 组) | P0 | ADR-008 |
| TC-UNI-TYP-037-NEG | 错误出口不泄漏密钥 | P0 | ADR-008 |
| TC-UNI-TYP-038-POS | panic 体含 request id | P1 | ADR-008 |

## 关键用例详述

### TC-UNI-TYP-020-BND:分页页码基数 ★

```yaml
背景: >
  审查发现 v1.0 的 CONTRACTS.md 写「p(页码,0 起)」,而 new-api 实际是 1 起:
  common/page_info.go 中 GetStartIdx() = (Page-1)*PageSize,
  且当 p < 1 时统一归一为 1。
  若按 0 起实现,所有列表接口的第一页会返回第二页的数据 —— 用户看不到最新记录。

matrix:
  - { p: 缺省, page_size: 10, expect_page: 1, expect_start: 0 }
  - { p: 0,    page_size: 10, expect_page: 1, expect_start: 0 }
  - { p: 1,    page_size: 10, expect_page: 1, expect_start: 0 }
  - { p: 2,    page_size: 10, expect_page: 2, expect_start: 10 }
  - { p: 3,    page_size: 20, expect_page: 3, expect_start: 40 }
  - { p: -5,   page_size: 10, expect_page: 1, expect_start: 0 }

失败含义: "start_idx == p*page_size 说明按 0 起实现了"
```

### TC-UNI-TYP-031-POS:管理面业务错误是 HTTP 200 ★

```yaml
背景: >
  new-api 的管理面用 `{success, message, data}` 表达业务结果,业务错误也返回 200。
  这在 REST 洁癖看来"不规范",但它是契约 —— 前端 axios 拦截器与全部存量客户端
  都按「200 + success 判定」写的。改成 4xx 会让它们全部走错分支:
  前端会把业务错误当成网络错误弹"请求失败",丢失真实的 message。

matrix:
  - { error: Biz,           expect_status: 200 }
  - { error: BadRequest,    expect_status: 200 }
  - { error: QuotaExceeded, expect_status: 200 }
  - { error: Unauthorized,  expect_status: 401 }
  - { error: Forbidden,     expect_status: 403 }
  - { error: RateLimited,   expect_status: 429 }
```

### TC-UNI-TYP-010-BND:令牌脱敏 ★

```yaml
背景: >
  脱敏字符串直接展示在前端令牌列表。审查发现文档曾写成 `sk-****` 前缀形式,
  与实际的三段规则不符。

规则:
  - "len ≤ 4  → 全掩码(长度等于原长)"
  - "len ≤ 8  → 前2 + '****' + 后2"
  - "len > 8  → 前4 + '**********' + 后4"

matrix:
  - { input: "",          expect: "" }
  - { input: "ab",        expect: "**" }
  - { input: "abcd",      expect: "****" }
  - { input: "abcde",     expect: "ab****de" }
  - { input: "abcdefgh",  expect: "ab****gh" }
  - { input: "abcdefghi", expect: "abcd**********fghi" }
  - { input: "<48 位>",   expect: "前4 + 10星 + 后4,总长 18" }
```
