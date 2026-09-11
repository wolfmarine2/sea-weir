# 12. 部署后冒烟测试用例

**版本**:v1.0 | **层**:L4(需已部署实例)
**测试代码**:`test/src/live_smoke.rs`
**环境变量**:`SEAWEIR_HTTP_BASE_URL`

## 定位

针对**已部署实例**的黑盒探测。prod 环境只跑这一层(加审查与单元)。
目标是发现"代码没问题但部署有问题"的一类故障:配置注入、反代规则、探针路径。

**不做破坏性操作,不写业务数据。**

## 用例清单

| 编号 | 名称 | 优先级 | 发现的问题类型 |
|---|---|---|---|
| TC-E2E-SMK-001-POS | /api/status 可达 | P0 | 配置/数据库连接 |
| TC-E2E-SMK-002-POS | 受保护端点 401 ★ | P0 | 中间件漏挂 |
| TC-E2E-SMK-003-POS | 中继面无令牌 401 | P0 | 同上 |
| TC-E2E-SMK-004-POS | pricing 匿名可访问 | P1 | 闸门多挂 |
| TC-E2E-SMK-005-POS | SPA fallback | P0 | nginx try_files |
| TC-E2E-SMK-006-POS | SSE 未被缓冲 ★ | P0 | nginx proxy_buffering |
| TC-E2E-SMK-007-POS | 响应含 request-id | P1 | 可观测性 |
| TC-E2E-SMK-008-POS | 探针端点不要求鉴权 | P0 | K8s 探针失败 |

## 关键用例详述

### TC-E2E-SMK-002-POS:受保护端点必须 401 ★

```yaml
背景: >
  反向验证。若这里返回 200,说明鉴权中间件在部署形态下没生效 ——
  可能是 layer 嵌套写错,也可能是某个环境变量让鉴权被跳过。
  这是最严重的线上问题,必须在冒烟阶段拦住。

request: "GET /api/user/self(不带任何凭证)"
expected: 401
```

### TC-E2E-SMK-006-POS:SSE 未被缓冲 ★

```yaml
背景: >
  部署配置最常见的坑。nginx 默认开启 proxy_buffering,会把流式响应
  攒够缓冲区才下发。表现为"用户点了发送后等很久,然后内容一次性刷出来" ——
  功能上没错,体验上完全破坏了流式的意义。
  代码测试完全测不到,只能在真实部署上验。

方法:
  1. 用有效令牌发起 stream=true 请求
  2. 记录首字节到达时间(TTFB)
  3. 记录相邻 chunk 的到达间隔
expected:
  - "TTFB < 5s"
  - "chunk 是逐步到达的,不是一次性全部到达"

对应配置: "nginx.conf 中 SSE 路径的 proxy_buffering off"
```

### TC-E2E-SMK-005-POS:SPA fallback

```yaml
背景: >
  前端用 BrowserRouter,深链接(如 /console/token)直接访问或刷新页面时,
  请求会打到 nginx。没配 try_files 就会 404,用户看到白屏。
  开发环境用 vite dev server 不会遇到,只在生产暴露。

request: "GET /console/token"
expected:
  status: 200
  content_type: "含 text/html"
```
