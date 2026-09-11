#!/usr/bin/env bash
# dev 环境策略:审查 + L1 必跑;L2/L3 依赖不可达时告警跳过;L4 跳过。
set -uo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

log "环境:dev(依赖不可达时降级跳过)"
export STRICT=0
resolve_deps

rc=0
stage_review      || rc=1
stage_unit        || rc=1
stage_integration || rc=1
stage_contract    || rc=1
stage_perf
stage_coverage

summary || rc=1
exit $rc
