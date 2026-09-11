# ADR-004: 双 API 面与认证模型

## Metadata

| 项 | 内容 |
|----|------|
| 编号 | ADR-004 |
| 标题 | 双 API 面与认证模型(管理面会话 / 中继面 sk-token) |
| 状态 | Proposed |
| 日期 | 2026-09-10 |
| 决策者 | Architecture Team |
| 影响范围 | `sea-weir-server/middleware`、`handlers`、前端 api 层 |
| 依赖 | ADR-001, ADR-002 |

## Status History

| 日期 | 状态 | 说明 |
|------|------|------|
| 2026-09-10 | Proposed | 初始提议 |

## Context(上下文)

### 问题陈述

系统同时服务两类调用方:浏览器控制台(管理面,人机交互、Cookie 会话)与 AI 客户端(中继面,机器调用、sk-token)。两类流量的认证方式、限流维度、错误格式、CORS 策略完全不同,需要明确划分与各自的认证契约。

### 约束条件

- 契约兼容 new-api:管理面 session cookie + access token + `New-Api-User` 头;中继面 sk-token 多源提取(Bearer / x-api-key / ?key= / WS 子协议 / mj-api-secret)
- 多节点部署:会话不能绑定单节点内存
- 角色四级:guest 0 / common 1 / admin 10 / root 100

### 驱动力

- 安全:防串号(New-Api-User)、会话可吊销(登出/改密/禁用即时生效)
- 性能:中继面认证走缓存,不能每请求查库
- 兼容:现有客户端(SDK、one-api 生态工具)零改动

### 目标状态

两条中间件链独立;会话吊销 ≤1s 生效;中继面令牌认证 P99 < 1ms(缓存命中)。

### 备选方案

#### 方案 A:同进程同端口,按路径分流两条中间件链

**描述**: axum 单服务 :8080,`/api` 走会话链,`/v1` 等走 sk-token 链;会话为签名 Cookie(jti)+ Valkey 吊销列表。
**优点**:
- 部署最简单(与 new-api 一致);契约兼容
- 吊销走 Valkey,多节点即时生效
**缺点**:
- 两类流量资源竞争(需限流与性能护栏隔离)

#### 方案 B:拆两个服务(管理服务 + 中继服务)

**描述**: 物理拆分部署。
**优点**:
- 故障域与扩缩容独立
**缺点**:
- 共享代码(用户/令牌/计费)需抽公共 crate;部署复杂度翻倍;与灰度替换策略冲突

#### 方案 C:管理面改 JWT(无吊销列表)

**描述**: 纯 JWT 自校验。
**优点**:
- 无 Valkey 依赖
**缺点**:
- 无法即时吊销(禁用用户/登出不生效),安全不达标

### 决策依据

| 维度 | 权重 | A | B | C |
|---|---|---|---|---|
| 契约兼容 | 高 | ✔ | ✔ | ✘ |
| 即时吊销 | 高 | ✔ | ✔ | ✘ |
| 部署复杂度 | 中 | ✔ | ✘ | ✔ |
| 故障隔离 | 低 | △ | ✔ | △ |

## Decision(决策)

### 选择的方案

在 sea-weir 认证设计中,面对「人机双调用方、即时吊销、契约兼容」的关注点,我们选择 **同进程双 API 面 + 管理面签名会话 Cookie(Valkey 吊销列表)+ 中继面 sk-token 缓存认证**,而非物理拆服务或纯 JWT,以获得部署简单与即时吊销,接受两类流量同进程(以限流与性能护栏隔离)。

### 决策理由

1. 会话 Cookie 自承载(jti)+ Valkey 吊销列表,多节点一致且即时生效
2. sk-token 以 HMAC 为 Valkey 缓存键(明文不落缓存),认证 P99 亚毫秒
3. 与 new-api 契约逐点兼容,客户端零改动,支持灰度替换

### 技术架构

- 管理面链:CORS → gzip → GlobalAPIRateLimit(IP)→ auth(session Cookie → access token 回落;`New-Api-User` 一致性;角色闸门 UserAuth/AdminAuth/RootAuth)
- 中继面链:CORS → 解压 → 统计 → 性能护栏 → TokenAuth(sk 提取/缓存校验/IP 白名单/分组)→ ModelRequestRateLimit(用户维度)→ Distribute(选渠)
- 敏感操作凭证:`POST /api/verify` 换发短时签名凭证,Valkey 一次性消费(查看渠道密钥等)
- 会话吊销场景:登出、改密、用户禁用、角色变更

### 实施计划

1. 实现两条中间件链与四个角色闸门,契约测试对齐 new-api 行为
2. Valkey 吊销列表 + 缓存键设计(`sess:revoke:{jti}`、`token:{hmac}`、`user:{id}`)
3. 安全测试:串号、重放、吊销时效、IP 白名单绕过用例

## Consequences(后果)

### 正面影响

1. 部署形态与 new-api 一致,灰度替换无障碍
2. 吊销即时生效,安全水位提升(原实现 session 无吊销)
3. 中继面认证路径无 DB 直查(缓存命中),高并发下稳定

### 负面影响

1. Valkey 成为认证路径关键依赖(不可用时需降级策略)

### 缓解措施

1. 缓存 miss 回源 DB;Valkey 故障时会话吊销 fail-close(拒绝)、令牌认证回源 DB(ADR-007)

### 长期影响

C4 边界已预留 relay_engine 拆服务的可能;届时认证中间件作为共享 crate 复用。

## Notes

- `sk-xxx-{channelId}` 指定渠道能力仅 admin/root 保留(排障用)
- TokenAuthReadOnly(自查余额/日志)语义保留:只验证存在性与用户未封禁

## References

- `doc/system-design.md` §6.6
- `doc/architecture/CONTRACTS.md` §1
- `doc/architecture/sequence-diagram/SEQ-001、SEQ-002`
- 相关 ADR:ADR-007(缓存)、ADR-008(错误出口)

---

**文档版本**: v1.0
**最后更新**: 2026-09-10
**下次评审**: 2026-12-10(3个月后)
**评审责任人**: Architecture Team Lead
