# sea-weir AI 网关系统设计(React + Rust)

**文档版本**:v1.1
**最后更新**:2026-09-10
**范围**:sea-weir —— 对 new-api(QuantumNous/new-api,Go + Gin + GORM)的整体重构,React SPA 前端 + Rust 后端(`code/frontend`、`code/backend`)
**相关文档**:`architecture/CONTRACTS.md`、`architecture/adr/README.md`、`architecture/c4/*.puml`、`architecture/sequence-diagram/*.puml`、`architecture/er-diagram.puml`

---

## 1. 概述

### 1.1 项目背景

new-api 是开源的**新一代 LLM 网关与 AI 资产管理中台**(one-api 的下游增强分支),对外提供 OpenAI / Claude / Gemini 等兼容的中继 API,对内提供用户、令牌、渠道、计费、日志、充值、订阅等完整运营能力。其技术栈为 Go 1.x + Gin + GORM(单进程单体)+ React 18(纯 JSX + Semi Design),数据存储支持 SQLite / MySQL / PostgreSQL,缓存可选 Redis。

sea-weir 是对 new-api 的**整体重构**项目,目标架构为 **React 18 + TypeScript 前端 + Rust(axum + tokio + sqlx)后端**,数据库统一定型为 **openGauss**,缓存定型为 **Valkey**(Redis 协议兼容)。重构的业务动机:

1. **性能与资源效率**:中继面是 IO 密集 + 长连接(SSE/WebSocket)场景,Rust 零成本异步与无 GC 特性可降低 P99 延迟与常驻内存,单机承载更高的并发流式会话;
2. **类型安全与可维护性**:Go 版已有 `service/billing_session.go`(带 `settled`/`refunded` 幂等标志)收敛了计费主流程,但仍以 `*gin.Context` 贯穿三层传参,且额度扣减是无条件表达式(`model/user.go` 的 `quota - ?`,缺 `AND quota >= ?` 守卫),并发下存在扣成负值的窗口;重构为不依赖 HTTP 上下文的显式计费状态机 + 条件原子扣减 + sqlx 编译期校验;
3. **修复已知缺陷**:Go 版批量异步更新(`BATCH_UPDATE_ENABLED`,`common/constants.go` 中**默认关闭**)一旦开启,崩溃即丢失最近一个周期的账务数据 —— 本质是「性能换正确性」的开关,重构后由架构层面消除该取舍;前端大量服务端配置平铺进 localStorage;权限守卫仅依赖前端 localStorage 角色字段;
4. **公司技术栈统一**:数据库 openGauss、配置中心 Nacos、缓存 Valkey、K8s 部署形态与 service-auth / service-hr 等微服务一致。

### 1.2 设计目标

1. **契约兼容**:管理面 REST 路径、请求/响应 JSON 字段、`{success, message, data}` 包裹与 new-api 完全一致;中继面保持 OpenAI / Claude / Gemini 原生协议兼容(含错误格式),**客户端与前端可灰度替换、数据可迁移**(ADR-003、ADR-004)
2. **账务正确**:计费三段式(预扣 → 结算 → 退款)状态机化,额度扣减全部走数据库原子表达式,幂等键唯一约束兜底;去除批量异步更新的丢账窗口(ADR-005)
3. **类型安全**:Rust 强类型 + sqlx 编译期 SQL 校验;前端 TypeScript strict 模式,`tsc -b` 零错误
4. **无状态可横向扩展**:服务节点不持有会话状态(session 改为签名 Cookie 自承载 / Valkey 托管),渠道缓存、亲和性缓存支持多节点一致(ADR-007)
5. **安全**:中继面 sk-token 多源提取与 IP 白名单、管理面 session + access token + `New-Api-User` 防串号、敏感操作二次验证(2FA/Passkey)、出网 SSRF 防护、渠道密钥加密存储
6. **可观测**:tracing 结构化日志、请求级 request-id 贯穿中继链路与计费日志、渠道测试/监控内建

### 1.3 核心功能范围

**管理面(`/api/*`,浏览器控制台)**—— 共 16 个功能域:

- **用户域**:注册/登录(账密、邮箱验证码、2FA、Passkey/WebAuthn)、OAuth 登录(GitHub/Discord/LinuxDO/OIDC/微信/Telegram/自定义 Provider)、个人设置、访问令牌(access token)、签到、邀请返利;管理员用户 CRUD
- **令牌域(API key)**:令牌 CRUD、模型限制、IP 白名单、分组、批量操作、令牌自查余额/日志(只读)
- **渠道域**:渠道 CRUD、批量/标签管理、多 key 管理(random/polling)、渠道测试与余额探测、上游模型拉取、Codex OAuth 凭证、Ollama 管理、上游更新检测
- **日志域**:消费/充值/管理/系统/错误/退款日志查询、统计(rpm/tpm)、按令牌查询、历史清理、日志归档文件
- **计费与定价域**:模型定价(pricing)、模型倍率/分组倍率/缓存倍率配置(ratio_config)、上游倍率同步、阶梯计费、仪表盘计费兼容端点(`/dashboard/billing/*`)
- **充值与支付域**:兑换码 CRUD、在线充值(易支付/Stripe/Creem/Waffo)、支付回调(webhook)、人工补单
- **订阅域**:套餐计划 CRUD、订阅购买(epay/Stripe/Creem)、用户订阅管理、周期额度重置
- **模型域**:模型元数据/厂商 CRUD、模型同步、缺失模型检测、部署管理(io.net)
- **设置域**:系统选项 KV(Option)读写、分层配置组、控制台设置迁移、渠道亲和性缓存管理
- **安全域**:统一安全验证(密码/2FA/Passkey 换敏感操作凭证)、邮箱验证码、Turnstile 人机校验
- **任务域**:异步任务(视频/音乐)与 Midjourney 记录查询
- **数据看板域**:额度消耗聚合数据(按模型/用户)、Uptime 监控状态
- **分组域**:分组列表、预填分组(model/tag/endpoint)CRUD
- **系统域**:首装向导(setup)、系统状态(status)、公告/关于/协议文案
- **性能运维域**:运行统计、GC、磁盘缓存与性能日志管理(root)
- **OAuth Provider 管理域**:自定义 OAuth Provider CRUD 与发现(root)

**中继面(`/v1`、`/v1beta`、`/mj`、`/suno`、`/kling`、`/jimeng`、`/pg`)**:

- OpenAI 兼容:chat/completions、completions、responses(+compact)、images(generations/edits)、audio(speech/transcriptions/translations)、embeddings、rerank、moderations、models(列表/详情)、realtime(WebSocket)
- Claude 原生:`/v1/messages`;Gemini 原生:`/v1beta/models/{model}:{action}`(generateContent / streamGenerateContent / embedContent / batchEmbedContents)
- 异步任务:Midjourney 全系(submit ×11 / task fetch / image-seed / list-by-condition / insight-face,`/mj` 与 `/:mode/mj` 双前缀;`/mj/notify` 在 new-api 已注释停用,不迁移)、Suno、视频生成(OpenAI videos / Kling / 即梦 / Sora / 通义万相等)
- Playground:`/pg/chat/completions`(登录用户会话态调试)
- 核心能力:令牌认证 → 模型请求限流 → 渠道分发(分组×模型×优先级×权重、auto 分组、亲和性)→ 预估计费 → 预扣 → 格式转换 → 上游调用 → 流式转发 → 结算/退款 → 日志

**前端(React SPA)**—— 覆盖 new-api `web/src/pages/` 的 24 个页面目录,重组为 **28 个页面**
(差异:new-api 的登录/注册/OAuth 回调散在组件中,此处提升为 3 个独立页;个人设置由
`components/settings/PersonalSetting.jsx` 提升为独立页):
- **认证(3)**:登录 / 注册 / OAuth 回调(含 Passkey、2FA、Turnstile)
- **控制台(8)**:看板(Dashboard)/ 令牌(Token)/ 日志(Log)/ 绘图(Midjourney)/ 任务(Task)/ 钱包(TopUp)/ 个人设置(PersonalSetting)/ 操练场(Playground)
- **管理端(7)**:渠道(Channel)/ 模型(Model)/ 部署(ModelDeployment)/ 订阅(Subscription)/ 兑换码(Redemption)/ 用户(User)/ 系统设置(Setting,12 个配置组)
- **公共与运营文案(4)**:首页(Home,对接 `/api/home_page_content`)/ 关于(About,对接 `/api/about`)/ 用户协议(`/api/user-agreement`)/ 隐私政策(`/api/privacy-policy`)
- **外部聊天入口(2)**:Chat(嵌入式聊天页)/ Chat2Link(聊天链接跳转模板)
- **兜底页(2)**:404 NotFound / 403 Forbidden
- **模型广场(1)**:Pricing(定价视图,对接 `/api/pricing`,可匿名访问)
- **首装向导(1)**:Setup

**明确不做(Out of Scope)**:
- 不提供 gRPC 服务端(原项目无 gRPC 面,无迁移需求)
- ClickHouse 日志分析(原代码仅有占位变量,未实现;日志分库能力保留)
- Electron 桌面端(原仓库 `electron/` 不迁移)
- 支付渠道中已下线的历史渠道若公司无支付牌照需求,可在迭代中裁剪(ADR 不锁定)
- OpenAI files / fine-tunes / images.variations 共 11 条端点:new-api 本就是 `RelayNotImplemented` 占位,sea-weir **保留路由与错误体形状**以维持客户端探测行为,但不实现业务语义(清单见 CONTRACTS.md 附录 A.2.7)
- new-api 源码中已注释停用的 6 条路由(`/api/midjourney`、`/api/waffo-pancake/webhook`、`/api/user/tokenlog`、`/api/user/waffo-pancake/{amount,pay}`、`/mj/notify`)不迁移

---

## 2. 现有系统分析(迁移参照)

### 2.1 当前技术栈

| 层 | 现实现(new-api) | 位置 |
|---|---|---|
| 后端 | Go 1.x + Gin + GORM + go-redis v8 + tiktoken + gopool | `main.go`、`router/`、`controller/`、`service/`、`model/` |
| 前端 | React 18(**纯 JSX**)+ Semi Design + axios + react-router v6 + i18next + Tailwind + VChart,Context + useReducer 状态 | `web/src/` |
| 存储 | SQLite(默认)/ MySQL ≥5.7.8 / PostgreSQL ≥9.6(三方言);Redis 可选 | `model/main.go`、`common/redis.go` |
| 部署 | 单容器(后端 embed `web/dist` 静态资源),`:3000` | `Dockerfile`、`main.go` |

### 2.2 项目结构分析

- 后端为 Gin 单体:`router/` 五个路由文件(api/relay/video/dashboard/web)→ `middleware/`(认证/分发/限流)→ `controller/`(54 个文件,HTTP 编排)→ `service/`(业务逻辑)→ `model/`(GORM 模型 + 缓存)→ `setting/`(包级配置变量 + 注册式分层配置);中继适配器在 `relay/channel/`(36 个渠道目录 → 35 个 `ApiType` 常量,`APITypeDummy` 仅作计数位;另 `relay/channel/task/` 下 10 家异步任务平台)
- 前端为「页面薄壳 + table/域三件套(Table/ColumnDefs/Actions)+ 数据 hooks」模式;状态仅 UserContext / StatusContext / ThemeContext;API 层为单例 axios 实例(GET 去重),无数据缓存层
- 配置为「options 表 KV + 进程内 OptionMap + 包级变量」混合体,带 `.` 前缀的键走注册式配置组

### 2.3 当前数据流

```
浏览器 →(同源, embed 静态站)→ Gin :3000 ──┬─ /api/*  → controller → service → GORM → SQLite/MySQL/PG
AI 客户端 ──────────────────────────────┘            ↕ Redis(用户/令牌缓存、限流)
        └─ /v1|/v1beta|/mj|/suno → TokenAuth → Distribute(选渠道)→ Relay(预扣→上游→结算)
                                          ↘ 后台:渠道测试、MJ/Task 轮询、Option 同步、批量更新
```

### 2.4 已识别特性与问题(重构时的处置)

| 现实现特性/问题 | 处置 |
|---|---|
| 批量异步更新(`BATCH_UPDATE_ENABLED`,默认 false;开启后崩溃丢最近周期账务) | **账务路径无条件同步落库**(条件原子表达式),取消该开关;仅统计口径(used_quota/request_count/quota_data)批量合并(ADR-005) |
| 用户/令牌额度 Go int 与渠道 bigint 宽度不一致 | 统一 `BIGINT`(i64)存储额度(ADR-003) |
| 额度扣减为无条件表达式(`quota - ?`),并发下可扣成负值 | 改为条件原子扣减 `... WHERE id=$2 AND quota >= $1`,影响 0 行即余额不足(ADR-005) |
| 配置平铺进 localStorage(20+ 键) | 收敛为单一 status store,仅持久化用户偏好(ADR-009) |
| 路由守卫仅查 localStorage `user.role` | 前端静态路由 + 服务端逐端点鉴权为准,前端守卫仅为体验层(ADR-009) |
| 三方言(SQLite/MySQL/PG)分支与保留字转义 | 定型 openGauss 单方言语义,sqlx 编译期校验(ADR-003) |
| session 存 cookie store,多节点共享依赖相同密钥 | 签名 Cookie 自承载会话 + Valkey 会话吊销列表(ADR-004) |
| 中继失败重试/渠道禁用逻辑散在 controller/service | 归并为 relay_engine 内的显式状态机(ADR-006、ADR-008) |
| 邮件验证码内存存储(多节点不共享) | 迁移 Valkey(ADR-007) |
| 敏感操作凭证(取渠道 key 等)由 session 标记 | 保留语义,改为短时签名凭证 + Valkey 一次性消费(ADR-004) |

---

## 3. C4 架构设计

> PlantUML(C4-PlantUML stdlib)源文件位于 `architecture/c4/`,本章 ASCII 简图与其保持一致。

### 3.1 L1:系统上下文图

```
                        ┌──────────────┐          ┌──────────────┐
                        │  终端用户      │          │  运营管理员    │
                        │ (Person)     │          │  (Person)    │
                        └──────┬───────┘          └──────┬───────┘
                               │ HTTPS(浏览器)            │ HTTPS(浏览器)
                               ▼                          ▼
                    ┌─────────────────────────────────────────┐
                    │                sea-weir                 │
                    │   AI 网关与大模型 API 管理中台             │
                    │   (React SPA + Rust axum 服务)           │
                    └──┬───────┬──────────┬─────────┬─────────┘
                       │       │          │         │
        OpenAI/Claude/ │       │ SQL      │ RESP    │ HTTP
        Gemini 兼容 API │       ▼          ▼         ▼
                       ▼   ┌────────┐ ┌────────┐ ┌─────────┐
   ┌────────────────────┐  │openGauss│ │ Valkey │ │  Nacos  │
   │   上游模型提供方      │  │ 主库+日志库│ │ 缓存/限流│ │ 配置中心 │
   │ (OpenAI/Claude/    │  └────────┘ └────────┘ └─────────┘
   │  Gemini/火山/… 35家)│          ▲
   └────────────────────┘          │ HTTPS(中继转发)
                                   │
              ┌────────────────────┼─────────────┐
              ▼                    ▼             ▼
        ┌───────────┐      ┌────────────┐  ┌──────────┐
        │ AI 客户端  │      │ OAuth 提供方 │  │ 支付渠道   │
        │ (sk-token)│      │ (GitHub 等) │  │(易支付等) │
        └───────────┘      └────────────┘  └──────────┘
```

- **终端用户**:浏览器使用控制台(令牌、钱包、日志、操练场)
- **运营管理员**:浏览器管理渠道/定价/用户/设置;admin/root 两级
- **AI 客户端**:以 `sk-xxx` 令牌调用 OpenAI/Claude/Gemini 兼容 API
- **上游模型提供方**:35 个同步渠道适配器(`ApiType` 常量数,对应 `relay/channel/` 36 个目录)+ 10 家异步任务平台
- **openGauss / Valkey / Nacos**:公司标准基础设施(ADR-002、ADR-003)
- **OAuth 提供方 / 支付渠道**:外部依赖,回调入口无鉴权但签名校验

### 3.2 L2:容器图

```
┌──────────────────────────────── sea-weir 系统边界 ────────────────────────────────┐
│                                                                                  │
│  ┌────────────────┐   静态资源    ┌───────────────────────────────────────────┐   │
│  │  web_frontend  │◀────────────│ 浏览器(终端用户/运营管理员)                  │   │
│  │  React SPA     │             └───────────────────────────────────────────┘   │
│  │  (nginx 托管)   │◀── /api/* REST(session cookie + New-Api-User)──────────┐   │
│  └────────────────┘                                                          │   │
│                                                                              │   │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │   │
│  │ api_server   Rust / axum :8080                                          │◀┼───┘
│  │  ├ router           管理面/中继面/兼容面/静态面路由                          │ │   ◀── /v1|/v1beta|/mj|/suno|/kling
│  │  ├ middleware       session/token 认证、限流、i18n、request-id、性能护栏     │ │        (AI 客户端, sk-token)
│  │  └ handlers         16 个管理功能域编排                                    │ │
│  └───────┬──────────────────────────────────┬──────────────────────────────┘ │
│          │ 调用                              │ 调用                            │
│  ┌───────▼───────────────┐         ┌────────▼────────────────────────────┐   │
│  │ admin_engine          │         │ relay_engine                        │   │
│  │ (sea-weir-core)       │         │ (sea-weir-core)                     │   │
│  │ 用户/渠道/令牌/订单/      │         │ 渠道选择/适配器/计费状态机/             │   │
│  │ 设置/看板 业务逻辑       │         │ 流式转发/任务轮询/自动禁用             │   │
│  └───────┬───────────────┘         └────────┬───────────────┬────────────┘   │
│          │ 数据操作                          │ 数据操作        │ HTTPS/WS      │
│  ┌───────▼───────────────────────────────────▼──────────┐   │               │
│  │ repository_layer  Rust / sqlx(Repository Trait 实现)  │   │               │
│  └───────┬───────────────────────────────────┬──────────┘   │               │
└──────────┼───────────────────────────────────┼──────────────┼───────────────┘
           ▼                                   ▼              ▼
   ┌───────────────┐                  ┌──────────────┐  ┌──────────────┐
   │ opengauss_db  │                  │ redis_cache  │  │ upstream_llm │
   │ openGauss     │                  │ Valkey :6379 │  │ 上游模型提供方 │
   │ (主库+日志库)   │                  │ 缓存/限流/会话 │  │ (OpenAI 等)  │
   └───────────────┘                  └──────────────┘  └──────────────┘
```

| 容器 | 技术 | 职责 |
|---|---|---|
| web_frontend | React 18 + TS(nginx 托管) | 控制台与管理端 SPA;`/api`、`/v1` 由 nginx 反代到后端 |
| api_server | Rust / axum :8080 | 统一 HTTP 入口:路由、中间件链、管理面 handlers、中继面入口 |
| admin_engine | Rust / sea-weir-core | 16 个管理域业务编排(用户/渠道/令牌/订单/设置/看板) |
| relay_engine | Rust / sea-weir-core | 中继管线:渠道选择、适配器调用、计费状态机、流式转发、任务轮询 |
| repository_layer | Rust / sqlx | Repository Trait 实现、连接池(主库/日志库)、迁移 |
| opengauss_db | openGauss :5432 | 主数据存储 + 日志库(逻辑分离 pool) |
| redis_cache | Valkey :6379 | 用户/令牌/亲和性缓存、限流窗口、会话吊销、幂等键 |
| upstream_llm | 外部 | 35 个同步渠道适配器(ApiType)+ 10 家异步任务平台 |

### 3.3 L3:组件设计

**api_server 组件**(完整见 `c4/c4-l3-component-api-server.puml`):

| 组件 | 源文件 | 职责 |
|---|---|---|
| router | `sea-weir-server/src/router/*.rs` | 四个路由面注册与中间件挂载(api / relay / dashboard / web) |
| auth_middleware | `middleware/auth.rs` | session/access token/sk-token 三级认证,`New-Api-User` 防串号,角色闸门(UserAuth/AdminAuth/RootAuth/TokenAuth) |
| rate_limit | `middleware/rate_limit.rs` | IP/用户维度滑动窗口限流(Valkey/内存降级) |
| distribute_middleware | `middleware/distribute.rs` | 中继面渠道分发:模型解析、亲和性命中、选渠、渠道上下文注入 |
| admin_handlers | `handlers/*.rs`(16 域) | 管理面编排:参数校验 → admin_engine → 统一响应包裹 |
| relay_entry | `handlers/relay.rs` | 中继面入口:重试循环、错误出口格式化 |
| response | `response.rs` | `{success,message,data}` 与 OpenAI/Claude 错误格式出口 |

**relay_engine 组件**(完整见 `c4/c4-l3-component-relay-engine.puml`):

| 组件 | 源文件 | 职责 |
|---|---|---|
| channel_selector | `core/relay/select.rs` | (group, model, retry) → 优先级档 → 档内加权随机;auto 分组降级;亲和性优先 |
| channel_cache | `core/relay/channel_cache.rs` | 进程内 `group→model→[channel]` 索引 + Valkey 失效广播 |
| adaptor_registry | `sea-weir-adaptors/src/lib.rs` | `Adaptor` / `TaskAdaptor` trait 注册表(35 同步 + 10 任务实现) |
| format_convert | `core/relay/convert.rs` | OpenAI ⇄ Claude ⇄ Gemini 请求/响应/流式改写 |
| billing | `core/relay/billing.rs` | 计费状态机:预估 → 预扣(信任旁路)→ 结算 → 退款;幂等 |
| stream_pipe | `core/relay/stream.rs` | SSE 行扫描/延迟一拍转发/usage 注入/ping 保活;WSS 双向 pipe |
| task_polling | `core/relay/task_polling.rs` | 异步任务 15s 轮询、CAS 终态、补差/退款 |
| autoban | `core/relay/autoban.rs` | 渠道错误判定(状态码+关键词)→ 单 key/多 key 粒度禁用 → 通知 |

**web_frontend 组件**(完整见 `c4/c4-l3-component-web-frontend.puml`):

| 组件 | 源文件 | 职责 |
|---|---|---|
| app_shell | `App.tsx`、`layouts/AppLayout.tsx` | 主题(亮/暗)、路由守卫(体验层)、布局(顶栏+侧边导航) |
| api_layer | `api/client.ts`、`api/modules/*.ts` | axios 封装(New-Api-User 头、success=false 统一报错、GET 去重)+ 按域 API 函数 |
| stores | `stores/{user,status,theme}.ts` | zustand:登录态、全站配置(服务端 `/api/status`)、主题 |
| table_kit | `components/table/{DataTable,ColumnDefs,Filters,Actions}.tsx` | 域表格三件套(11 个域复用) |
| console_pages | `pages/console/*.tsx` | 用户端 8 页(看板/令牌/日志/绘图/任务/钱包/个人/操练场) |
| admin_pages | `pages/admin/*.tsx` | 管理端 7 页(渠道/模型/订阅/兑换码/用户/设置/部署) |
| playground | `pages/playground/*`、`hooks/playground/*` | 聊天调试台(SSE 直连 /pg) |
| public_pages | `pages/public/*.tsx` | 运营文案 4 页(首页/关于/用户协议/隐私政策,文案取自 `/api/*`)+ 兜底 2 页(404/403) |
| chat_entry | `pages/chat/*.tsx` | 外部聊天入口 2 页(Chat 嵌入页 / Chat2Link 链接跳转模板) |
| pricing_pages | `pages/pricing/*.tsx` | 模型广场/定价 |

### 3.4 模块依赖

与 `architecture/module-dependency.puml` 一致。后端 workspace 五个 crate,依赖方向单向无环:

```
sea-weir-server → sea-weir-core → sea-weir-repository → sea-weir-types
sea-weir-core   → sea-weir-adaptors → sea-weir-types
sea-weir-server → sea-weir-types
```

前端分层:页面层(pages)→ 通用组件层(components/hooks)→ API 层(api)→ 状态层(stores)。

---

## 4. 技术栈选型

> 完整论证见 `adr/ADR-002-tech-stack-selection.md`。

### 4.1 Rust 语言选型

中继面是典型的高并发 IO 密集 + 长连接(SSE 流式、WebSocket realtime)负载;计费链路要求强一致与可审计。Rust 提供:内存安全(无数据竞争)、tokio 零成本异步(单节点万级并发流)、sqlx 编译期 SQL 校验、强类型状态机(计费三段式)。详见 ADR-002。

### 4.2 核心依赖表

**后端(Rust)**:

| 依赖 | 版本 | 用途 |
|------|------|------|
| tokio | 1.x | 异步运行时 |
| axum | 0.8 | HTTP 框架(:8080),管理面 + 中继面统一入口 |
| sqlx | 0.8(macros) | openGauss/PostgreSQL 访问,编译期 SQL 校验;双连接池(主库/日志库) |
| reqwest | 0.12(rustls, stream) | 上游 HTTP 调用(连接池、代理、SSE 流式读取) |
| tokio-tungstenite | 0.26 | realtime WebSocket 双向转发 |
| serde / serde_json | 1 | DTO 序列化;全局 `rename_all = "snake_case"`,与 new-api 现契约逐字段一致 |
| tower / tower-http | 0.5 / 0.6 | 中间件:CORS、gzip、超时、限流 |
| redis(fred) | — | Valkey 客户端(协议兼容) |
| jsonwebtoken | 9 | 会话/敏感操作签名凭证(HS256) |
| bcrypt / totp-rs | — | 密码哈希、2FA TOTP |
| webauthn-rs | 0.5 | Passkey 注册/登录 |
| tiktoken-rs | — | prompt token 预估 |
| rust-embed | 8 | 可选嵌入前端 dist(单容器部署形态) |
| clap | 4 | `-c/--config` 配置参数 |
| tracing + tracing-subscriber | 0.1 / 0.3 | 结构化日志,request-id 贯穿 |
| rust_decimal | 1 | 计费精确小数运算 |
| moka | 0.12 | 进程内缓存(渠道索引、定价视图) |

**前端(React)**:

| 依赖 | 版本 | 用途 |
|------|------|------|
| react / react-dom | 18 | UI 框架 |
| react-router-dom | 6(BrowserRouter) | 路由(nginx 配 SPA fallback) |
| antd / @ant-design/icons | 5 | 组件库(公司统一,替换 Semi Design,见 ADR-009) |
| zustand | 4(persist 仅用户偏好) | 状态管理(替换 Context+useReducer) |
| axios | 1 | HTTP 客户端(GET 去重、统一错误提示) |
| @ant-design/charts | 2 | 看板图表(替换 VChart) |
| i18next + react-i18next | 23 / 13 | 多语言,**7 种全量沿用**:zh-CN / zh-TW / en / ja / fr / ru / vi(与 new-api `web/src/i18n/locales/` 一致,不裁剪) |
| dayjs | 1 | 时间处理 |
| vite | 5 | 构建(`tsc -b` 随构建检查) |
| typescript | 5.5(strict) | 类型检查 |

### 4.3 数据库选型

**定型 openGauss**(公司标准,PostgreSQL 系语义;ADR-003):

- 原三方言(SQLite/MySQL/PostgreSQL)收敛为单方言,消除保留字转义与类型宽度分支;
- 表结构与 new-api **同名同语义**(便于存量 MySQL 数据迁移与回滚);列类型按 ADR-003 的映射表(如 `int`→`BIGINT`、`text`→`TEXT`/`JSONB`、unix 秒时间戳保留 `BIGINT`);
- JSON 列(channel_info / setting / properties 等)统一 `JSONB`;
- 软删除沿用 `deleted_at`,唯一约束用**部分索引**(`WHERE deleted_at IS NULL`);
- 日志表(logs)物理上分库:独立 `LOG_SQL_DSN` 连接池,默认同库。

### 4.4 分层架构图

```
后端:入口层(axum router + middleware)
        → 编排层(handlers:16 管理域 + relay 入口)
        → 业务层(sea-weir-core:admin_engine / relay_engine / adaptors)
        → 数据层(sea-weir-repository:Repository Trait + sqlx)
        → 基础层(sea-weir-types:DTO / AppError / 常量 / 配置)

前端:页面层(pages)→ 通用组件层(components/hooks)→ API 层(api)→ 状态层(stores)
```

---

## 5. 模块设计

### 5.1 Rust Workspace 树形结构

```
code/backend/                              code/frontend/src/
├── Cargo.toml          # workspace        ├── main.tsx / App.tsx       # 入口/路由
├── migrations/         # sqlx DDL         ├── layouts/AppLayout.tsx
└── crates/                                ├── pages/
    ├── sea-weir-server/  # 二进制入口      │   ├── console/×8          # 用户端
    │   └── src/                           │   ├── admin/×7            # 管理端
    │       ├── main.rs   # 启动/装配       │   ├── auth/×3   public/×6
    │       ├── router/   # 4 路由面        │   ├── chat/×2  pricing/ playground/
    │       ├── middleware/ # 认证/限流/分发  │   └── setup/              # 首装向导
    │       ├── handlers/   # 16 域+relay  ├── components/
    │       └── response.rs               │   ├── table/×11 域三件套
    ├── sea-weir-core/                     │   └── common/ settings/ topup/
    │   └── src/                           ├── hooks/                  # 按域数据 hooks
    │       ├── admin/      # 管理业务      ├── api/{client.ts,modules/} # axios 层
    │       └── relay/      # 中继引擎      ├── stores/{user,status,theme}.ts
    │           ├── select.rs channel_cache.rs  ├── utils/{quota,render,crypto}.ts
    │           ├── billing.rs stream.rs        └── i18n/locales/*.json  # 7 种语言
    │           ├── task_polling.rs autoban.rs
    │           └── convert.rs
    ├── sea-weir-adaptors/  # 渠道适配器(Adaptor/TaskAdaptor trait + 35 同步 + 10 任务实现)
    ├── sea-weir-repository/ # Repository Trait + sqlx 实现 + 缓存回写
    └── sea-weir-types/     # DTO/领域模型/AppError/常量/分层配置
```

### 5.2 模块依赖图

见 `architecture/module-dependency.puml`。约束:`sea-weir-types` 不依赖任何内部 crate;`sea-weir-repository` 不依赖 core/adaptors;`sea-weir-adaptors` 不依赖 repository(渠道数据经 core 注入),依赖方向无环。

### 5.3 模块职责矩阵(后端)

| Crate | 职责 | 依赖 |
|-------|------|------|
| sea-weir-server | 启动装配(配置/连接池/缓存初始化/后台任务)、路由注册、中间件、handlers、统一响应出口 | core, adaptors, repository, types |
| sea-weir-core | admin 域业务逻辑;relay 引擎(选路/计费/流式/任务/自动禁用/格式转换) | adaptors, repository, types |
| sea-weir-adaptors | `Adaptor`/`TaskAdaptor` trait 与 35 同步 + 10 任务渠道实现;请求/响应格式转换 | types |
| sea-weir-repository | Repository Trait + sqlx 实现、迁移、Valkey 缓存读写与失效广播 | types |
| sea-weir-types | 领域模型、REST/中继 DTO、`AppError` 与错误码、常量、分层配置结构 | — |

**前端模块职责矩阵**:

| 模块 | 职责 | 依赖 |
|-------|------|------|
| App / AppLayout | 主题、路由注册、守卫(体验层)、布局 | stores, pages |
| api | axios 实例(New-Api-User、success 判定、GET 去重)、按域函数 | stores, types |
| stores | user(登录态)/status(全站配置)/theme;仅用户偏好持久化 | — |
| table 三件套 + hooks | 配置驱动表格(搜索/列定义/操作/分页) | api |
| pages | 页面编排 | 以上全部 |

---

## 6. 接口契约设计

> 完整契约(请求/响应字段、前置/后置条件、错误传播)见 `architecture/CONTRACTS.md`,两者必须一致。

### 6.1 统一响应包裹(双协议面)

| 面 | 成功 | 业务错误 | 说明 |
|---|---|---|---|
| 管理面 `/api/*` | HTTP 200 `{"success":true,"message":"","data":…}` | HTTP 200 `{"success":false,"message":"…"}`(鉴权失败 401/403) | 与 new-api 逐字段一致 |
| 中继面 `/v1` 等 | 上游原生响应(OpenAI/Claude/Gemini 格式) | `{"error":{"message","type","code"}}`(Claude 路径 `{"type":"error","error":{…}}`) | OpenAI 兼容错误,ADR-008 |

> **字段命名铁律**:管理面所有请求/响应 DTO 字段一律 **snake_case**(new-api 的 Go struct tag 即 snake_case,如 `display_name`、`remain_quota`、`expired_time`、`model_limits_enabled`、`unlimited_quota`、`aff_code`、`last_login_at`)。Rust 侧统一 `#[serde(rename_all = "snake_case")]`,任何 camelCase 字段都会破坏 §1.2 目标 1「客户端与前端可灰度替换」。契约测试须对 new-api 实际响应做字段集比对。

### 6.2 HTTP 路由表(摘要)

**管理面(`/api`)**:setup、status、notice 等公开端点;`/api/user`(注册/登录/2FA/Passkey/个人/充值/签到 + AdminAuth CRUD)、`/api/token`(UserAuth)、`/api/channel`(AdminAuth,+取密钥 RootAuth+二次验证)、`/api/log`、`/api/data`、`/api/redemption`、`/api/group`、`/api/prefill_group`、`/api/option`(RootAuth)、`/api/pricing`、`/api/ratio_config`、`/api/ratio_sync`(RootAuth)、`/api/models`、`/api/vendors`、`/api/deployments`(AdminAuth)、`/api/subscription`(UserAuth)+`/api/subscription/admin`(AdminAuth)、`/api/custom-oauth-provider`(RootAuth)、`/api/performance`(RootAuth)、`/api/mj`、`/api/task`、`/api/usage/token`(TokenAuthReadOnly)、`/api/oauth/*`、支付 webhook。完整 305 条端点清单(逐条含鉴权与限流中间件标注)见 CONTRACTS.md **附录 A**。

**中继面**:`/v1/{chat/completions,completions,responses(+compact),messages,edits,images/{generations,edits},audio/{speech,transcriptions,translations},embeddings,engines/:model/embeddings,rerank,moderations,models,models/*path,realtime}`、`/v1beta/models/*`、`/mj/**`(同时注册于 `/:mode/mj/**`)、`/suno/**`、`/v1/video*/**`、`/kling/v1/**`、`/jimeng/`、`/pg/chat/completions`;另有 11 条 OpenAI files / fine-tunes / images.variations 占位端点原样保留并返回「未实现」。**兼容面**:`/dashboard/billing/{subscription,usage}`(含 `/v1` 前缀变体);**静态面**:SPA 资源改由 nginx 托管。

### 6.3 Adaptor Trait 契约(中继核心抽象)

```rust
#[async_trait]
pub trait Adaptor: Send + Sync {
    fn init(&self, info: &RelayInfo) -> Result<(), AppError>;
    fn get_request_url(&self, info: &RelayInfo) -> Result<String, AppError>;
    fn setup_request_header(&self, headers: &mut HeaderMap, info: &RelayInfo) -> Result<(), AppError>;
    fn convert_request(&self, req: RelayRequest, info: &RelayInfo) -> Result<Bytes, AppError>;
    async fn do_request(&self, info: &RelayInfo, body: Bytes) -> Result<UpstreamResponse, AppError>;
    async fn do_response(&self, resp: UpstreamResponse, info: &mut RelayInfo)
        -> Result<Usage, NewApiError>;   // 流式经 stream_pipe 驱动
    fn get_model_list(&self) -> &[String];
}

#[async_trait]
pub trait TaskAdaptor: Send + Sync {
    // ValidateRequestAndSetAction / EstimateBilling / AdjustBillingOnSubmit /
    // AdjustBillingOnComplete / BuildRequest* / FetchTask / ParseTaskResult
}
```

### 6.4 核心 Repository Trait

`UserRepository` / `TokenRepository` / `ChannelRepository` / `AbilityRepository` / `LogRepository` / `OptionRepository` / `OrderRepository`(topup/subscription)/ `TaskRepository` 等,签名与前置/后置条件见 CONTRACTS.md §5。关键不变量:额度增减必须为数据库原子表达式;兑换/补单/订阅预扣必须事务 + 行锁(`FOR UPDATE`);幂等键唯一约束兜底。

### 6.5 统一错误类型

`AppError` 单一枚举 + 四级分类(Transient / Permanent / Recoverable / Unrecoverable),`NewApiError`(中继面:status_code + error_code + 格式类型 + skip_retry 标记)→ 出口按 relay 格式转 OpenAI/Claude 错误。详见 ADR-008 与第 10 章。

### 6.6 业务规则约定(关键不变量)

1. **角色语义**:0 guest / 1 common / 10 admin / 100 root;`UserAuth≥1`、`AdminAuth≥10`、`RootAuth≥100`;root 唯一(仅初始 root)
2. **额度单位**:`QuotaPerUnit = 500000`,即 500000 quota = $1(模型倍率 1 = $0.002/1K tokens)
3. **令牌分组**:令牌分组非空则覆盖用户分组;`auto` 分组按配置的分组列表逐组降级;分组倍率 = GroupRatio × GroupGroupRatio(用户分组对使用分组的特殊倍率)
4. **渠道选择**:`(group, model)` → 第 retry 高优先级档 → 档内加权随机(weight+10 平滑);亲和性命中优先;`sk-xxx-{channelId}` 仅 root/admin 可指定渠道
5. **计费三段式**:预扣(信任旁路可选)→ 结算(差额补退,幂等)→ 失败退款(异步幂等);结算后不退
6. **日志类型**:0 Unknown / 1 Topup / 2 Consume / 3 Manage / 4 System / 5 Error / 6 Refund

---

## 7. 数据库设计

### 7.1 ER 图(ASCII 简图)

完整 ER 图见 `architecture/er-diagram.puml`(openGauss,26 张表,与 new-api `model/main.go` 的 AutoMigrate 清单一一对应)。核心关系:

```
users 1───N tokens                 channels 1───N abilities(复合PK: group+model+channel_id)
users 1───N logs(写日志库)         channels 1───N tasks / midjourneys
users 1───N top_ups / checkins / user_oauth_bindings / file_objects
users 1───1 two_fas / passkey_credentials
users 1───N user_subscriptions N───1 subscription_plans 1───N subscription_orders
user_subscriptions 1───N subscription_pre_consume_records(幂等: request_id 唯一)
vendors 1───N models(模型元数据)   options(配置 KV)   quota_data(看板聚合)
redemptions(兑换码)   prefill_groups   custom_oauth_providers   setups(首装标记)
```

### 7.2 SQL DDL(openGauss 语法,核心表)

> **权威 DDL 位于 `data/ddl.sql`**(26 张表 + 索引,openGauss,全部 `IF NOT EXISTS` 幂等且无破坏性语句);
> `code/backend/migrations/` 是 sqlx 迁移的落点,内容与 `data/ddl.sql` 保持同步。
> 此处仅给出核心 8 表摘要;初始化/种子/分环境脚本见 `data/README.md`。类型映射规则见 ADR-003。

```sql
CREATE TABLE users (
  id            BIGSERIAL PRIMARY KEY,
  username      VARCHAR(64)  NOT NULL,
  password      VARCHAR(255) NOT NULL DEFAULT '',      -- bcrypt
  display_name  VARCHAR(64)  DEFAULT '',
  role          INT          NOT NULL DEFAULT 1,       -- 0/1/10/100
  status        INT          NOT NULL DEFAULT 1,       -- 1 启用 2 禁用
  email         VARCHAR(128) DEFAULT '',
  github_id     VARCHAR(64)  DEFAULT '',               -- 其余 OAuth 列同型:discord_id/oidc_id/wechat_id/telegram_id/linux_do_id
  access_token  CHAR(32),                              -- 系统访问令牌
  quota         BIGINT       NOT NULL DEFAULT 0,       -- 钱包剩余额度
  used_quota    BIGINT       NOT NULL DEFAULT 0,
  request_count BIGINT       NOT NULL DEFAULT 0,
  "group"       VARCHAR(64)  NOT NULL DEFAULT 'default',
  aff_code      VARCHAR(32), aff_count INT DEFAULT 0,
  aff_quota     BIGINT DEFAULT 0, aff_history_quota BIGINT DEFAULT 0,
  inviter_id    BIGINT, stripe_customer VARCHAR(64),
  setting       JSONB,                                 -- 语言/通知/计费偏好
  created_at    BIGINT NOT NULL DEFAULT 0, last_login_at BIGINT DEFAULT 0,
  deleted_at    TIMESTAMPTZ
);
CREATE UNIQUE INDEX uk_users_username ON users(username) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX uk_users_access_token ON users(access_token) WHERE access_token IS NOT NULL;
CREATE UNIQUE INDEX uk_users_aff_code ON users(aff_code) WHERE aff_code IS NOT NULL;

CREATE TABLE tokens (
  id BIGSERIAL PRIMARY KEY, user_id BIGINT NOT NULL,
  key CHAR(48) NOT NULL,                             -- 唯一索引;sk- 前缀不入库
  status INT NOT NULL DEFAULT 1,                     -- 1启用/2禁用/3过期/4额度耗尽
  name VARCHAR(64) DEFAULT '',
  created_time BIGINT DEFAULT 0, accessed_time BIGINT DEFAULT 0,
  expired_time BIGINT DEFAULT -1,                    -- -1 永不过期
  remain_quota BIGINT NOT NULL DEFAULT 0, unlimited_quota BOOLEAN NOT NULL DEFAULT FALSE,
  model_limits_enabled BOOLEAN NOT NULL DEFAULT FALSE, model_limits TEXT DEFAULT '',
  allow_ips TEXT, used_quota BIGINT NOT NULL DEFAULT 0,
  "group" VARCHAR(64) NOT NULL DEFAULT '',           -- 非空覆盖用户分组
  cross_group_retry BOOLEAN NOT NULL DEFAULT FALSE,
  deleted_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX uk_tokens_key ON tokens(key) WHERE deleted_at IS NULL;
CREATE INDEX idx_tokens_user ON tokens(user_id);

CREATE TABLE channels (
  id BIGSERIAL PRIMARY KEY, type INT NOT NULL,       -- 渠道类型→ApiType 映射
  key TEXT NOT NULL,                                 -- 多 key 换行分隔
  status INT NOT NULL DEFAULT 1,                     -- 1启用/2手动禁用/3自动禁用
  name VARCHAR(64) DEFAULT '', weight BIGINT DEFAULT 0, priority BIGINT DEFAULT 0,
  "group" VARCHAR(64) DEFAULT 'default',             -- 逗号分隔多分组
  models TEXT DEFAULT '',                            -- 逗号分隔模型
  model_mapping JSONB, param_override JSONB, header_override JSONB,
  base_url VARCHAR(255), openai_organization VARCHAR(64), test_model VARCHAR(64),
  balance DOUBLE PRECISION DEFAULT 0, balance_updated_time BIGINT DEFAULT 0,
  used_quota BIGINT NOT NULL DEFAULT 0,
  auto_ban INT DEFAULT 1, tag VARCHAR(64),
  setting JSONB, other_settings JSONB, channel_info JSONB,  -- 含 multi-key 状态
  status_code_mapping VARCHAR(1024), other_info TEXT,
  created_time BIGINT DEFAULT 0, test_time BIGINT DEFAULT 0, response_time BIGINT DEFAULT 0
);
CREATE INDEX idx_channels_tag ON channels(tag);

CREATE TABLE abilities (                             -- 渠道-模型-分组三元组(选路索引)
  "group" VARCHAR(64) NOT NULL, model VARCHAR(255) NOT NULL, channel_id BIGINT NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  priority BIGINT DEFAULT 0, weight BIGINT DEFAULT 0, tag VARCHAR(64),
  PRIMARY KEY ("group", model, channel_id)
);
CREATE INDEX idx_abilities_priority ON abilities(priority);

CREATE TABLE logs (                                  -- 日志库(可独立 LOG_SQL_DSN)
  id BIGSERIAL PRIMARY KEY, user_id BIGINT NOT NULL, created_at BIGINT NOT NULL,
  type INT NOT NULL DEFAULT 0, content TEXT, username VARCHAR(64), token_name VARCHAR(64),
  model_name VARCHAR(128), quota BIGINT DEFAULT 0,
  prompt_tokens BIGINT DEFAULT 0, completion_tokens BIGINT DEFAULT 0,
  use_time INT DEFAULT 0, is_stream BOOLEAN DEFAULT FALSE,
  channel_id BIGINT, token_id BIGINT, "group" VARCHAR(64), ip VARCHAR(64),
  request_id VARCHAR(64), other JSONB, request_body TEXT, response_body TEXT
);
CREATE INDEX idx_logs_created_id ON logs(created_at, id);
CREATE INDEX idx_logs_user_id ON logs(user_id, id);

CREATE TABLE options ( key VARCHAR(128) PRIMARY KEY, value TEXT );

CREATE TABLE redemptions (
  id BIGSERIAL PRIMARY KEY, user_id BIGINT NOT NULL,
  key CHAR(32) NOT NULL, status INT NOT NULL DEFAULT 1,   -- 1可用/2禁用/3已用
  name VARCHAR(64), quota BIGINT NOT NULL DEFAULT 100,
  created_time BIGINT DEFAULT 0, redeemed_time BIGINT DEFAULT 0,
  used_user_id BIGINT, expired_time BIGINT DEFAULT 0, deleted_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX uk_redemptions_key ON redemptions(key);

CREATE TABLE top_ups (
  id BIGSERIAL PRIMARY KEY, user_id BIGINT NOT NULL,
  amount BIGINT NOT NULL, money NUMERIC(12,2) NOT NULL,
  trade_no VARCHAR(255) NOT NULL,                       -- 幂等:支付流水号
  payment_method VARCHAR(50), payment_provider VARCHAR(50),
  create_time BIGINT DEFAULT 0, complete_time BIGINT DEFAULT 0,
  status VARCHAR(20) NOT NULL DEFAULT 'pending'         -- pending/success/failed/expired
);
CREATE UNIQUE INDEX uk_topups_trade_no ON top_ups(trade_no);
```

其余 18 表(`subscription_plans` / `subscription_orders` / `user_subscriptions` / `subscription_pre_consume_records` / `tasks` / `midjourneys` / `checkins` / `models` / `vendors` / `prefill_groups` / `passkey_credentials` / `two_fas` / `two_fa_backup_codes` / `user_oauth_bindings` / `custom_oauth_providers` / `quota_data` / `file_objects` / `setups`)的字段与索引与 er-diagram.puml 一致,DDL 以 migrations 为准。

### 7.3 openGauss 特性使用 / 数据特征

- **JSONB**:channels 的 model_mapping/param_override/channel_info、tasks 的 properties/private_data、users.setting 等,用 `->>` / `@>` 查询;
- **部分索引**:软删除表的唯一约束全部 `WHERE deleted_at IS NULL`;
- **保留字**:`"group"`、`"key"` 统一双引号引用,sqlx 编译期校验;
- **原子额度更新**:`UPDATE users SET quota = quota - $1 WHERE id = $2 AND quota >= $1`(条件原子扣减,0 行即余额不足);
- **行锁幂等**:兑换码/订阅订单/订阅预扣走 `SELECT … FOR UPDATE` + 唯一约束;任务终态 CAS:`UPDATE tasks SET status=$1 WHERE id=$2 AND status=$3`;
- **时间戳**:业务表沿用 `BIGINT` unix 秒(兼容存量数据),跨节点以数据库时间为准(`SELECT EXTRACT(EPOCH FROM now())`)。

### 7.4 Rust Model 映射

| SQL | Rust | 说明 |
|---|---|---|
| BIGINT | i64 | 额度/时间戳统一 i64 |
| VARCHAR/TEXT | String | |
| JSONB | serde_json::Value / 强类型 struct | setting/properties 用强类型 |
| BOOLEAN | bool | |
| NUMERIC(10,6) | rust_decimal::Decimal | 金额 |
| TIMESTAMPTZ | Option\<chrono::DateTime\<Utc\>\> | 仅软删除列 |

---

## 8. 时序设计

> PlantUML 源文件位于 `architecture/sequence-diagram/`,参与者名称来自 C4 L2 容器定义。

### 8.1 SEQ-001:用户登录与管理面请求认证

POST `/api/user/login`(可含 2FA)→ api_server 校验账密(bcrypt)/验证码 → 写签名会话 Cookie(HttpOnly、SameSite=Strict、30 天)→ 返回用户信息。后续管理面请求:中间件解析会话 Cookie(无则回落 access token 头)→ 校验 `New-Api-User` 头与用户一致(防串号)→ 校验用户状态与角色闸门 → 注入用户上下文。吊销:登出/改密/禁用 → Valkey 会话吊销列表(jti 黑名单,TTL=剩余有效期)。

### 8.2 SEQ-002:中继面令牌认证与渠道分发

AI 客户端携带 `Authorization: Bearer sk-xxx`(或 `x-api-key` / `?key=` / WS 子协议)→ TokenAuth:Valkey 读令牌缓存(未命中回源 openGauss)→ 校验状态/过期/额度/IP 白名单/分组可用性 → 模型请求限流(用户维度) → Distribute:解析目标模型与 relay 模式 → 亲和性缓存命中? → 否则 channel_selector 按 (group, model, retry=0) 选渠(优先级档+加权随机)→ 注入渠道上下文(含 multi-key 选 key)→ 进入 relay_entry。

### 8.3 SEQ-003:文本中继计费全流程(预扣 → 调用 → 结算)

relay_entry 校验请求 → 敏感词检查 → token 预估(tiktoken-rs)→ 计算 PriceData(模型倍率/价格、分组倍率、缓存倍率)→ billing 预扣(信任旁路判断 → 扣令牌额度 → 扣资金源:钱包原子扣减 / 订阅 FOR UPDATE + request_id 幂等,失败回滚令牌)→ adaptor 转换请求并发往 upstream_llm → 成功:按实际 usage 精确结算(rust_decimal,差额补扣/退还)→ 写消费日志;失败:退款(异步幂等)→ 视错误判定重试/禁用渠道(SEQ-005)。

### 8.4 SEQ-004:流式 SSE 转发与 usage 注入

adaptor 收到上游 SSE → stream_pipe 行扫描(`data:` 行)→ 延迟一拍转发(保留末行判 usage)→ 按入口格式改写(OpenAI 原样 / 转 Claude / 转 Gemini)→ 上游无 usage 而用户要求 `include_usage` 时注入估算 usage chunk → `[DONE]`。看门狗:空闲超时、ping 保活(10s)、客户端断开检测;首字节时间/流状态记入日志。

### 8.5 SEQ-005:渠道重试与自动禁用

上游返回错误 → RelayErrorHandler 归一化错误(状态码映射可配)→ shouldRetry 判定(渠道类错误/网络错误/可重试状态码;400/408/504/524 不重试)→ 重试循环内重新选渠(下一优先级档、multi-key 换 key、auto 分组可跨组)→ 同时 autoban 判定:命中禁用状态码(默认 401)或关键词 → 单 key 渠道置自动禁用并从缓存摘除;多 key 渠道按 key 粒度禁用 → 通知 root。渠道测试通过(手动/定时)且开启自动恢复时重新启用。

### 8.6 SEQ-006:异步任务提交与轮询结算(视频/音乐)

提交:relay_entry(Task)→ ResolveOriginTask(remix 锁原渠道)→ TaskAdaptor 校验/估费 → **全额预扣**(无信任旁路)→ 上游提交 → 落 tasks 表(public task_id + PrivateData 计费快照)→ 返回任务句柄。轮询:后台 15s 循环 → 超时清扫(CAS 置失败+退款)→ 按平台分组回源 FetchTask → 终态时按 AdjustBillingOnComplete 补差或按 tokens 重算 → 失败 RefundTaskQuota;Gemini/Vertex 任务查询走实时回源。

---

## 9. 部署架构

### 9.1 拓扑

与 `architecture/deployment-architecture.puml` 一致。K8s 同集群部署:

```
namespace: loong-service-sea-weir
├── sea-weir-frontend  Deployment(nginx:alpine, :80)   ← Ingress/HttpRoute(静态资源 + /api、/v1 等反代)
├── sea-weir-backend   Deployment(sea-weir-server, :8080, ≥2 副本,无状态)
├── opengauss          StatefulSet(:5432, PVC)         ← 主库;日志库可独立实例
├── valkey             StatefulSet(:6379, PVC)
└── (平台基础设施) nacos(nacos-svc.nacos:8848)、ingress、CoreDNS
```

前端容器同时承担 `/api/*`、`/v1`、`/v1beta`、`/mj`、`/suno`、`/pg`、`/dashboard` 的反向代理(nginx.conf),浏览器侧同源无跨域;后端 CORS 放开以兼容第三方客户端直连中继面(中继契约要求)。

### 9.2 配置管理

三层配置(与公司微服务一致):**Nacos 下发(生产)> 环境变量覆盖 > 本地 YAML fallback**。

- Nacos:data-id=`sea-weir.yaml`、group=`sea-weir`、namespace=`loong`;连接参数经环境变量 `NACOS_ADDR / NACOS_NAMESPACE / NACOS_GROUP / NACOS_DATA_ID / NACOS_USERNAME / NACOS_PASSWORD`
- 运行期业务配置(系统设置/倍率/分组/支付等)仍在 `options` 表 + 进程内缓存,经管理面 API 热更新,多节点经 Valkey pub/sub 广播失效(替代原 60s 轮询,保留轮询兜底)
- 密钥类(SESSION_SECRET/CRYPTO_SECRET/数据库口令)走 K8s Secret 环境变量,不入 Nacos 明文

### 9.3 端口约定

单协议:后端仅 HTTP :8080(管理面 + 中继面 + 兼容面同端口,按路径分流);前端容器 :80。可选 pprof/metrics 侧口 :8005(仅集群内)。

### 9.4 Dockerfile(多阶段构建)

- 后端:`rust:1.90-slim` 构建(sqlx offline 模式,`SQLX_OFFLINE=true` + `.sqlx/` 缓存;**builder 阶段需 `libssl-dev pkg-config`** —— `webauthn-rs 0.5` 硬依赖 openssl)→ `debian:bookworm-slim` 运行(**需 `libssl3`**),入口 `sea-weir-server -c /etc/sea-weir/config.yaml`
  - 工具链下限 **Rust 1.85**:依赖树中 `clap_lex`、`base64ct` 等已使用 edition 2024
- 前端:`node:20-slim` 构建(`npm run build` 含 `tsc -b`)→ `nginx:stable-alpine` 托管 dist + nginx.conf(SPA fallback + API 反代)
- **CI 出镜像**:`.github/workflows/docker-image.yml` 在推送 `main`、打 `v*` tag 或手动触发时,
  构建 backend / frontend 两个镜像并推送 GHCR(`ghcr.io/<owner>/sea-weir/{backend,frontend}`);
  使用内置 `GITHUB_TOKEN`(`packages: write`),无需 Secret;默认 `linux/amd64`,手动触发可选多架构。
  公司内网仍由 LoongCICD + Harbor 发布正式镜像,GHCR 为 GitHub 侧的镜像通道。

### 9.5 容器资源规划

| 容器 | requests | limits | 依据 |
|---|---|---|---|
| sea-weir-backend | 250m / 256Mi | 2000m / 2Gi | IO 密集 + 长连接;Rust 低常驻内存,上限给突发流式并发与 tiktoken |
| sea-weir-frontend | 50m / 32Mi | 200m / 128Mi | 静态托管 + 反代(注意 nginx 缓冲大响应/流式关闭 proxy_buffering) |
| opengauss | 500m / 1Gi | 2000m / 4Gi | 参照公司 PG 系基线 |
| valkey | 100m / 128Mi | 500m / 1Gi | 缓存 + 限流窗口 |

---

## 10. 错误分类与恢复

### 10.1 错误分类表

| 错误类别 | 分类 | 恢复策略 |
|----------|------|----------|
| 上游调用网络错误/超时 | Transient | 计费退款(幂等)→ 重试循环重新选渠(SEQ-005);重试耗尽返回 OpenAI 兼容错误 |
| 上游业务错误(4xx/5xx) | Permanent(按状态码) | 状态码映射可配;命中禁用规则异步禁用渠道;可重试区间换渠道重试 |
| 额度不足(用户/令牌/订阅) | Recoverable | 中继面 403 文案引导充值;管理面引导钱包页;不做自动恢复 |
| 令牌缺失/非法/过期/IP 越权 | Permanent | 401;中继面 OpenAI 格式错误;管理面跳登录 |
| 角色不足(admin/root 端点) | Permanent | 403 |
| 限流命中(IP/用户维度) | Recoverable | 429 + Retry-After;客户端指数退避 |
| 会话被吊销(登出/改密/禁用) | Permanent | 401,前端清理本地态跳登录 |
| 计费预扣成功但进程崩溃 | Unrecoverable(兜底) | 对账任务:扫描「预扣未结算」记录(订阅预扣有 request_id 台账;钱包预扣写 billing_journal)→ 启动时自动退款 |
| 数据库连接失败 | Transient | sqlx 池自动重连;启动期失败拒绝启动 |
| Valkey 不可用 | Transient(降级) | 用户/令牌缓存回源 DB;限流降级进程内滑动窗口;会话吊销降级 fail-close(拒绝) |
| openGauss 主库只读/切换 | Transient | 写失败按 500 返回,客户端重试;中继面请求退款后报 Transient 错误 |
| 请求体超限/解析失败 | Permanent | 413 / 400(中继面映射 OpenAI invalid_request) |

### 10.2 详细错误映射(错误 → HTTP code / 出口格式)

| AppError / NewApiError | HTTP | 管理面出口 | 中继面出口 |
|---|---|---|---|
| Unauthorized / InvalidToken | 401 | `{success:false,message}` | `{"error":{"type":"authentication_error",…}}` |
| Forbidden / RoleDenied | 403 | 同上 | `{"error":{"type":"permission_error",…}}` |
| RateLimited | 429 | 同上 | `{"error":{"type":"rate_limit_error",…}}` |
| QuotaExceeded | 403 | `{success:false,"余额不足"}` | `{"error":{"type":"insufficient_quota",…}}` |
| ChannelError(`channel:*`) | 502/透传 | — | 归一化后透传或重试 |
| BadRequest | 400 | `{success:false,message}` | `{"error":{"type":"invalid_request_error",…}}` |
| Internal | 500 | `{success:false,"系统错误"}` | `{"error":{"type":"new_api_error",…}}`(附 request id) |

Claude 路径(`RelayFormatClaude`)错误出口为 `{"type":"error","error":{"type","message"}}`;panic 经 panic-hook 中间件兜底为 `new_api_panic` 类型错误。

### 10.3 各层错误传播规则

```
repository: sqlx::Error ──▶ AppError::Database(Transient)
core:       业务规则 ──▶ AppError::Biz(Permanent/Recoverable);计费失败 ──▶ 触发退款状态机
server:     AppError ──▶ 管理面 {success:false} / 中继面按 RelayFormat 格式化
前端:       success===false → antd message.error(message) + Promise.reject
            HTTP 401 → 清理 user store → 跳 /login;HTTP 非 2xx → 状态码文案表
```

---

## 11. 未来演进

1. **对账与账务审计增强**:billing_journal 全量台账 + 定时对账报表(本期仅崩溃兜底扫描)
2. **日志库独立存储引擎**:logs 表迁 ClickHouse(原代码留有占位),看板聚合走 OLAP
3. **渠道健康主动探测**:按模型维度定时探测 + 成功率熔断(现为失败触发 + 测试恢复)
4. **计费规则引擎化**:tiered_expr 表达式计费扩展为可视化规则编排
5. **多 Region 部署**:Valkey 会话/缓存跨区复制,中继面就近接入
6. **可观测性**:OpenTelemetry 导出(tracing 已埋点)、RED 指标与渠道维度 Prometheus 指标
7. **Electron 桌面端与 SDK**:待 Web 版稳定后评估
8. **gRPC 内部管理面**:如后续纳入公司服务网格,可增加 tonic 管理面(契约已按域隔离,改动局限在 server 层)

---

## 文档历史

| 版本 | 日期 | 作者 | 变更 |
|------|------|------|------|
| v1.0 | 2026-09-10 | Architecture Team | 基于 new-api(Go)调研初始编写,目标架构 React + Rust + openGauss |
| v1.1 | 2026-09-10 | Architecture Team | 修复与 new-api 的契约偏差:DTO 字段改 snake_case(§4.2/§6.1)、表数量统一 26(§7)、适配器口径 35+10、端点清单改指 CONTRACTS 附录 A(§6.2)、前端页面域补齐至 24(§1.3/§3.3/§5.1)、i18n 7 种语言、Out of Scope 增列 11 条占位端点与 6 条停用路由、更正 §1.1/§2.4 对 Go 版计费现状的描述 |
