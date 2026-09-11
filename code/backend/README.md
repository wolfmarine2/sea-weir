# sea-weir 后端

Rust + axum 0.8 + tokio + sqlx(openGauss)+ Valkey。
架构见 [system-design.md](../../doc/system-design.md),
契约见 [CONTRACTS.md](../../doc/architecture/CONTRACTS.md)。

## 当前状态

**骨架阶段** —— workspace、crate 边界、Trait 契约、模块划分已就位;
业务实现为 `todo!()`,每个模块附带该模块的测试要点清单,供 TDD 阶段逐条转成测试。

## Workspace

```
sea-weir-server  ─→ sea-weir-core ─→ sea-weir-repository ─→ sea-weir-types
       │                  └────────→ sea-weir-adaptors ───────────┘
       └──────────────────────────────────────────────────────────┘
```

| Crate | 职责 | 关键边界 |
|---|---|---|
| `sea-weir-types` | 领域模型 / DTO / AppError / 常量 / 配置 | **不依赖任何内部 crate** |
| `sea-weir-repository` | Repository Trait + sqlx 实现 + Valkey | 不依赖 core / adaptors |
| `sea-weir-adaptors` | Adaptor / TaskAdaptor,35 同步 + 10 任务 | **不依赖 repository**(渠道数据由 core 注入) |
| `sea-weir-core` | admin(16 域)+ relay(中继引擎 8 组件) | 不接触 HTTP 类型 |
| `sea-weir-server` | axum 装配、4 路由面、中间件、handlers | — |

这些边界不是形式主义:它们决定了各层能否脱离外部依赖做单测。
`adaptors` 不碰数据库 → 可用 wiremock 单测;`core` 只吃 Trait → 可用 mockall 单测;
`handlers` 不含业务 → 业务测试不需要构造 HTTP 请求。

## 四条不可违反的契约

1. **字段命名**:管理面 DTO 一律 `snake_case`(serde `rename_all`)。
2. **分页页码 1 起**:`start_idx = (page-1)*page_size`,`p < 1` 归一为 1。
3. **业务错误是 HTTP 200**:仅鉴权(401/403)与限流(429)用真实状态码。
4. **额度扣减必须带守卫**:`... AND quota >= $1`,影响 0 行 = 余额不足。
   这是相对 new-api 的实质改进(Go 版是无条件的 `quota - ?`)。

## 构建前置

| 依赖 | 用途 | 安装 |
|---|---|---|
| Rust ≥ 1.85 | 依赖树中多个 crate 已用 edition 2024 | `rustup toolchain install 1.90` |
| **libssl-dev** | `webauthn-rs 0.5` 硬依赖 openssl(Passkey 功能) | `apt install libssl-dev pkg-config` |

`webauthn-rs` 是全栈唯一的 openssl 依赖 —— 其余 TLS 路径(reqwest / sqlx)均已锁定
rustls。这意味着 Dockerfile 的 **builder 阶段需 `libssl-dev`,runtime 阶段需 `libssl3`**,
不能用纯静态镜像。若后续要消除该依赖,需替换 Passkey 库或将其拆为可选 feature。

不装 libssl-dev 时,其余四个 crate 仍可独立检查:

```bash
cargo check -p sea-weir-types -p sea-weir-repository -p sea-weir-adaptors -p sea-weir-core
```

## 开发

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p sea-weir-server -- -c config/config.example.yaml
```

sqlx 编译期校验需要 `.sqlx/` 离线缓存或可达数据库:

```bash
cargo sqlx prepare --workspace
```

## TDD 建议顺序

按「正确性风险 × 依赖深度」排:

1. **`sea-weir-types`** —— 常量与错误映射,纯函数,锁死契约取值
2. **`repository::traits`** —— 额度扣减、兑换、订阅预扣的并发正确性(testcontainers 起 openGauss)
3. **`core::relay::billing`** —— 计费三段式与幂等,本项目正确性风险最高的模块
4. **`core::relay::select` / `autoban`** —— 纯函数 + 表驱动,覆盖成本低
5. **`adaptors::sync::common`** —— 一次覆盖 35 个适配器的共性逻辑
6. **`core::relay::convert` / `stream`** —— 用录制的真实上游响应做回放测试
7. **`core::relay::pipeline`** —— mock repository + wiremock 上游,验证账目守恒
8. **`server::router`** —— 路由数与附录 A 对表,角色闸门遍历断言
9. 各 `adaptors::sync::*` 与 `admin::*` 域 —— 按使用频度铺开

## 端点清单

完整 305 条见 [CONTRACTS.md 附录 A](../../doc/architecture/CONTRACTS.md)。
建议把「注册路由数 == 附录 A 条数」做成 CI 断言,防漏防重。

## 构建镜像

```bash
# 上下文为 code/backend
docker build -f code/backend/Dockerfile -t sea-weir-backend:local code/backend
```

- 构建阶段 `rust:1.90-slim-bookworm` + `libssl-dev pkg-config`(webauthn-rs 经
  webauthn-attestation-ca 硬依赖 openssl);运行阶段 `debian:bookworm-slim` + `libssl3`。
- 依赖层缓存:先只复制各 crate 的 `Cargo.toml` 与占位源码编译一次,再复制真实源码重建。
- 入口 `sea-weir-server -c /etc/sea-weir/config.yaml`;默认配置取自
  `config/config.example.yaml`,生产由 Nacos / ConfigMap 覆盖,镜像内不写密钥。
- 端口 `8080`(HTTP,管理面+中继面)与 `8005`(可选 metrics/pprof)。K8s 探针建议
  `httpGet /api/status`。
- 本机若缺少系统 openssl 开发包,可在构建机或 CI 镜像安装 `libssl-dev` 后完整构建。

### 提交后自动出镜像(GitHub Actions)

`.github/workflows/docker-image.yml` 在推送到 `main`、打 `v*` tag 或手动触发时,
自动构建后端与前端两个镜像并推送到 GitHub Container Registry:

```bash
docker pull ghcr.io/<owner>/sea-weir/backend:latest   # 默认分支
docker pull ghcr.io/<owner>/sea-weir/backend:main
docker pull ghcr.io/<owner>/sea-weir/backend:v1.2.3   # 打 tag 时
docker pull ghcr.io/<owner>/sea-weir/backend:sha-abc1234
```

- 使用内置 `GITHUB_TOKEN`(`packages: write`),**无需配置任何 Secret**;
- 默认仅构建 `linux/amd64`;手动触发时可把 `platforms` 填为
  `linux/amd64,linux/arm64`(arm64 走 QEMU,耗时显著增加);
- GHCR 包默认私有;需公开时在 Package 设置里改为 Public,或为拉取方签发 token。


