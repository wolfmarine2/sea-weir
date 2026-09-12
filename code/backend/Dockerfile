# syntax=docker/dockerfile:1
#
# sea-weir 后端镜像(多阶段构建)。
# 与 doc/system-design.md §9.4、architecture/deployment-architecture.puml 一致。
#
# 构建:
#   docker build -f code/backend/Dockerfile -t sea-weir-backend:local code/backend
#   docker build -f code/backend/Dockerfile -t sea-weir-backend:local .   # 仓库根为上下文时
#
# 运行(K8s 由 ConfigMap/Secret 注入配置;本地调试可挂载 config.yaml):
#   docker run --rm -p 8080:8080 -e RUST_LOG=info \
#     -v $PWD/config.yaml:/etc/sea-weir/config.yaml:ro sea-weir-backend:local

# ───────────────────────────── 构建阶段 ─────────────────────────────
# 工具链下限 Rust 1.85(依赖树中 clap_lex / base64ct 等使用 edition 2024)。
FROM rust:1.90-slim-bookworm AS builder

# webauthn-rs 0.5 经 webauthn-attestation-ca 硬依赖 openssl,构建期需要开发头文件。
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# 依赖层缓存:先只复制清单文件,用占位源码把第三方依赖编译一次。
# 之后复制真实源码重建时,第三方依赖层可命中缓存。
COPY Cargo.toml Cargo.lock ./
COPY crates/sea-weir-types/Cargo.toml       crates/sea-weir-types/
COPY crates/sea-weir-repository/Cargo.toml  crates/sea-weir-repository/
COPY crates/sea-weir-adaptors/Cargo.toml    crates/sea-weir-adaptors/
COPY crates/sea-weir-core/Cargo.toml        crates/sea-weir-core/
COPY crates/sea-weir-server/Cargo.toml      crates/sea-weir-server/

RUN set -eux; \
    for crate in sea-weir-types sea-weir-repository sea-weir-adaptors sea-weir-core sea-weir-server; do \
        mkdir -p "crates/${crate}/src"; \
        echo '// placeholder for dependency caching' > "crates/${crate}/src/lib.rs"; \
    done; \
    echo 'fn main() {}' > crates/sea-weir-server/src/main.rs; \
    cargo build --release --locked -p sea-weir-server

# 真实源码。注意:sqlx 目前未使用 query! 编译期宏,故无需 .sqlx/ 与 SQLX_OFFLINE。
# 引入编译期 SQL 校验后,在此前增加 `COPY .sqlx /build/.sqlx` 并设置 SQLX_OFFLINE=true。
COPY crates ./crates
COPY migrations ./migrations
# 配置模板:`sea-weir-server/src/config.rs` 用 include_str! 内嵌它做缺省兜底,
# 编译期必须存在,否则报 "couldn't read .../config/config.example.yaml"。
COPY config ./config

# 刷新 mtime:占位源码先编译过,若 COPY 进来的源码时间戳更早,Cargo 会跳过重编译,
# 导致镜像里是占位二进制。
RUN find crates -name '*.rs' -exec touch {} + \
    && cargo build --release --locked -p sea-weir-server

# ───────────────────────────── 运行阶段 ─────────────────────────────
FROM debian:bookworm-slim AS runtime

# 运行期只需 libssl3(openssl-sys 动态链接)与根证书。
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -g 10001 seaweir \
    && useradd -u 10001 -g seaweir -M -s /usr/sbin/nologin seaweir

WORKDIR /app

COPY --from=builder /build/target/release/sea-weir-server /usr/local/bin/sea-weir-server
# sqlx migrations 随镜像分发(启动时由 repository 层执行)。
COPY --from=builder /build/migrations ./migrations
# 默认配置;生产由 Nacos/ConfigMap 覆盖,勿在镜像内写密钥。
COPY config/config.example.yaml /etc/sea-weir/config.yaml

RUN chown -R seaweir:seaweir /app /etc/sea-weir
USER seaweir

# 8080 HTTP(管理面 + 中继面);8005 可选 pprof/metrics(仅集群内)。
EXPOSE 8080 8005

# K8s 探针建议:httpGet /api/status(公开端点,无需鉴权)。
ENTRYPOINT ["/usr/local/bin/sea-weir-server"]
CMD ["-c", "/etc/sea-weir/config.yaml"]
