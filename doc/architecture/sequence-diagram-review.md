# Sequence Diagram 完整性与一致性审查报告

**审查日期**: 2026-09-10(第 1 轮)/ 2026-09-10(第 2 轮,含对 new-api 源码的逐项复核)
**审查员**: Architecture Team
**审查材料**: `sequence-diagram/SEQ-*.puml`(6 份)+ C4 L2 容器图 + CONTRACTS.md + new-api 源码交叉引用(`/home/zhangzc/project/new-api`)
**项目**: sea-weir(AI 网关与大模型 API 管理中台)

> **第 2 轮说明**:第 1 轮结论为「0 问题、全部 PASS」。第 2 轮补做源码级复核后,时序图本身的参与者与调用方向仍然成立,但发现 **2 项覆盖缺口**(均已修复),且 C4 图文件扩展名问题影响 B1 的引用有效性。

---

## 检查结果(第 2 轮)

### A. 覆盖完整性

| 项 | 结论 | 证据 |
|---|---|---|
| A1: 每个核心业务流程均有对应 SEQ 时序图 | PASS | 核心流程覆盖:登录/管理面认证(SEQ-001)、令牌认证与分发(SEQ-002)、中继计费(SEQ-003)、流式转发(SEQ-004)、重试/禁用(SEQ-005)、异步任务(SEQ-006)。管理面 CRUD 类流程为通用模式,不单独出图(模板约定「核心业务流程」) |
| A2: system-design.md §时序设计章节与 puml 文件一一对应 | PASS | §8.1~§8.6 ↔ SEQ-001~006,文件名与标题一致 |
| A3(新增): 时序图覆盖的端点集合是 CONTRACTS 附录 A 的真子集,且无「图里有、契约没有」的端点 | PASS(修复后) | 附录 A 建立后完成反向核对 |

### B. 参与者一致性

| 项 | 结论 | 证据 |
|---|---|---|
| B1: 所有参与者名称来自 C4 L2 容器定义,无凭空参与者 | PASS(引用已更正) | 使用的参与者:web_frontend、api_server、admin_engine、relay_engine、repository_layer、opengauss_db、redis_cache、upstream_llm,全部为 `c4-l2-container.puml` 定义的容器名;actor(终端用户/运营管理员/AI 客户端)为 L2 中的 Person。**第 1 轮引用的文件名 `c4-l2-container.dsl` 已随扩展名修正更新** |
| B2: 参与者调用方向与模块依赖图一致(不出现反向依赖) | PASS | api_server→admin_engine/relay_engine、relay_engine→repository_layer、repository_layer→opengauss_db/redis_cache,与 module-dependency.puml 方向一致;无 repository→core 反向调用 |

### C. 契约一致性

| 项 | 结论 | 证据 |
|---|---|---|
| C1: 时序图中的方法调用与 CONTRACTS.md 中的签名一致 | PASS | `find_by_key(HMAC)`/`pre_consume(request_id)`/`settle`/`refund`/`cas_status`/`update_status`/`record_error_log` 等与 CONTRACTS.md §12 Repository Trait、§11 Adaptor 契约一致 |
| C2: 错误分支与错误码表一致 | PASS | insufficient_quota(403)/429 rate_limit_exceeded/OpenAI 错误格式/退款语义与 CONTRACTS.md §13 错误码表、system-design.md §10 一致 |
| C3: 与 new-api 行为兼容 | PASS | 见下方源码复核明细 |

---

## 源码复核明细

### 复核通过

| 图 | 声明 | new-api 证据 |
|---|---|---|
| SEQ-002 | 缓存 TTL 60s | `common/init.go:102`(`SYNC_FREQUENCY` 默认 60) |
| SEQ-002 | `sk-xxx-{channelId}` 仅 admin/root | `middleware/auth.go:429-437` |
| SEQ-003 | 按次计费全额预扣(ForcePreConsume);信任旁路 | `service/billing_session.go:152,282` |
| SEQ-004 | ping 保活 10s、延迟一拍转发判 usage | `relay/helper/stream_scanner.go:27,65-76` |
| SEQ-005 | 重试排除 400/408/504/524;禁用默认 401 | `setting/operation_setting/status_code_ranges.go:17-33`;`controller/relay.go:319-349` |
| SEQ-005 | 亲和性失败跳过重试、`specific_channel_id` 不重试 | `controller/relay.go:323,335` |
| SEQ-006 | 任务轮询 15s;终态 CAS | `service/task_polling.go:93`;`model/task_cas_test.go` |

### 第 2 轮发现并修复的问题(2 项)

| # | 级别 | 问题 | 修复 |
|---|---|---|---|
| 1 | P2 | SEQ-006 与 §8.6 覆盖异步任务提交/轮询,但 CONTRACTS §10 的中继端点表遗漏了 MJ 的 `submit/shorten`、`simple-change`、`edits`、`video`、`upload-discord-images`、`task/:id/image-seed`、`task/list-by-condition`,以及 `/mj` 与 `/:mode/mj` 双前缀注册这一事实 —— 时序图描述的「MJ 全系」在契约侧没有落位 | CONTRACTS §10 补齐 MJ 全部 16 条并标注双前缀;附录 A.2.5 给出完整清单 |
| 2 | P2 | §1.3 声明中继面含 Midjourney `notify`,但 new-api `router/relay-router.go:217` 中 `/mj/notify` 已被注释停用,SEQ-006 也未画该回调分支 —— 属文档多写 | §1.3 与 CONTRACTS §10 均改为明确「`/mj/notify` 已停用,不迁移」,与 SEQ-006 保持一致 |

---

## 迭代收敛状态

| 轮次 | 日期 | 审查范围 | 发现问题 | 已修复 | 状态 |
|------|------|----------|----------|--------|------|
| 1 | 2026-09-10 | 图内部 + 契约交叉核对 | 0 | 0 | 结论不可靠(未做端点集合反向核对) |
| 2 | 2026-09-10 | 追加「图↔附录 A 端点集合」反向核对 + 源码复核 | 2(P2×2) | 2 | 全部 PASS |

---

## 附录:时序图场景覆盖矩阵

| 业务流程 | 时序图 | 正常路径 | 错误路径 | 覆盖状态 |
|----------|--------|----------|----------|----------|
| 用户登录与管理面认证 | SEQ-001 | ✔ | ✔(密码错/2FA/吊销/角色不足) | 完整 |
| 令牌认证与渠道分发 | SEQ-002 | ✔ | ✔(令牌非法/限流/亲和性未命中) | 完整 |
| 文本中继计费全流程 | SEQ-003 | ✔ | ✔(余额不足/上游失败退款) | 完整 |
| 流式 SSE 转发 | SEQ-004 | ✔ | ✔(空闲超时/usage 缺失注入) | 完整 |
| 渠道重试与自动禁用 | SEQ-005 | ✔ | ✔(可重试/不可重试/重试耗尽/恢复) | 完整 |
| 异步任务提交与轮询 | SEQ-006 | ✔ | ✔(超时清扫/失败退款/CAS 冲突) | 完整 |

## 附录:参与者使用频率统计

| 参与者(C4 L2 容器) | 出现的时序图 | 次数 |
|----------------------|--------------|------|
| api_server | SEQ-001/002/003/004/005/006 | 6 |
| relay_engine | SEQ-002/003/004/005/006 | 5 |
| repository_layer | SEQ-001/002/003/005/006 | 5 |
| opengauss_db | SEQ-001/002/003/006 | 4 |
| upstream_llm | SEQ-003/004/005/006 | 4 |
| redis_cache | SEQ-001/002/005 | 3 |
| web_frontend | SEQ-001 | 1 |
| admin_engine | SEQ-001 | 1 |

## 后续检查项(实施阶段)

1. WebSocket realtime(`/v1/realtime`)增量扣费流程未单独出图,SEQ-004 仅覆盖 SSE —— 若实现时分歧较大,补 SEQ-007
2. 支付/订阅回调的幂等状态机(CONTRACTS §7)目前只有文字契约,建议补 SEQ-008

---

## 文档历史

| 版本 | 日期 | 作者 | 变更 |
|------|------|------|------|
| v1.0 | 2026-09-10 | Architecture Team | 首轮审查,全部 PASS |
| v2.0 | 2026-09-10 | Architecture Team | 补做端点集合反向核对与源码复核;发现 2 项覆盖缺口并修复;更正 C4 图文件名引用;推翻 v1.0「0 问题」结论 |
