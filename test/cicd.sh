#!/usr/bin/env bash
# CD test 阶段统一入口。按 ENV 分发到 env/<env>.sh。
# 与 LoongCICD 调用约定一致(参照 service-auth/test/cicd.sh)。
#
#   NAMESPACE=loong-service-sea-weir ENV=dev  bash test/cicd.sh
#   NAMESPACE=loong-service-sea-weir ENV=test bash test/cicd.sh
#   NAMESPACE=loong-service-sea-weir ENV=prod bash test/cicd.sh
set -uo pipefail

ENV="${ENV:-dev}"
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

case "$ENV" in
  dev|test|prod) ;;
  *) echo "[fail] 未知 ENV=$ENV,应为 dev / test / prod"; exit 2 ;;
esac

echo "[sea-weir-test] ENV=$ENV NAMESPACE=${NAMESPACE:-<未设置>}"
exec bash "$DIR/env/$ENV.sh"
