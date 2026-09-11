#!/usr/bin/env bash
# 从运行中的 new-api 实例录制契约基线。
#
# 用法:
#   NEWAPI_BASE_URL=http://localhost:3000 \
#   NEWAPI_ADMIN_TOKEN=<系统访问令牌> \
#   NEWAPI_USER_ID=1 \
#   NEWAPI_SK_TOKEN=<sk-xxx,可选,用于中继面端点> \
#   NEWAPI_LOWPRIV_TOKEN=<role=1 用户的访问令牌,可选,用于角色闸门用例> \
#   bash test/env/record-baseline.sh
#
# 产出 test/cases/fixtures/baseline/*.json
#
# 注意:
#   - 不要对生产实例录制,响应可能含真实用户数据
#   - 管理面的 access token 路径**同样要求 New-Api-User 头**,故 NEWAPI_USER_ID 必填
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$TEST_DIR/cases/fixtures/baseline"

: "${NEWAPI_BASE_URL:?需要 NEWAPI_BASE_URL,例如 http://localhost:3000}"
: "${NEWAPI_ADMIN_TOKEN:?需要 NEWAPI_ADMIN_TOKEN(new-api 的系统访问令牌,GET /api/user/token 获取)}"
NEWAPI_USER_ID="${NEWAPI_USER_ID:-1}"
NEWAPI_SK_TOKEN="${NEWAPI_SK_TOKEN:-}"
NEWAPI_LOWPRIV_TOKEN="${NEWAPI_LOWPRIV_TOKEN:-}"
NEWAPI_LOWPRIV_USER_ID="${NEWAPI_LOWPRIV_USER_ID:-2}"

if ! curl -fsS --max-time 5 "$NEWAPI_BASE_URL/api/status" >/dev/null 2>&1; then
  echo "[fail] 无法访问 $NEWAPI_BASE_URL/api/status —— 实例是否已启动?" >&2
  exit 1
fi

exec python3 "$TEST_DIR/env/record_baseline.py" \
  --base-url    "$NEWAPI_BASE_URL" \
  --admin-token "$NEWAPI_ADMIN_TOKEN" \
  --user-id     "$NEWAPI_USER_ID" \
  --sk-token    "$NEWAPI_SK_TOKEN" \
  --lowpriv-token   "$NEWAPI_LOWPRIV_TOKEN" \
  --lowpriv-user-id "$NEWAPI_LOWPRIV_USER_ID" \
  --out         "$OUT_DIR"
