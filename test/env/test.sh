#!/usr/bin/env bash
# test 环境策略:全量门禁。缺连接参数即失败,杜绝"跳过依赖套件"的假绿。
set -uo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

log "环境:test(全量门禁,STRICT=1)"
export STRICT=1
resolve_deps

# 前置检查:必填参数缺一即失败
missing=()
[[ -z "${DATABASE_URL:-}" ]]          && missing+=("DATABASE_URL")
[[ -z "${REDIS_URL:-}" ]]             && missing+=("REDIS_URL")
[[ -z "${SEAWEIR_HTTP_BASE_URL:-}" ]] && missing+=("SEAWEIR_HTTP_BASE_URL")
if [[ ${#missing[@]} -gt 0 ]]; then
  fail "test 环境缺少必填连接参数:${missing[*]}"
  fail "拒绝以跳过依赖套件的方式通过 —— 那会产生假绿"
  exit 1
fi

rc=0
stage_review      || rc=1
stage_unit        || rc=1
stage_integration || rc=1
stage_contract    || rc=1
stage_smoke       || rc=1
stage_perf
stage_coverage    || rc=1

summary || rc=1
exit $rc
