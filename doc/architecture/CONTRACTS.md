# 接口契约文档(Calling Contracts)

**文档版本**:v1.1
**最后更新**:2026-09-10
**适用范围**:sea-weir(React SPA + Rust axum 后端)
**相关文档**:`../system-design.md` 第 6 章、`adr/ADR-004-dual-api-surface-and-auth.md`、`adr/ADR-008-error-classification-and-exit.md`、`sequence-diagram/`

---

## 概述

本文档定义 sea-weir 的四类契约:

1. **管理面 REST 契约**:浏览器(前端 api 层)→ api_server 的 `/api/*` 端点
2. **中继面协议契约**:AI 客户端 → api_server 的 `/v1`、`/v1beta`、`/mj`、`/suno`、`/pg` 等端点(OpenAI/Claude/Gemini 兼容)
3. **Adaptor Trait 契约**:relay_engine → sea-weir-adaptors 的渠道适配接口
4. **Repository Trait 契约**:core → sea-weir-repository 的数据访问接口

每条契约包含 **前置条件(Preconditions)**、**后置条件(Postconditions)**、**错误传播(Error Propagation)**。所有契约以 new-api 现行行为为兼容基线。

### 统一响应包裹

| 面 | 场景 | HTTP 状态 | 响应体 |
|---|---|---|---|
| 管理面 | 成功 | 200 | `{"success":true,"message":"","data":<payload>}` |
| 管理面 | 业务错误 | 200 | `{"success":false,"message":"<错误信息>"}` |
| 管理面 | **未认证**(无 session / 无 access token) | 401 | `{"success":false,"message":"…"}` |
| 管理面 | **角色不足**(AdminAuth / RootAuth 未通过)★ | **200** | `{"success":false,"message":"Unauthorized, insufficient privileges"}` |
| 管理面 | 限流 | 429 | `{"success":false,"message":"请求过于频繁"}` + `Retry-After` 头 |
| 中继面 | 成功 | 200(流式 SSE / WS) | 上游原生格式(OpenAI/Claude/Gemini) |
| 中继面 | 错误 | 4xx/5xx | OpenAI 格式 `{"error":{"message","type","code"}}`;Claude 路径 `{"type":"error","error":{…}}`;MJ 路径 `{"code","description","result"}` |
| 任意 | panic 兜底 | 500 | `{"error":{"type":"new_api_panic","message":"…(request id: xxx)"}}` |

### 通用约定

- 管理面 JSON 字段一律为 **snake_case**(如 `display_name`、`remain_quota`、`expired_time`、`model_limits_enabled`),与 new-api 现契约逐字段一致 —— serde 全局 `rename_all = "snake_case"`,禁止使用 camelCase;中继面为各上游原生协议字段
- 管理面受保护端点:签名会话 Cookie(HttpOnly、SameSite=Strict)+ 必须携带 `New-Api-User: <user_id>` 头(与会话用户一致,防串号);无会话时接受 `Authorization: Bearer <access_token>`(系统访问令牌,非 sk-)——
  **注意 `New-Api-User` 在两条认证路径下都必需**,仅带 access token 会 401(实测确认)
- 中继面 sk-token 提取顺序:`Authorization: Bearer sk-…` → `x-api-key`(Claude 路径)→ `?key=` / `x-goog-api-key`(Gemini 路径)→ `Sec-WebSocket-Protocol`(realtime)→ `mj-api-secret`(MJ 路径);`sk-xxx-{channelId}` 后缀指定渠道仅 admin/root 可用
- 角色语义:`guest=0 < common=1 < admin=10 < root=100`;UserAuth≥1、AdminAuth≥10、RootAuth≥100。
  **★ 角色闸门不通过返回 HTTP 200 + `success:false`,不是 403**(溯源:`middleware/auth.go`
  的 `authHelper`,`role < minRole` 分支走 `c.JSON(http.StatusOK, …)`;2026-09-10 录制实测确认)。
  403 只出现在**中继面**(如「无权访问 X 分组」`abortWithOpenAiMessage(c, 403, …)`)。
  若管理面按 403 实现,前端与存量客户端的错误分支会全部走错。
- 管理面受保护端点响应带 `Auth-Version` 头(值为固定哈希),用于阻断不同 new-api 版本间的数据串用
- 列表端点分页约定:`p`(页码,**1 起**;new-api `common/page_info.go` 中 `start_idx=(page-1)*page_size`,且 `p<1` 一律归一为 1)+ `page_size`(兼容别名 `ps`、`size`);响应恒为 `{items, total, page, page_size}` 包在 `data` 内
- 额度单位:500000 quota = $1;接口中 `quota` 为整数额度,`money/amount` 为货币

---

## 1. 认证契约(AUTH)

**模块**:`middleware/auth.rs`、`handlers/user.rs` + `handlers/twofa.rs` + `handlers/passkey.rs`

| 端点 | 前置条件 | 后置条件 | 错误传播 |
|---|---|---|---|
| `POST /api/user/login` | 用户名+密码(bcrypt 校验);开启 2FA 时先走 `login/2fa`;Turnstile 开启时需过人机校验 | 写签名会话 Cookie(30 天);返回 user 对象(不含 password) | 密码错 → `success:false`;2FA 未完成 → 返回需二次验证标记 |
| `POST /api/user/login/2fa` | 已通过账密校验(待验证态) | TOTP/备用码校验通过 → 正式建立会话 | 连续失败按 two_fas.failed_attempts 锁定至 locked_until |
| `POST /api/user/register` | 注册开关打开;邮箱验证码有效(若开启);邀请码可选 | 创建用户(发放注册/邀请赠送额度并记系统日志);自动登录 | 验证码错/用户名冲突 → 业务错误 |
| `GET /api/user/logout` | 已登录 | 会话 jti 写入 Valkey 吊销列表(TTL=剩余有效期);清 Cookie | 幂等,未登录也成功 |
| `GET /api/verification` | 邮箱格式合法 | 验证码写入 Valkey(TTL 10 分钟)+ 发送邮件;受邮箱验证限流 | 限流 → 429 |
| `GET /api/reset_password` + `POST /api/user/reset` | 邮箱已绑定 | 验证码一次性消费后重置密码 | 验证码错/过期 → 业务错误 |
| `GET /api/user/self/token` → access token | 已登录 | 返回/生成 32 位系统访问令牌(`users.access_token`,唯一索引) | — |
| `POST /api/verify`(统一安全验证) | UserAuth;密码/TOTP/Passkey 之一通过 | 换发短时签名「敏感操作凭证」(默认 10 分钟,Valkey 一次性消费);查看渠道密钥等敏感端点强制要求 | 验证失败 → 401 |
| OAuth:`GET /api/oauth/{github,discord,oidc,linuxdo,:custom}` 等 | 对应 OAuth 开关打开;`GET /api/oauth/state` 先取防 CSRF state(可带 `?aff=` 邀请码) | 回调后绑定或注册登录;绑定关系落 `user_oauth_bindings` / 用户表 id 列 | state 错 → 拒绝;绑定冲突 → 业务错误 |

### 会话中间件契约(AUTH-MW,管理面)

- **前置**:会话 Cookie 签名有效且未过期,jti 不在 Valkey 吊销列表;`New-Api-User` 头 = 会话 user id;用户 `status=1`
- **后置**:请求扩展注入 `{user_id, username, role, group}`;响应无额外头
- **错误**:缺失/非法/吊销 → 401;角色不足 → 403;用户被禁用 → 401("用户已被封禁")

### 令牌中间件契约(TOKEN-MW,中继面)

- **前置**:sk-token 存在且 `status=1`、未过期(`expired_time=-1` 或 > now)、非无限额度时 `remain_quota>0`;`allow_ips` 非空时客户端 IP 命中 CIDR;归属用户未禁用;令牌分组(非空覆盖用户分组)在用户可用分组内
- **后置**:注入 `{token_id, user_id, group, model_limits, remain_quota, specific_channel_id?}`;Valkey 缓存键为 `token:{HMAC(key)}`(明文不落缓存)
- **错误**:全部映射为中继面 OpenAI 格式 401/403

---

## 2. 用户域契约(USR)

**模块**:`handlers/user.rs`,UserAuth(除标注外)

| 端点 | 前置条件 | 后置条件 | 错误传播 |
|---|---|---|---|
| `GET /api/user/self` | — | 返回本人信息(脱敏:无 password) | — |
| `PUT /api/user/self` / `PUT /api/user/setting` | 字段合法 | 更新显示名/通知/语言/计费偏好(`setting` JSONB) | — |
| `DELETE /api/user/self` | 密码确认 | 软删除本人 + 吊销全部会话与令牌 | — |
| `GET /api/user/self/groups` | — | 本人可用分组列表(用户分组 ∪ 公开分组) | — |
| `GET /api/user/models` | — | 可用模型列表 = 分组可用模型 ∩ 定价已配置 | — |
| `GET /api/user/aff` / `POST /api/user/aff_transfer` | 邀请码存在 | 邀请统计 / 邀请额度转入钱包(事务 + 行锁) | 余额不足 → 业务错误 |
| `GET /api/user/checkin` / `POST /api/user/checkin` | 签到开关打开 | 每日签到发放额度;`checkins(user_id, checkin_date)` 唯一约束防重 | 重复签到 → 业务错误 |
| `GET /api/user/2fa/status`、`POST /api/user/2fa/{setup,enable,disable,backup_codes}` | — | TOTP 密钥不出 API(setup 返回一次性 otpauth URL);备用码哈希存储 | — |
| `POST /api/user/passkey/{register,verify}/begin|finish` | WebAuthn RP 配置就绪 | 凭证落 `passkey_credentials`(credential_id 唯一) | — |
| 管理员(AdminAuth):`GET /api/user/`、`GET /api/user/search`、`GET /api/user/:id`、`POST /api/user/`、`PUT /api/user/`、`DELETE /api/user/:id`、`POST /api/user/manage`(改角色/分组/额度) | 操作者角色 ≥ admin;root 用户仅 root 可操作 | 用户 CRUD;额度调整记管理日志 | 越权操作 root → 403 |

---

## 3. 令牌域契约(TKN)

**模块**:`handlers/token.rs`,UserAuth

| 端点 | 前置条件 | 后置条件 | 错误传播 |
|---|---|---|---|
| `GET /api/token/`、`GET /api/token/search` | — | 本人令牌分页;key 按 new-api `MaskTokenKey` 脱敏:`前4 + '**********' + 后4`(长度 ≤4 全掩码,≤8 为 `前2+'****'+后2`) | SearchRateLimit |
| `POST /api/token/` | 名称唯一(本人范围);分组在可用分组内 | 生成 48 位随机 key 落库(不含 `sk-` 前缀);expired_time=-1 默认 | — |
| `PUT /api/token/` | 本人令牌 | 全量更新(模型限制/IP 白名单/分组/额度) | — |
| `DELETE /api/token/:id`、`POST /api/token/batch` | 本人令牌 | 软删除 / 批量操作 | — |
| `POST /api/token/:id/key`、`POST /api/token/batch/keys` | 本人令牌 | 返回完整 key(仅此处明文返回) | — |
| `GET /api/usage/token/`(TokenAuthReadOnly) | sk-token 存在且用户未封禁 | 令牌自查:额度/已用/过期/模型限制;CORS 开放 | 不校验令牌状态/额度(只读语义) |
| `GET /api/log/token`(TokenAuthReadOnly) | 同上 | 该令牌的消费日志(只读) | — |

---

## 4. 渠道域契约(CHN)

**模块**:`handlers/channel.rs`,AdminAuth(标注除外)

| 端点 | 前置条件 | 后置条件 | 错误传播 |
|---|---|---|---|
| `GET /api/channel/`、`/search`、`GET /:id` | — | 渠道分页(key 脱敏);含 multi-key 状态聚合 | — |
| `POST /api/channel/`、`POST /batch`、`POST /copy/:id` | type 合法;models/group 非空 | 创建渠道并同步 abilities 三元组(事务) | abilities 同步失败 → 整体回滚 |
| `PUT /api/channel/`、`PUT /tag` | 渠道存在 | 更新并重建 abilities(事务内先删后插);缓存失效广播 | 同上 |
| `DELETE /api/channel/:id`、`DELETE /disabled` | — | 删除渠道 + abilities + 缓存摘除 | — |
| `POST /:id/key`(RootAuth + 敏感操作凭证 + CriticalRateLimit) | 渠道存在;凭证有效且一次性消费 | 返回渠道完整密钥(审计日志记录) | 无凭证 → 401 |
| `GET /test`、`GET /test/:id` | — | 实测渠道(发测试模型请求);通过且开启自动恢复时启用渠道;记录 response_time | 测试失败 → 返回原因 |
| `GET /update_balance`、`/update_balance/:id` | 渠道支持余额探测 | 更新 balance(USD) | 不支持的渠道类型 → 业务错误 |
| `GET /fetch_models/:id`、`POST /fetch_models`(RootAuth) | 渠道可用 | 回源拉取上游模型列表 | — |
| `POST /multi_key/manage` | 渠道存在 | 多 key 的启停/模式(random/polling)管理;逐 key 状态入 `channel_info` JSONB | — |
| `POST /codex/oauth/{start,complete}`、`POST /:id/codex/refresh` | Codex 渠道 | OAuth 凭证获取与自动刷新 | — |
| `POST /ollama/pull(/stream)`、`DELETE /ollama/delete`、`GET /ollama/version/:id` | Ollama 渠道 | 模型拉取(流式进度)/删除/版本 | — |
| `POST /upstream_updates/{apply,apply_all,detect,detect_all}` | — | 上游模型变更检测与应用 | — |

---

## 5. 日志域契约(LOG)

**模块**:`handlers/log.rs`

| 端点 | 鉴权 | 前置条件 | 后置条件 |
|---|---|---|---|
| `GET /api/log/`、`/search`、`GET /stat`、`DELETE /` | AdminAuth | 时间范围/类型/模型/用户/渠道过滤 | 分页日志;stat 返回总额度 + 最近 60s rpm/tpm;Delete 清理历史 |
| `GET /api/log/self`、`/self/search`、`/self/stat` | UserAuth | 仅本人 | 同上(本人范围) |
| `GET /api/log/self/files` | UserAuth | — | 本人日志归档文件列表(对象存储) |
| `GET /api/log/channel_affinity_usage_cache` | AdminAuth | — | 渠道亲和性缓存命中率统计 |

---

## 6. 计费与定价域契约(BILL)

**模块**:`handlers/pricing.rs`、`handlers/billing.rs`、`handlers/ratio_config.rs`

| 端点 | 鉴权 | 前置/后置 | 错误传播 |
|---|---|---|---|
| `GET /api/pricing` | TryUserAuth(可匿名) | — | 定价视图(abilities × models × vendors × 倍率合成,进程内 1 分钟缓存) |
| `GET /api/ratio_config` | CriticalRateLimit | — | 当前全部倍率配置快照(模型/分组/缓存/图片/音频) |
| `GET /dashboard/billing/subscription`、`GET /dashboard/billing/usage`(含 `/v1` 前缀变体) | TokenAuth | sk-token 有效 | OpenAI dashboard 兼容:总额度/区间用量 |
| 阶梯计费/倍率修改 | 经 `PUT /api/option`(RootAuth,见 OPT 域) | 修改后进程内缓存即时失效 + Valkey 广播 | — |

计费语义(中继面,详见 SEQ-003)。以下与 new-api `service/text_quota.go`
`calculateTextQuotaSummary` 逐步骤对齐,黄金用例见 `TEST-VECTORS.md` §1:

**按量计费**

```text
# 1. 基数扣减 —— 取决于 usage 语义
#    OpenAI 语义:prompt_tokens 已含 cache/cache_creation,须扣除后单独计价
#    Claude 语义:Anthropic 分开上报,prompt_tokens 不含,故不扣除
base = prompt
     − cache            (仅 OpenAI 语义)
     − cache_creation   (仅 OpenAI 语义)
     − image
     − audio            (仅当该模型配置了 audio_input_price > 0)

# 2. 分项计价
cache_q   = cache × CacheRatio
image_q   = image × ImageRatio
create_q  = OpenAI 语义: cache_creation × CacheCreationRatio
            Claude 语义: (cache_creation − cc_5m − cc_1h) × CacheCreationRatio
                       + cc_5m × CacheCreation5mRatio
                       + cc_1h × CacheCreation1hRatio
prompt_q  = base + cache_q + image_q + create_q
compl_q   = completion × CompletionRatio

# 3. 主体 × 倍率,再加外挂项(注意:audio 与工具附加费在倍率之外)
quota = (prompt_q + compl_q) × (ModelRatio × GroupRatio)
      + tool_surcharge
      + audio_q            # audio_price ÷ 1e6 × audio × GroupRatio × QuotaPerUnit

# 4. 其他倍率连乘(任务类的时长/分辨率修正)
for r in OtherRatios: quota ×= r

# 5. 下限与取整
if (ModelRatio × GroupRatio) ≠ 0 and quota ≤ 0: quota = 1
quota = round(quota)                      # 见下方「取整」
if total_tokens == 0: quota = 0
elif (ModelRatio × GroupRatio) ≠ 0 and quota == 0: quota = 1
```

**按次计费**

```text
quota = ModelPrice × QuotaPerUnit × GroupRatio + tool_surcharge + audio_q
for r in OtherRatios: quota ×= r
quota = round(quota)
```

**阶梯计费**:`tiered_expr` 表达式按实际 usage 重算。

**取整(关键)**:new-api 用 shopspring/decimal 的 `Round(0)`,即
**half away from zero**(1487.5 → 1488、−0.5 → −1)。`rust_decimal` 的 `.round()`
默认是**银行家舍入**(MidpointNearestEven),两者在 `x.5` 且整数部分为偶数时结果不同
(798.5 → half-away 799 / banker 798)。因此计费落库**必须**显式指定:

```rust
quota.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
```

用 `.round()` 会产生与 new-api 差 1 的账目偏差。此规则由 `TEST-VECTORS.md` TV-BILL-005 守住。

---

## 7. 充值与订阅域契约(PAY/SUB)

**模块**:`handlers/topup*.rs`、`handlers/subscription*.rs`、`handlers/redemption.rs`

| 端点 | 鉴权 | 前置条件 | 后置条件 | 错误传播 |
|---|---|---|---|---|
| `POST /api/user/topup`(兑换码充值) | UserAuth | 兑换码存在、可用、未过期 | **事务 + `FOR UPDATE` 行锁**:置已用 + 用户额度增加 + 记充值日志;并发兑换由行锁 + 唯一状态保证 | 已用/禁用 → 业务错误 |
| `POST /api/user/pay`(易支付)、`/stripe/pay`、`/creem/pay`、`/waffo/pay` | UserAuth | 金额 ≥ MinTopUp;支付方式启用 | 创建 top_ups(pending,`trade_no` 唯一)→ 返回支付参数/跳转地址 | — |
| `POST /api/user/epay/notify`、`POST /api/stripe/webhook`、`/creem/webhook`、`/waffo/webhook` | 无鉴权 | **签名校验**(易支付 MD5 / Stripe sig / Creem / Waffo 各自方案) | 幂等完成:`trade_no` 唯一 + 状态机 pending→success(CAS);用户额度入账 + 按 TopupGroupRatio 赠送;重复回调直接返回成功 | 签名错 → 400 |
| `POST /api/user/topup/complete`(AdminAuth) | 订单 pending | 人工补单(同事务幂等) | — |
| `GET /api/subscription/plans`、`POST /api/subscription/{epay,stripe,creem}/pay` | UserAuth | 套餐启用;购买数 < MaxPurchasePerUser | 创建 subscription_orders → 支付 → 完成回调(事务幂等)→ 生成 user_subscriptions(分组升级、周期重置参数) | — |
| `PUT /api/subscription/self/preference` | UserAuth | — | 更新计费偏好(wallet_only/subscription_only/wallet_first/subscription_first) | — |
| 管理端:`/api/subscription/admin/plans` CRUD、`/bind`、`/users/:id/subscriptions`、`/user_subscriptions/:id/{invalidate,DELETE}` | AdminAuth | — | 套餐与用户订阅管理;失效/删除触发分组回退 | — |

---

## 8. 设置与系统域契约(OPT/SYS)

**模块**:`handlers/option.rs`、`handlers/misc.rs`、`handlers/setup.rs`

| 端点 | 鉴权 | 前置/后置 |
|---|---|---|
| `GET /api/setup` / `POST /api/setup` | 公开 | 未初始化(getups 表无记录)时 PostSetup 创建 root 并写初始化标记;已初始化后拒绝 |
| `GET /api/status` | 公开 | 全站配置(系统名/logo/OAuth 开关/支付开关/导航模块可见性等),前端 status store 唯一来源 |
| `GET /api/notice`、`/about`、`/user-agreement`、`/privacy-policy`、`/home_page_content` | 公开 | 运营文案 |
| `GET /api/option/`、`PUT /api/option/` | RootAuth | 读全部选项;更新:落 options 表 → 进程内配置即时生效 → Valkey pub/sub 广播多节点失效;`xxx_setting.yyy` 键走分层配置组 |
| `POST /api/option/rest_model_ratio`、`POST /api/migrate_console_setting` | RootAuth | 倍率重置 / 控制台设置迁移 |
| `GET/DELETE /api/option/channel_affinity_cache` | RootAuth | 亲和性缓存查看/清空 |
| `GET /api/uptime/status` | 公开 | Uptime Kuma 监控状态 |
| `GET /api/performance/stats`、`/logs`、`POST /reset_stats`、`/gc`、`DELETE /disk_cache`、`/logs` | RootAuth | 运行统计与运维操作 |

---

## 9. 模型/分组/任务/看板域契约

| 端点 | 鉴权 | 说明 |
|---|---|---|
| `GET /api/models`(管理面 DashboardListModels) | UserAuth | 全量可用模型(看板) |
| `/api/models` CRUD + `/sync_upstream*` + `/missing`(AdminAuth) | AdminAuth | 模型元数据 CRUD、上游同步、缺失模型(abilities 有而 models 无)检测 |
| `/api/vendors` CRUD | AdminAuth | 厂商元数据 |
| `GET /api/group/`、`/api/prefill_group` CRUD | AdminAuth | 分组列表、预填分组(model/tag/endpoint 三类) |
| `GET /api/mj/self`、`GET /api/mj/`;`GET /api/task/self`、`GET /api/task/` | UserAuth / AdminAuth | Midjourney 与异步任务记录查询 |
| `GET /api/data/`、`/users`(AdminAuth)、`/self`(UserAuth) | — | 额度消耗看板聚合数据(quota_data 按小时) |
| `/api/deployments/**` | AdminAuth | io.net 集群部署管理(设置/CRUD/日志/容器/估价) |
| `/api/custom-oauth-provider/**` | RootAuth | 自定义 OAuth Provider CRUD + OIDC discovery |
| `/api/ratio_sync/{channels,fetch}` | RootAuth | 上游倍率拉取同步 |

---

## 10. 中继面协议契约(RELAY)

**模块**:`middleware/auth.rs`(TokenAuth)→ `middleware/distribute.rs` → `handlers/relay.rs` → relay_engine。中间件链:CORS → 解压 → 统计 → 性能护栏 → TokenAuth → 模型请求限流 → Distribute。

| 端点 | 格式 | 前置条件 | 后置条件 |
|---|---|---|---|
| `POST /v1/chat/completions`、`/v1/completions`、`/v1/moderations` | OpenAI | TOKEN-MW 通过;模型在令牌白名单内 | 流式 SSE / 非流式;usage 注入规则见 SEQ-004 |
| `POST /v1/responses`、`/v1/responses/compact` | OpenAI Responses | 同上 | 同上 |
| `POST /v1/messages` | Claude | `x-api-key` 兼容;`-thinking` / effort 后缀解析 | Claude 原生格式出口 |
| `POST /v1beta/models/{model}:{action}`、`POST /v1/models/*path` | Gemini | `?key=` / `x-goog-api-key` 兼容;action ∈ generateContent/streamGenerateContent/embedContent/batchEmbedContents | Gemini 原生格式出口 |
| `POST /v1/images/{generations,edits}`、`/v1/edits` | OpenAI Image | multipart 支持 | 按次计费 |
| `POST /v1/audio/{speech,transcriptions,translations}` | OpenAI Audio | TTS/STT;STT 按音频时长预估 | — |
| `POST /v1/embeddings`、`/v1/engines/:model/embeddings`、`/v1/rerank` | OpenAI / Cohere 兼容 | `engines/:model` 变体将 path 中的 model 覆盖进 body | — |
| `POST /v1/edits`、`/v1/models/*path` | OpenAI | 泛路径按请求头分派到对应 relay 格式 | — |
| `GET /v1/realtime` | OpenAI Realtime(WS) | WS 子协议提取 key | 双向转发;`response.done` 事件增量扣费 |
| `GET /v1/models`、`GET /v1/models/:model`;`GET /v1beta/models`、`/v1beta/openai/models` | 按请求头分派 | TokenAuth(不选渠道) | 令牌可用模型列表(∩ 分组可用 ∩ 已配置定价) |
| `POST /mj/submit/{imagine,change,simple-change,blend,describe,action,modal,shorten,edits,video,upload-discord-images}`、`GET /mj/task/:id/fetch`、`GET /mj/task/:id/image-seed`、`POST /mj/task/list-by-condition`、`POST /mj/insight-face/swap`、`GET /mj/image/:id`(公开图片代理,注册于 TokenAuth 之前) | Midjourney | 变换类锁定原任务渠道;整组同时注册于 `/mj/**` 与 `/:mode/mj/**` 两个前缀 | MJ 错误格式 `{"code","description","result"}`(code=30 → 429)。注:new-api 的 `/mj/notify` 已在源码中注释停用,sea-weir 不实现 |
| `POST /suno/submit/:action`、`POST /suno/fetch`、`GET /suno/fetch/:id` | Suno | — | 异步任务模式(SEQ-006) |
| `POST /v1/video/generations`、`POST /v1/videos`、`GET /v1/videos/:task_id`、`POST /v1/videos/:video_id/remix`、`/kling/v1/videos/**`、`/jimeng` | 视频任务 | remix 锁原渠道 | 异步任务模式;GET 查询不选渠道 |
| `GET /v1/videos/:task_id/content` | TokenOrUserAuth | session 或 sk-token | 视频内容代理(SSRF 校验) |
| `POST /pg/chat/completions` | OpenAI | **UserAuth**(非 sk-token);可指定分组 | 同 chat/completions 管线 |
| `POST /v1/images/variations`;`/v1/files`(GET、POST、GET `/:id`、GET `/:id/content`、DELETE `/:id`);`/v1/fine-tunes`(POST、GET、GET `/:id`、POST `/:id/cancel`、GET `/:id/events`);`DELETE /v1/models/:model` | OpenAI | — | **占位端点**:路由保留,统一返回 new-api `RelayNotImplemented` 的同构错误体;不实现业务语义(共 11 条,清单见附录 A.2.7) |

**渠道上下文注入约定**(Distribute 后置):`{channel_id, channel_type, base_url, key(含 multi-key 选中下标), model_mapping, param_override, header_override, status_code_mapping, auto_ban, setting}`;请求成功(状态 <400)后回写亲和性缓存。

**重试契约**:最大尝试 = `RetryTimes + 1`;retry 档位 = 优先级降序第 N 档;`specific_channel_id` / 亲和性 SkipRetryOnFailure / `IsAlwaysSkipRetryCode` / 400·408·504·524 不重试。

---

## 11. Adaptor Trait 契约

**模块**:`sea-weir-adaptors/src/lib.rs`;注册表按 `ApiType`(35 个常量)与任务平台(10 个)索引。

```rust
#[async_trait]
pub trait Adaptor: Send + Sync {
    fn init(&self, info: &RelayInfo) -> Result<(), AppError>;
    fn get_request_url(&self, info: &RelayInfo) -> Result<String, AppError>;
    fn setup_request_header(&self, headers: &mut HeaderMap, info: &RelayInfo) -> Result<(), AppError>;
    fn convert_request(&self, req: RelayRequest, info: &RelayInfo) -> Result<Bytes, AppError>;
    async fn do_request(&self, info: &RelayInfo, body: Bytes) -> Result<UpstreamResponse, AppError>;
    /// 流式:驱动 stream_pipe;非流式:解析 usage;错误:Err(NewApiError)
    async fn do_response(&self, resp: UpstreamResponse, info: &mut RelayInfo) -> Result<Usage, NewApiError>;
    fn get_model_list(&self) -> &[String];
    fn channel_name(&self) -> &'static str;
}

#[async_trait]
pub trait TaskAdaptor: Send + Sync {
    fn validate_request_and_set_action(&self, req: &mut TaskRequest) -> Result<(), AppError>;
    fn estimate_billing(&self, req: &TaskRequest) -> Vec<OtherRatio>;       // 时长/分辨率修正
    fn adjust_billing_on_submit(&self, ...) -> i64;                         // 提交后额度修正
    fn adjust_billing_on_complete(&self, task: &Task) -> Option<i64>;       // 完成补差(None=按 tokens 重算)
    fn build_request(&self, info: &RelayInfo) -> Result<(String, HeaderMap, Bytes), AppError>;
    async fn fetch_task(&self, task_id: &str, channel: &Channel) -> Result<TaskResult, AppError>;
    fn parse_task_result(&self, raw: Value) -> Result<TaskResult, AppError>;
}
```

- **前置**:`info` 已完成渠道上下文注入与模型映射;**后置**:`do_response` 返回的 `Usage` 进入结算;任何 Err 进入重试/禁用判定
- **错误**:适配器内部错误标 `LocalError=true`(不触发渠道禁用/重试);上游错误经 `RelayErrorHandler` 归一化

## 12. Repository Trait 契约(摘录核心)

```rust
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: i64) -> Result<Option<User>, AppError>;
    /// 原子扣减:UPDATE users SET quota = quota - $1 WHERE id=$2 AND quota >= $1;0 行 = 余额不足
    async fn try_decrease_quota(&self, id: i64, amount: i64) -> Result<bool, AppError>;
    async fn increase_quota(&self, id: i64, amount: i64) -> Result<(), AppError>;
}

#[async_trait]
pub trait TokenRepository: Send + Sync {
    async fn find_by_key(&self, key_hmac: &str) -> Result<Option<Token>, AppError>;  // Valkey 读穿
    async fn try_decrease_quota(&self, id: i64, amount: i64) -> Result<bool, AppError>;
}

#[async_trait]
pub trait ChannelRepository: Send + Sync {
    async fn list_enabled(&self) -> Result<Vec<Channel>, AppError>;          // 渠道缓存全量重建用
    async fn update_status(&self, id: i64, status: i32, reason: &str) -> Result<(), AppError>;
    async fn sync_abilities(&self, channel: &Channel) -> Result<(), AppError>; // 事务:先删后插
}

#[async_trait]
pub trait RedemptionRepository: Send + Sync {
    /// 事务 + FOR UPDATE;返回 Err 表示已用/无效
    async fn redeem(&self, key: &str, user_id: i64) -> Result<i64, AppError>;
}

#[async_trait]
pub trait SubscriptionRepository: Send + Sync {
    /// 幂等预扣:request_id 唯一;事务内按 end_time 升序 FOR UPDATE 逐个扣
    async fn pre_consume(&self, request_id: &str, user_id: i64, amount: i64) -> Result<i64, AppError>;
    async fn settle(&self, request_id: &str, actual: i64) -> Result<(), AppError>;
    async fn refund(&self, request_id: &str) -> Result<(), AppError>;
}

#[async_trait]
pub trait TaskRepository: Send + Sync {
    /// CAS:UPDATE tasks SET status=$1 WHERE id=$2 AND status=$3
    async fn cas_status(&self, id: i64, from: TaskStatus, to: TaskStatus) -> Result<bool, AppError>;
}
```

## 13. 错误码表(中继面)

| error_code | type | HTTP | 说明 |
|---|---|---|---|
| `invalid_request` | invalid_request_error | 400 | 请求解析/校验失败(含 body 超限 413) |
| `authentication_error` | authentication_error | 401 | 令牌缺失/非法/吊销 |
| `permission_error` | permission_error | 403 | 模型限制/IP 白名单/分组不可用 |
| `insufficient_quota` | insufficient_quota | 403 | 额度不足(预扣失败) |
| `model_not_found` | invalid_request_error | 404 | 无可用渠道承载该模型 |
| `rate_limit_exceeded` | rate_limit_error | 429 | 模型请求限流 / 全局限流 |
| `channel:invalid_key` 等 `channel:*` | new_api_error | 502/透传 | 渠道类错误(触发重试与自动禁用判定) |
| `upstream_error` | upstream_error | 透传 | 上游业务错误(状态码映射可配) |
| `new_api_panic` | new_api_error | 500 | panic 兜底 |

## 14. 连接与并发契约

- **连接池**:主库 max 100(可配);日志库独立 pool;Valkey 连接池 fred 内置
- **线程安全**:`Arc<AppState>` 共享;所有 Trait `Send + Sync`;计费会话内部 `tokio::sync::Mutex`,settled/refunded 标志保证幂等
- **缓存一致性**:Valkey 读穿 + 失效广播(pub/sub `cache:invalidate:{user|token|channel|option}`);TTL=60s 兜底;以 DB 为准
- **后台任务**:Option 同步(60s 轮询兜底 + pub/sub)、渠道缓存同步、渠道定时测试、任务轮询(15s)、订阅周期重置、崩溃预扣对账(启动时 + 每小时)

## 15. 时序图交叉引用

| 契约 | 相关时序图 |
|---|---|
| AUTH / AUTH-MW | `sequence-diagram/SEQ-001-user-login-and-admin-auth.puml` |
| TOKEN-MW / RELAY 分发 | `sequence-diagram/SEQ-002-token-auth-and-distribution.puml` |
| BILL 计费语义 / RELAY | `sequence-diagram/SEQ-003-text-relay-billing.puml` |
| RELAY 流式 | `sequence-diagram/SEQ-004-sse-stream-relay.puml` |
| RELAY 重试 / CHN 禁用 | `sequence-diagram/SEQ-005-channel-retry-autoban.puml` |
| RELAY 异步任务 | `sequence-diagram/SEQ-006-async-task-polling.puml` |

---

## 附录 A:完整端点清单(305 条,与 new-api `router/` 逐条对齐)

> 本附录是 system-design.md §6.2 的展开,也是契约兼容的验收基线。统计口径:剔除源码中已注释的 6 条(`/api/midjourney`、`/api/waffo-pancake/webhook`、`/api/user/tokenlog`、`/api/user/waffo-pancake/{amount,pay}`、`/mj/notify`),按注册声明处计数。
>
> 鉴权标注:**Pub** 公开 / **UA** UserAuth(≥1) / **AA** AdminAuth(≥10) / **RA** RootAuth(=100) / **TA** TokenAuth(sk-token) / **TRO** TokenAuthReadOnly / **TUA** TokenOrUserAuth / **TryUA** TryUserAuth(可匿名)。限流中间件另注:`CRL`=CriticalRateLimit、`SRL`=SearchRateLimit、`EVRL`=EmailVerificationRateLimit、`TS`=TurnstileCheck、`SVR`=SecureVerificationRequired、`SPC`=SystemPerformanceCheck、`MRRL`=ModelRequestRateLimit、`DIST`=Distribute。
>
> 全局:`/api/*` 挂 gzip + BodyStorageCleanup + GlobalAPIRateLimit。

### A.1 管理面 `/api`(236 条)

**A.1.1 公开与系统域(23 条)**

| 方法 | 路径 | 鉴权 |
|---|---|---|
| GET / POST | `/api/setup` | Pub |
| GET | `/api/status` | Pub |
| GET | `/api/status/test` | AA |
| GET | `/api/uptime/status` | Pub |
| GET | `/api/models` | UA |
| GET | `/api/notice`、`/api/about`、`/api/user-agreement`、`/api/privacy-policy`、`/api/home_page_content` | Pub |
| GET | `/api/pricing` | TryUA |
| GET | `/api/ratio_config` | Pub + CRL |
| GET | `/api/verification` | Pub + EVRL + TS |
| GET | `/api/reset_password` | Pub + CRL + TS |
| POST | `/api/user/reset` | Pub + CRL |
| POST | `/api/verify`(统一安全验证) | UA + CRL |
| POST | `/api/stripe/webhook`、`/api/creem/webhook`、`/api/waffo/webhook` | Pub(签名校验) |

**A.1.2 OAuth(8 条)**

| 方法 | 路径 | 鉴权 |
|---|---|---|
| GET | `/api/oauth/state` | Pub + CRL |
| GET | `/api/oauth/:provider`(github/discord/oidc/linuxdo/自定义) | Pub + CRL |
| POST | `/api/oauth/email/bind` | Pub + CRL |
| GET | `/api/oauth/wechat`;POST `/api/oauth/wechat/bind` | Pub + CRL |
| GET | `/api/oauth/telegram/login`、`/api/oauth/telegram/bind` | Pub + CRL |

**A.1.3 用户域 `/api/user`(57 条)**

| 分组 | 方法 路径 | 鉴权 |
|---|---|---|
| 认证 | POST `/register`(CRL+TS)、POST `/login`(CRL+TS)、POST `/login/2fa`(CRL)、POST `/passkey/login/begin`、POST `/passkey/login/finish`(CRL)、GET `/logout`、GET `/groups` | Pub |
| 支付回调 | POST / GET `/epay/notify` | Pub(签名校验) |
| 个人(UA) | GET `/self`、PUT `/self`、DELETE `/self`、GET `/self/groups`、GET `/models`、GET `/token`、PUT `/setting` | UA |
| Passkey(UA) | GET `/passkey`、DELETE `/passkey`、POST `/passkey/register/begin`、`/passkey/register/finish`、`/passkey/verify/begin`、`/passkey/verify/finish` | UA |
| 2FA(UA) | GET `/2fa/status`、POST `/2fa/setup`、`/2fa/enable`、`/2fa/disable`、`/2fa/backup_codes` | UA |
| 钱包(UA) | GET `/aff`、POST `/aff_transfer`、GET `/topup/info`、GET `/topup/self`、POST `/topup`(CRL)、POST `/amount`、POST `/pay`(CRL) | UA |
| 支付下单(UA) | POST `/stripe/amount`、`/stripe/pay`(CRL)、`/creem/pay`(CRL)、`/waffo/amount`、`/waffo/pay`(CRL) | UA |
| 签到(UA) | GET `/checkin`、POST `/checkin`(TS) | UA |
| OAuth 绑定(UA) | GET `/oauth/bindings`、DELETE `/oauth/bindings/:provider_id` | UA |
| 管理(AA) | GET `/`、GET `/search`、GET `/:id`、POST `/`、PUT `/`、DELETE `/:id`、POST `/manage` | AA |
| 管理-充值(AA) | GET `/topup`、POST `/topup/complete` | AA |
| 管理-凭证(AA) | GET `/:id/oauth/bindings`、DELETE `/:id/oauth/bindings/:provider_id`、DELETE `/:id/bindings/:binding_type`、DELETE `/:id/reset_passkey`、GET `/2fa/stats`、DELETE `/:id/2fa` | AA |

**A.1.4 令牌域 `/api/token`(9 条,UA)**

GET `/`、GET `/search`(SRL)、GET `/:id`、POST `/`、PUT `/`、DELETE `/:id`、POST `/batch`、POST `/:id/key`(CRL+DisableCache)、POST `/batch/keys`(CRL+DisableCache)

**A.1.5 令牌自查 `/api/usage/token/`(1 条,TRO + CORS + CRL)**

**A.1.6 渠道域 `/api/channel`(39 条,AA)**

| 分组 | 路径 |
|---|---|
| 查询 | GET `/`、`/search`、`/:id`、`/models`、`/models_enabled`、`/tag/models` |
| 写入 | POST `/`、PUT `/`、DELETE `/:id`、DELETE `/disabled`、POST `/batch`、POST `/batch/tag`、POST `/copy/:id`、POST `/fix`、PUT `/tag`、POST `/tag/disabled`、POST `/tag/enabled` |
| 密钥 | POST `/:id/key`(**RA + CRL + DisableCache + SVR**) |
| 测试/余额 | GET `/test`、`/test/:id`、`/update_balance`、`/update_balance/:id` |
| 上游模型 | GET `/fetch_models/:id`、POST `/fetch_models`(**RA**) |
| 多 key | POST `/multi_key/manage` |
| Codex | POST `/codex/oauth/start`、`/codex/oauth/complete`、`/:id/codex/oauth/start`、`/:id/codex/oauth/complete`、`/:id/codex/refresh`、GET `/:id/codex/usage` |
| Ollama | POST `/ollama/pull`、`/ollama/pull/stream`、DELETE `/ollama/delete`、GET `/ollama/version/:id` |
| 上游更新 | POST `/upstream_updates/{apply,apply_all,detect,detect_all}` |

**A.1.7 日志域 `/api/log`(10 条)**

GET `/`(AA)、DELETE `/`(AA)、GET `/search`(AA)、GET `/stat`(AA)、GET `/channel_affinity_usage_cache`(AA)、GET `/self`(UA)、GET `/self/search`(UA+SRL)、GET `/self/stat`(UA)、GET `/files`(UA)、GET `/token`(**TRO + CORS + CRL**)

**A.1.8 看板域 `/api/data`(3 条)**:GET `/`(AA)、GET `/users`(AA)、GET `/self`(UA)

**A.1.9 兑换码 `/api/redemption`(7 条,AA)**:GET `/`、`/search`、`/:id`、POST `/`、PUT `/`、DELETE `/:id`、DELETE `/invalid`

**A.1.10 订阅域(16 条)**

| 方法 路径 | 鉴权 |
|---|---|
| GET `/api/subscription/plans`、`/self`、PUT `/self/preference` | UA |
| POST `/api/subscription/{epay,stripe,creem}/pay` | UA + CRL |
| POST / GET `/api/subscription/epay/notify`、GET / POST `/api/subscription/epay/return` | Pub(签名校验) |
| GET / POST `/api/subscription/admin/plans`、PUT / PATCH `/plans/:id`、POST `/bind`、GET / POST `/users/:id/subscriptions`、POST `/user_subscriptions/:id/invalidate`、DELETE `/user_subscriptions/:id` | AA |

**A.1.11 设置域 `/api/option`(6 条,RA)**:GET `/`、PUT `/`、GET / DELETE `/channel_affinity_cache`、POST `/rest_model_ratio`、POST `/migrate_console_setting`

**A.1.12 OAuth Provider `/api/custom-oauth-provider`(6 条,RA)**:GET `/`、GET `/:id`、POST `/`、PUT `/:id`、DELETE `/:id`、POST `/discovery`

**A.1.13 性能运维 `/api/performance`(6 条,RA)**:GET `/stats`、GET `/logs`、DELETE `/logs`、DELETE `/disk_cache`、POST `/reset_stats`、POST `/gc`

**A.1.14 倍率同步 `/api/ratio_sync`(2 条,RA)**:GET `/channels`、POST `/fetch`

**A.1.15 分组(5 条,AA)**:GET `/api/group/`;`/api/prefill_group` GET `/`、POST `/`、PUT `/`、DELETE `/:id`

**A.1.16 模型与厂商(15 条,AA)**

`/api/models`:GET `/`、`/search`、`/:id`、`/missing`、`/sync_upstream/preview`、POST `/`、POST `/sync_upstream`、PUT `/`、DELETE `/:id`
`/api/vendors`:GET `/`、`/search`、`/:id`、POST `/`、PUT `/`、DELETE `/:id`

**A.1.17 部署管理 `/api/deployments`(19 条,AA)**:GET `/settings`、POST `/settings/test-connection`、GET `/`、`/search`、`/hardware-types`、`/locations`、`/available-replicas`、`/check-name`、`/:id`、`/:id/logs`、`/:id/containers`、`/:id/containers/:container_id`、POST `/`、`/test-connection`、`/price-estimation`、`/:id/extend`、PUT `/:id`、`/:id/name`、DELETE `/:id`

**A.1.18 任务记录(4 条)**:GET `/api/mj/self`(UA)、GET `/api/mj/`(AA)、GET `/api/task/self`(UA)、GET `/api/task/`(AA)

### A.2 中继面(54 条)

**A.2.1 模型列表(4 条,TA,不选渠道)**:GET `/v1/models`、GET `/v1/models/:model`、GET `/v1beta/models`、GET `/v1beta/openai/models`

**A.2.2 OpenAI 兼容(TA + SPC + MRRL + DIST)**

POST `/v1/chat/completions`、`/v1/completions`、`/v1/responses`、`/v1/responses/compact`、`/v1/messages`(Claude 原生)、`/v1/edits`、`/v1/images/generations`、`/v1/images/edits`、`/v1/embeddings`、`/v1/engines/:model/embeddings`、`/v1/audio/transcriptions`、`/v1/audio/translations`、`/v1/audio/speech`、`/v1/rerank`、`/v1/moderations`、`/v1/models/*path`;GET `/v1/realtime`(WebSocket)

**A.2.3 Gemini 原生(1 条)**:POST `/v1beta/models/*path`(TA + SPC + MRRL + DIST)

**A.2.4 Playground(1 条)**:POST `/pg/chat/completions`(**UA** + SPC + DIST)

**A.2.5 Midjourney(16 条,TA + DIST;`/mj/image/:id` 在 TokenAuth 之前注册故为 Pub)**

GET `/mj/image/:id`(Pub,图片代理)、POST `/mj/submit/{imagine,change,simple-change,describe,blend,action,modal,shorten,edits,video,upload-discord-images}`、GET `/mj/task/:id/fetch`、GET `/mj/task/:id/image-seed`、POST `/mj/task/list-by-condition`、POST `/mj/insight-face/swap`

> **双前缀注册**:同一组 16 条同时注册在 `/mj/**` 与 `/:mode/mj/**`(`registerMjRouterGroup` 被调用两次),路由表实际展开为 32 条。sea-weir 需保留 `:mode` 变体以兼容存量客户端。

**A.2.6 Suno(3 条,TA + SPC + DIST)**:POST `/suno/submit/:action`、POST `/suno/fetch`、GET `/suno/fetch/:id`

**A.2.7 未实现占位(11 条,返回 `RelayNotImplemented`)**

POST `/v1/images/variations`;`/v1/files`:GET、POST、GET `/:id`、GET `/:id/content`、DELETE `/:id`;`/v1/fine-tunes`:POST、GET、GET `/:id`、POST `/:id/cancel`、GET `/:id/events`;DELETE `/v1/models/:model`

> **处置**:sea-weir 保留这 11 条路由并返回同样的「未实现」错误体,保持客户端探测行为一致;不实现其业务语义(见 system-design.md §1.3 Out of Scope)。

### A.3 视频与任务面(11 条)

| 方法 路径 | 鉴权 |
|---|---|
| GET `/v1/videos/:task_id/content` | **TUA**(session 或 sk-token)+ SSRF 校验 |
| POST `/v1/video/generations`、GET `/v1/video/generations/:task_id` | TA + DIST |
| POST `/v1/videos`、GET `/v1/videos/:task_id`、POST `/v1/videos/:video_id/remix` | TA + DIST(remix 锁原渠道) |
| POST `/kling/v1/videos/text2video`、`/image2video`;GET `/kling/v1/videos/text2video/:task_id`、`/image2video/:task_id` | KlingRequestConvert + TA + DIST |
| POST `/jimeng/` | JimengRequestConvert + TA + DIST |

### A.4 兼容面 `/dashboard`(4 条,TA)

GET `/dashboard/billing/subscription`、`/v1/dashboard/billing/subscription`、`/dashboard/billing/usage`、`/v1/dashboard/billing/usage`

### A.5 计数汇总

| 面 | 源文件 | 条数 |
|---|---|---|
| 管理面 | `router/api-router.go` | 236 |
| 中继面 | `router/relay-router.go` | 54(MJ 16 条双前缀注册,展开 70) |
| 视频任务面 | `router/video-router.go` | 11 |
| 兼容面 | `router/dashboard.go` | 4 |
| **合计** | | **305**(展开 321) |

静态面(SPA 资源)由 `router/web-router.go` 承载,sea-weir 改由 nginx 托管,不计入路由数。

---

## 文档历史

| 版本 | 日期 | 作者 | 变更 |
|------|------|------|------|
| v1.0 | 2026-09-10 | Architecture Team | 初始编写,契约以 new-api 现行行为为兼容基线 |
| v1.1 | 2026-09-10 | Architecture Team | 管理面字段命名更正为 snake_case(P0);分页页码基数更正为 1 起;令牌脱敏格式对齐 `MaskTokenKey`;中继面补齐 MJ 全 16 条(含 `/:mode/mj` 双前缀)、`engines/:model/embeddings`、`edits`、`models/*path` 与 11 条占位端点;新增**附录 A:完整端点清单(305 条)** |
