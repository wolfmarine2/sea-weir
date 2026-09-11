#!/usr/bin/env bash
# 公共阶段执行器。被 dev.sh / test.sh / prod.sh 引用。
# 参照 service-auth/test/env/common.sh 的编排结构。
set -uo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_DIR="$(cd "$TEST_DIR/.." && pwd)"
BACKEND_DIR="$REPO_DIR/code/backend"
FRONTEND_DIR="$REPO_DIR/code/frontend"

log()  { printf '\033[1;34m[sea-weir-test]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[warn]\033[0m %s\n' "$*"; }
fail() { printf '\033[1;31m[fail]\033[0m %s\n' "$*"; }

# 记录各阶段结果,最后统一汇总(避免第一个失败就中断,丢失全貌)
declare -a STAGE_RESULTS=()
record() { STAGE_RESULTS+=("$1|$2|$3"); }   # 名称|状态|说明

summary() {
  echo
  log "===== 阶段汇总 ====="
  local failed=0
  for r in "${STAGE_RESULTS[@]}"; do
    IFS='|' read -r name status note <<< "$r"
    case "$status" in
      PASS) printf '  \033[1;32m✔\033[0m %-28s %s\n' "$name" "$note" ;;
      SKIP) printf '  \033[1;33m—\033[0m %-28s %s\n' "$name" "$note" ;;
      FAIL) printf '  \033[1;31m✘\033[0m %-28s %s\n' "$name" "$note"; failed=1 ;;
    esac
  done
  echo
  return $failed
}

# ---------------------------------------------------------------- 依赖参数推导
# 优先级:显式环境变量 > Nacos 推导。仓库内不内置任何默认口令。
resolve_deps() {
  : "${NAMESPACE:=loong-service-sea-weir}"

  if [[ -z "${DATABASE_URL:-}" && -n "${NACOS_ADDR:-}" ]]; then
    log "从 Nacos 推导 DATABASE_URL(namespace=$NAMESPACE)"
    # TODO(实施): 与 data 阶段同一配置位置读取 db.host/port/db-name/password
  fi
  if [[ -z "${REDIS_URL:-}" && -n "${NACOS_ADDR:-}" ]]; then
    log "从 Nacos 推导 REDIS_URL"
  fi
  if [[ -z "${SEAWEIR_HTTP_BASE_URL:-}" ]]; then
    export SEAWEIR_HTTP_BASE_URL="http://sea-weir-svc.${NAMESPACE}.svc.cluster.local:8080"
  fi

  log "DATABASE_URL          = ${DATABASE_URL:+<已设置>}${DATABASE_URL:-<未设置>}"
  log "REDIS_URL             = ${REDIS_URL:+<已设置>}${REDIS_URL:-<未设置>}"
  log "SEAWEIR_HTTP_BASE_URL = ${SEAWEIR_HTTP_BASE_URL:-<未设置>}"
}

# ---------------------------------------------------------------- 阶段:代码审查
stage_review() {
  [[ "${SKIP_REVIEW:-0}" == "1" ]] && { record "代码审查" SKIP "SKIP_REVIEW=1"; return 0; }

  local rc=0
  log "cargo fmt --check"
  (cd "$BACKEND_DIR" && cargo fmt --all -- --check) || rc=1

  log "cargo clippy -D warnings"
  (cd "$BACKEND_DIR" && cargo clippy --workspace --all-targets -- -D warnings) || rc=1

  if [[ -d "$FRONTEND_DIR/node_modules" ]]; then
    log "tsc -b"
    (cd "$FRONTEND_DIR" && npm run typecheck) || rc=1
  else
    warn "前端未安装依赖,跳过 tsc"
  fi

  [[ $rc -eq 0 ]] && record "代码审查" PASS "fmt + clippy + tsc" \
                  || record "代码审查" FAIL "存在格式/告警/类型错误"
  return $rc
}

# ---------------------------------------------------------------- 阶段:L1 单元
stage_unit() {
  [[ "${SKIP_FUNC:-0}" == "1" ]] && { record "L1 单元" SKIP "SKIP_FUNC=1"; return 0; }
  log "L1 单元测试(零外部依赖)"
  if (cd "$TEST_DIR" && cargo test --lib unit_ -- --nocapture); then
    record "L1 单元" PASS "全绿"; return 0
  fi
  record "L1 单元" FAIL "有失败用例"; return 1
}

# ---------------------------------------------------------------- 阶段:L2 集成
stage_integration() {
  [[ "${SKIP_FUNC:-0}" == "1" ]] && { record "L2 集成" SKIP "SKIP_FUNC=1"; return 0; }

  if [[ -z "${DATABASE_URL:-}" || -z "${REDIS_URL:-}" ]]; then
    if [[ "${STRICT:-0}" == "1" ]]; then
      record "L2 集成" FAIL "STRICT=1 但缺 DATABASE_URL / REDIS_URL"
      fail "拒绝跳过依赖套件 —— 跳过会造成假绿"
      return 1
    fi
    record "L2 集成" SKIP "缺依赖连接参数(dev 策略)"
    return 0
  fi

  log "L2 集成测试(openGauss + Valkey)"
  if (cd "$TEST_DIR" && cargo test --lib integration_ -- --nocapture --test-threads=1); then
    record "L2 集成" PASS "全绿"; return 0
  fi
  record "L2 集成" FAIL "有失败用例"; return 1
}

# ---------------------------------------------------------------- 阶段:L3 契约
stage_contract() {
  [[ "${SKIP_FUNC:-0}" == "1" ]] && { record "L3 契约" SKIP "SKIP_FUNC=1"; return 0; }

  local baseline_dir="$TEST_DIR/cases/fixtures/baseline"
  local n; n=$(find "$baseline_dir" -name '*.json' 2>/dev/null | wc -l)
  if [[ "$n" -eq 0 ]]; then
    if [[ "${STRICT:-0}" == "1" ]]; then
      record "L3 契约" FAIL "无录制基线,STRICT=1 下不允许跳过"
      fail "请先运行 test/env/record-baseline.sh"
      return 1
    fi
    record "L3 契约" SKIP "无录制基线(dev 策略)"
    return 0
  fi

  log "L3 契约测试(基线 $n 份)"
  if (cd "$TEST_DIR" && cargo test --lib contract_ -- --nocapture); then
    record "L3 契约" PASS "$n 份基线全绿"; return 0
  fi
  record "L3 契约" FAIL "契约偏差 —— 先看 fixture 的 recorded_at 判断是回归还是上游演进"
  return 1
}

# ---------------------------------------------------------------- 阶段:L4 冒烟
stage_smoke() {
  if [[ -z "${SEAWEIR_HTTP_BASE_URL:-}" ]]; then
    record "L4 冒烟" SKIP "缺 SEAWEIR_HTTP_BASE_URL"; return 0
  fi
  log "L4 部署后冒烟"
  if (cd "$TEST_DIR" && cargo test --lib live_ -- --nocapture); then
    record "L4 冒烟" PASS "$SEAWEIR_HTTP_BASE_URL"; return 0
  fi
  record "L4 冒烟" FAIL "部署实例异常"; return 1
}

# ---------------------------------------------------------------- 阶段:性能基线
stage_perf() {
  [[ "${SKIP_PERF:-0}" == "1" ]] && { record "性能基线" SKIP "SKIP_PERF=1"; return 0; }
  log "性能基线(微基准,不做门禁)"
  (cd "$TEST_DIR" && cargo test --release --lib perf -- --ignored --nocapture) \
    && record "性能基线" PASS "见输出" \
    || record "性能基线" SKIP "基线未达标或未实现(不阻断)"
  return 0
}

# ---------------------------------------------------------------- 阶段:覆盖率
stage_coverage() {
  [[ "${SKIP_COVERAGE:-0}" == "1" ]] && { record "覆盖率" SKIP "SKIP_COVERAGE=1"; return 0; }
  if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
    record "覆盖率" SKIP "未安装 cargo-llvm-cov"; return 0
  fi
  log "覆盖率(账务 ≥90%,其余 ≥70%)"
  # TODO(实施): cargo llvm-cov --workspace --fail-under-lines 70
  #             并对 billing / repository 额度方法单独设 90 门槛
  record "覆盖率" SKIP "待实施"
  return 0
}
