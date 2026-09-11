# syntax=docker/dockerfile:1
#
# sea-weir 前端镜像(多阶段构建)。
# 与 doc/system-design.md §9.4、architecture/deployment-architecture.puml 一致。
#
# 构建:
#   docker build -f code/frontend/Dockerfile -t sea-weir-frontend:local code/frontend
#
# 说明:该容器同时承担静态资源托管与 API 反向代理(nginx.conf),
# 浏览器侧与后端同源,无跨域;中继面(SSE/WebSocket)在 nginx.conf 中已关闭缓冲。

# ───────────────────────────── 构建阶段 ─────────────────────────────
FROM node:20-slim AS builder

WORKDIR /app

# 依赖层缓存:先装依赖,再复制源码。
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund

COPY . .
# `npm run build` = tsc -b(严格类型门禁)+ vite build。
RUN npm run build

# ───────────────────────────── 运行阶段 ─────────────────────────────
FROM nginx:stable-alpine AS runtime

# server{} 配置块放入 conf.d,覆盖默认站点。
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80
