# sea-weir 前端

React 18 + TypeScript(strict)+ antd 5 + zustand + Vite。
架构决策见 [ADR-009](../../doc/architecture/adr/ADR-009-frontend-architecture.md),
组件划分见 [C4 L3](../../doc/architecture/c4/c4-l3-component-web-frontend.puml)。

## 当前状态

**骨架阶段** —— 目录结构、类型契约、模块边界已就位,业务实现待 TDD 阶段填充。
所有待实现处标记为 `TODO(TDD)`,并附带该模块的测试要点清单。

## 目录

| 路径 | 职责 | C4 组件 |
|---|---|---|
| `src/App.tsx`、`src/layouts/` | 主题、路由守卫、布局 | `app_shell` |
| `src/api/` | axios 封装 + 16 个域 API 模块 | `api_layer` |
| `src/stores/` | zustand ×3(user / status / theme) | `stores` |
| `src/components/table/` | 域表格五件套 | `table_kit` |
| `src/pages/` | 28 个页面 | `console_pages` / `admin_pages` / … |
| `src/hooks/` | 按域数据 hooks | — |
| `src/types/` | 与后端逐字段对应的类型 | — |
| `src/i18n/` | 7 种语言 | — |

## 三条不可违反的契约

1. **字段命名**:跨越 API 边界的类型一律 `snake_case`。前端内部状态可用 camelCase,
   但请求体与响应体不行 —— 后端与 new-api 都是 snake_case。
2. **分页页码 1 起**:请求参数 `p` + `page_size`,响应 `{items,total,page,page_size}`。
3. **业务错误是 HTTP 200**:管理面失败响应是 `200 + {success:false}`,
   必须在 axios 响应拦截器里转成 reject,否则会把失败当成功。

## 开发

```bash
npm install
npm run dev        # :5173,已配好到 :8080 的代理
npm run typecheck  # tsc -b,零错误门禁
npm run test       # vitest
npm run build      # tsc -b && vite build
```

## TDD 建议顺序

1. `utils/quota.ts` —— 纯函数,先跑通测试链路
2. `api/client.ts` —— 用 msw 覆盖三条契约(尤其 success:false → reject)
3. `stores/` —— 状态迁移与持久化范围断言
4. `components/table/` + `hooks/useTableData` —— 11 个域的公共地基,收益最高
5. 各页面 —— 从令牌页与日志页两个样板做起(ADR-009 实施计划第 2 步)

## 契约测试

用从 new-api 录制的真实响应做 fixture,使前后端字段偏差在测试阶段暴露,
而不是等到联调。见 [审查报告](../../doc/architecture/adr-review-report.md) 后续检查项 1。

> 当前 `api/client.test.ts` 用**注入 axios adapter** 打桩(不依赖网络、无需启动 msw);
> `src/contract/baseline.test.ts` 直接读取 `test/cases/fixtures/baseline/*.json`
> 校验 snake_case 与分页形状。msw 预留给后续页面级集成测试。

## 构建镜像

```bash
docker build -f code/frontend/Dockerfile -t sea-weir-frontend:local code/frontend
```

- 构建阶段 `node:20-slim`(`npm ci` + `npm run build`,含 `tsc -b` 严格类型门禁)。
- 运行阶段 `nginx:stable-alpine`,托管 `dist/` 并按 `nginx.conf` 反代 `/api`、`/v1` 等;
  中继面已关闭 `proxy_buffering` 以支持 SSE,并开启 WebSocket 升级。

### 提交后自动出镜像(GitHub Actions)

与后端同一个工作流 `.github/workflows/docker-image.yml`,推送 `main` / 打 `v*` tag /
手动触发时构建并推送:

```bash
docker pull ghcr.io/<owner>/sea-weir/frontend:latest   # 默认分支
docker pull ghcr.io/<owner>/sea-weir/frontend:v1.2.3   # 打 tag 时
docker pull ghcr.io/<owner>/sea-weir/frontend:sha-abc1234
```

无需配置 Secret(使用内置 `GITHUB_TOKEN`);默认 `linux/amd64`,手动触发可选多架构。


