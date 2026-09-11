# 09. 管理面契约测试用例

**版本**:v1.0 | **层**:L3(录制基线)
**测试代码**:`test/src/contract_admin_api.rs`
**基准来源**:从 new-api 实例录制的真实响应(`cases/fixtures/baseline/`)

## 定位

**本层直接对应 §1.2 目标 1(契约兼容),是重构项目最该投入的一层。**

判定基准是录制的 new-api 响应,不是设计文档。原因见 `doc/test-design.md` §1:
v1.0 的设计文档带着 4 处契约偏差通过了首轮人工审查,按文档写测试会把偏差固化。

## 基线录制

```bash
NEWAPI_BASE_URL=http://localhost:3000 \
NEWAPI_ADMIN_TOKEN=<access_token> \
bash test/env/record-baseline.sh
```

产出带元信息的 fixture。**契约测试失败时先看 `recorded_at`**:
是我方回归,还是 new-api 上游演进?

## 用例清单

| 编号 | 名称 | 优先级 | 守住的偏差 |
|---|---|---|---|
| TC-CTR-API-001-POS | 字段全 snake_case(5 端点)★★ | P0 | 审查 P0 |
| TC-CTR-API-002-POS | 字段集不缺失 ★ | P0 | — |
| TC-CTR-API-010-POS | 成功包裹形状 | P0 | — |
| TC-CTR-API-011-POS | 业务错误 HTTP 200 ★★ | P0 | 易被"改良"破坏 |
| TC-CTR-API-012-POS | 鉴权走真实状态码(2 组) | P0 | — |
| TC-CTR-API-020-POS | 分页形状与页码基数 ★ | P0 | 审查 P1 |
| TC-CTR-API-021-POS | p=0 与 p=1 同页 | P0 | 审查 P1 |
| TC-CTR-API-030-NEG | 响应无 password | P0 | — |
| TC-CTR-API-031-POS | 令牌 key 已脱敏 | P0 | — |
| TC-CTR-API-032-NEG | 渠道无明文密钥 | P0 | — |
| TC-CTR-API-040-POS | status 开关集合完整 | P0 | — |
| TC-CTR-API-041-POS | pricing 匿名可访问 | P1 | — |
| TC-CTR-API-042-POS | 额度字段为整数 | P1 | — |

## 关键用例详述

### TC-CTR-API-001-POS:字段命名 ★★

```yaml
背景: >
  审查发现的 P0 偏差。v1.0 的 CONTRACTS.md 规定管理面 DTO 用 camelCase,
  并声称"与 new-api 逐字段一致" —— 而 new-api 的 Go struct tag 全量 snake_case:
    display_name / remain_quota / expired_time / model_limits_enabled /
    unlimited_quota / aff_code / last_login_at ...
  按 camelCase 实现,所有存量客户端与前端都会读不到字段(取值为 undefined),
  直接推翻"客户端与前端可灰度替换"的核心目标。

方法: >
  递归遍历响应的所有字段名,任一段含大写字母即失败。
  同时与基线做字段集比对:允许新增,不允许改名或缺失。

覆盖端点:
  - GET /api/user/self
  - GET /api/token/
  - GET /api/channel/
  - GET /api/log/self
  - GET /api/status
```

### TC-CTR-API-011-POS:业务错误 HTTP 200 ★★

```yaml
背景: >
  管理面用 `{success, message, data}` 表达业务结果。业务失败返回 **HTTP 200**
  + `success:false`,只有鉴权(401/403)与限流(429)用真实状态码。

  这条极易被"改成 RESTful 更规范"而破坏。破坏的后果:
  - 前端 axios 拦截器把它当网络错误,弹"请求失败"而丢失真实 message
  - 存量客户端的 try/catch 分支全部走错

assert:
  response.status: 200
  body.success: false
  body.message: "非空"
```

### TC-CTR-API-031-POS:令牌脱敏

```yaml
背景: >
  列表接口返回的 key 必须已脱敏。完整 key 只在
  `POST /api/token/:id/key`(带 CriticalRateLimit + DisableCache)返回。
  若列表泄漏完整 key,任何能看到列表的人都能盗用。

assert:
  - "items[].key 含 '*'"
  - "长度 <= 18"
```

## 待录制的 fixture 清单

| fixture 名 | 端点 | 用途 |
|---|---|---|
| `api_user_self` | GET /api/user/self | 字段命名、额度类型、无 password |
| `api_token_list` | GET /api/token/?p=1&page_size=10 | 分页形状、脱敏 |
| `api_channel_list` | GET /api/channel/ | 字段命名、无明文密钥 |
| `api_log_self` | GET /api/log/self | 字段命名 |
| `api_status` | GET /api/status | 开关集合 |
| `api_pricing_anonymous` | GET /api/pricing(无鉴权) | 匿名可访问 |
| `api_error_business` | 触发业务错误的请求 | HTTP 200 + success:false |
| `api_error_unauthorized` | 无鉴权访问受保护端点 | 401 |
| `api_error_forbidden` | 低权限访问高权限端点 | 403 |
