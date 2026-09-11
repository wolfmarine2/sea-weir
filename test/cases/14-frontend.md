# 14. 前端测试用例

**版本**:v1.0 | **层**:L1/L3(vitest + msw)
**代码位置**:`code/frontend/src/**/*.test.ts(x)`
**基准来源**:ADR-009;与后端共用录制基线

## 定位

前端测试的重点不是 UI 渲染,而是**三条契约**与**表格地基**。

## 三条契约

| 契约 | 破坏后果 | 测试位置 |
|---|---|---|
| 字段 snake_case | 读到 undefined,页面空白 | `types/` 类型断言 + msw fixture |
| 分页页码 1 起 | 第一页显示第二页数据 | `hooks/useTableData.test.ts` |
| 业务错误 HTTP 200 | 把业务错误当网络错误,丢失 message | `api/client.test.ts` |

## 用例清单

### API 层(`api/client.test.ts`)

| 编号 | 名称 | 优先级 |
|---|---|---|
| TC-UNI-FE-001-POS | 注入 New-Api-User 头 | P0 |
| TC-UNI-FE-002-POS | success:false 转 reject ★ | P0 |
| TC-UNI-FE-003-POS | HTTP 401 清态跳登录 | P0 |
| TC-UNI-FE-004-POS | GET 去重(并发同 URL 复用 promise) | P1 |
| TC-UNI-FE-005-NEG | 网络错误与业务错误可区分 | P0 |

### 表格地基(`hooks/useTableData.test.ts`)

| 编号 | 名称 | 优先级 |
|---|---|---|
| TC-UNI-FE-010-POS | 初始请求 p=1 ★ | P0 |
| TC-UNI-FE-011-POS | 修改筛选后重置回第 1 页 ★ | P0 |
| TC-UNI-FE-012-POS | 并发请求竞态处理 | P0 |
| TC-UNI-FE-013-POS | 失败时保留上次数据 | P1 |

### 状态层(`stores/*.test.ts`)

| 编号 | 名称 | 优先级 |
|---|---|---|
| TC-UNI-FE-020-POS | logout 清空并移除 persist | P0 |
| TC-UNI-FE-021-POS | persist 键数 ≤ 5 ★ | P1 |
| TC-UNI-FE-022-NEG | statusStore 不写 localStorage ★ | P1 |

### 工具(`utils/quota.test.ts`)

| 编号 | 名称 | 优先级 |
|---|---|---|
| TC-UNI-FE-030-POS | 500000 → "$1.00" | P1 |
| TC-UNI-FE-031-BND | 0 / 负数 / 大额 | P1 |
| TC-UNI-FE-032-POS | 取整方向与后端一致 ★ | P1 |

## 关键用例详述

### TC-UNI-FE-002-POS:success:false 必须转 reject ★

```yaml
背景: >
  管理面业务错误是 HTTP 200,axios 默认认为成功。
  若拦截器不把 success:false 转成 reject,调用方的 .then() 分支会拿到
  一个 data 为 undefined 的"成功"响应 —— 页面表现为空白或 crash,
  而真实的错误 message 被丢掉了。

steps:
  1. msw 返回 200 + {success:false, message:"余额不足"}
  2. 调用 api.user.self()
expected:
  - "promise 被 reject"
  - "错误对象携带 message == '余额不足'"
```

### TC-UNI-FE-011-POS:筛选后重置页码 ★

```yaml
背景: >
  常见 bug:用户在第 5 页改了筛选条件,请求仍带 p=5,
  但新条件下只有 2 页数据 → 返回空列表。用户以为"搜不到"。

steps:
  1. 翻到第 5 页
  2. 修改筛选条件
expected: "新请求的 p == 1"
```

### TC-UNI-FE-021/022:localStorage 收敛 ★

```yaml
背景: >
  ADR-009 的决策之一:localStorage 从 new-api 的 20+ 键收敛到 ≤5,
  且服务端配置一律走 statusStore(不持久化)。
  写成断言是为了防止后来者"顺手加个 persist" —— 那会让配置变更后
  前端一直拿旧值,且没有任何报错。

assert:
  - "persist 的键总数 <= 5"
  - "statusStore 相关的键不在 localStorage 中"
```

## 契约测试

前端与后端**共用录制基线**:用 `test/cases/fixtures/baseline/*.json` 的
response.body 作为 msw 的返回值。这样前端类型定义与真实契约的偏差会在
测试阶段暴露,而不是联调时。
