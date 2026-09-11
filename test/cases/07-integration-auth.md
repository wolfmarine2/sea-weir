# 07. 认证与鉴权集成测试用例

**版本**:v1.0 | **层**:L2(需 openGauss + Valkey)
**测试代码**:`test/src/integration_auth.rs`、`integration_cache.rs`
**基准来源**:TEST-VECTORS.md §7;ADR-004、ADR-007

## 定位

安全边界,优先级仅次于账务。本组的核心是 **fail-close** ——
缓存故障时宁可拒绝服务,也不能放行未经校验的请求。

## 用例清单

### sk-token 提取

| 编号 | 名称 | 优先级 |
|---|---|---|
| TC-INT-AUT-001-POS | 五源优先级 ★ | P0 |
| TC-INT-AUT-002-POS | 逐级回落(5 组) | P0 |

### 令牌校验

| 编号 | 名称 | 优先级 |
|---|---|---|
| TC-INT-AUT-010-BND | 状态与过期(4 组) | P0 |
| TC-INT-AUT-011-BND | 额度与 unlimited(4 组) | P0 |
| TC-INT-AUT-012-NEG | IP 白名单 | P0 |
| TC-INT-AUT-013-NEG | 用户被禁用即刻失效 ★ | P0 |
| TC-INT-AUT-014-NEG | model_limits 拦截 | P0 |

### 管理面会话

| 编号 | 名称 | 优先级 |
|---|---|---|
| TC-INT-AUT-020-NEG | 缺 New-Api-User → 401 | P0 |
| TC-INT-AUT-021-NEG | New-Api-User 不一致 → 401 ★ | P0 |
| TC-INT-AUT-022-NEG | 会话吊销即刻失效 | P0 |
| TC-INT-AUT-023-POS | access token 回落 | P1 |
| TC-INT-AUT-024-NEG | 缓存不可达 fail-close ★★ | P0 |

### 敏感操作凭证

| 编号 | 名称 | 优先级 |
|---|---|---|
| TC-INT-AUT-030-POS | 凭证一次性消费 ★ | P0 |
| TC-INT-AUT-031-NEG | 凭证过期失效 | P0 |
| TC-INT-AUT-032-NEG | 普通用户不得指定渠道 | P0 |
| TC-INT-AUT-033-POS | admin 可指定渠道 | P1 |

### 缓存与限流(`integration_cache.rs`)

| 编号 | 名称 | 优先级 |
|---|---|---|
| TC-INT-CAC-001-POS | 缓存键约定不可改 ★ | P0 |
| TC-INT-CAC-002-POS | 令牌键用 HMAC ★ | P0 |
| TC-INT-CAC-003-POS | 失效广播多节点一致 | P0 |
| TC-INT-CAC-004-POS | pub/sub 断线重连 | P1 |
| TC-INT-CAC-005-POS | 60s 轮询兜底 | P1 |
| TC-INT-CAC-010-BND | 限流 429 + Retry-After | P1 |
| TC-INT-CAC-011-POS | 窗口滑出恢复 | P1 |
| TC-INT-CAC-012-POS | 按用户隔离 | P1 |
| TC-INT-CAC-013-POS | Valkey 故障降级内存 | P1 |

## 关键用例详述

### TC-INT-AUT-024-NEG:缓存不可达时 fail-close ★★

```yaml
背景: >
  ADR-007 规定:**鉴权类路径 fail-close,非关键路径 fail-open**。
  若鉴权也 fail-open,Valkey 一挂全站鉴权失效 —— 任何人都能访问任何接口。
  这是最严重的一类可用性/安全权衡错误,且在缓存正常时完全测不出来。

steps:
  1. 建立合法会话,确认请求可通过
  2. 停掉 Valkey 容器
  3. 再次发起管理面请求
expected:
  status: 401     # 不是 200
对照组:
  "非关键路径(定价视图)应 fail-open 回源 DB,返回 200"

单元层辅助断言: |
  assert!(!should_fail_open(CachePathKind::Auth));
  assert!(should_fail_open(CachePathKind::Data));
```

### TC-INT-AUT-021-NEG:New-Api-User 防串号 ★

```yaml
背景: >
  new-api 的防串号设计:受保护端点除了 session cookie,还要求
  `New-Api-User: <user_id>` 头,且必须与会话中的 user id 一致。
  即使攻击者拿到了别人的 session(如 XSS 窃取),头对不上也无法冒用。
  这是一层纵深防御,移植时容易被当成"冗余校验"删掉。

steps:
  1. 用 user_id=1 登录取得 session
  2. 请求携带该 session,但 New-Api-User: 2
expected: 401
```

### TC-INT-CAC-002-POS:令牌缓存键用 HMAC ★

```yaml
背景: >
  令牌缓存的键若用明文 key,任何能读到 Redis 的人(误配置的公网端口、
  运维误操作、备份泄漏)都能直接拿到可用的 API key。
  用 HMAC(key, secret) 做键,泄漏缓存也无法反推出 key。

assert:
  - "hmac(plain) != plain"
  - "缓存键不含明文"
  - "同输入稳定(否则缓存永不命中)"
  - "不同 secret 产生不同摘要"
```

### TC-INT-CAC-001-POS:亲和性缓存前缀不可改 ★

```yaml
背景: >
  亲和性缓存键保留 `new-api:channel_affinity:v1:` 前缀,是为了
  **灰度切流期间与 new-api 共享缓存**。改了前缀,切流时所有粘性路由失效,
  用户会遇到会话中途换渠道(表现为模型"失忆"或风格突变)。

assert: "keys::channel_affinity(x).starts_with('new-api:channel_affinity:v1:')"
```
