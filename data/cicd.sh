#!/bin/bash
# =============================================================================
# 文件: data/cicd.sh
# 用途: sea-weir 数据阶段统一入口 —— 按环境分发给 data/env/<env>.sh
#        (与 LoongCICD 流水线 data 阶段的调用约定一致: bash data/cicd.sh)
#
# 环境策略:
#   dev   开发环境:补建缺失表/索引 -> 清空业务表重灌基础种子(options/setups 保留)
#   test  测试环境:无损升级 —— 只补建缺失表/索引、只在缺失时补基础种子,绝不覆盖
#   prod  生产环境:无损升级 —— 只补建缺失表/索引;仅在 users 为空时创建 root,绝不覆盖
#
# 需要的环境变量(由 CD workflow 注入):
#   NAMESPACE  必填,K8s 命名空间(openGauss 服务所在)
#   PROJECT    选填,项目名(默认 sea-weir)
#   ENV        必填,环境名: dev / test / prod(不区分大小写)
#
# 数据库连接参数(唯一来源为 Nacos 的 database.dsn,不接受 DB_* 覆盖):
#   NACOS_ADDR / NACOS_DATA_ID / NACOS_GROUP / NACOS_NAMESPACE 默认见 data/lib/nacos.sh
#   Nacos 账号缺省 loong/loong(与部署 Secret 一致),可用 NACOS_USERNAME/PASSWORD 覆盖。
# =============================================================================
set -euo pipefail

: "${NAMESPACE:?NAMESPACE is required}"
PROJECT="${PROJECT:-sea-weir}"
ENV="${ENV:-dev}"
ENV="$(printf '%s' "$ENV" | tr '[:upper:]' '[:lower:]')"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_SCRIPT="${SCRIPT_DIR}/env/${ENV}.sh"

case "$ENV" in
    dev|test|prod) ;;
    *)
        echo "❌ 不支持的环境: ${ENV} (仅支持 dev/test/prod)"
        echo "   用法: NAMESPACE=loong-service-sea-weir ENV=dev bash data/cicd.sh"
        exit 1
        ;;
esac

[ -f "$ENV_SCRIPT" ] || { echo "❌ 缺少环境脚本: ${ENV_SCRIPT}"; exit 1; }

echo "==> 数据阶段分发: PROJECT=${PROJECT} NAMESPACE=${NAMESPACE} ENV=${ENV} → ${ENV_SCRIPT}"
exec bash "$ENV_SCRIPT"
