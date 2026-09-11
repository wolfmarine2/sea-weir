#!/usr/bin/env bash
# prod 环境策略:部署后冒烟。审查 + L1 + HTTP 连通性;
# 服务端性能基准默认禁用(需 PROD_ALLOW_LOAD=1 显式授权)。
set -uo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

log "环境:prod(部署后冒烟,不做破坏性操作)"
export STRICT=0
resolve_deps

if [[ -z "${SEAWEIR_HTTP_BASE_URL:-}" ]]; then
  fail "prod 冒烟必须提供 SEAWEIR_HTTP_BASE_URL"
  exit 1
fi

rc=0
stage_review || rc=1
stage_unit   || rc=1
stage_smoke  || rc=1

if [[ "${PROD_ALLOW_LOAD:-0}" == "1" ]]; then
  warn "PROD_ALLOW_LOAD=1 —— 在生产执行性能基准"
  stage_perf
else
  record "性能基线" SKIP "生产默认禁用(需 PROD_ALLOW_LOAD=1)"
fi

# prod 不跑 L2(会写数据)、不跑 L3(需要 new-api 实例)
record "L2 集成" SKIP "prod 不执行(避免写入生产数据)"
record "L3 契约" SKIP "prod 不执行(需 new-api 实例)"

summary || rc=1
exit $rc
