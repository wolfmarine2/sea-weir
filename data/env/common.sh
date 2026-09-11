#!/bin/bash
# =============================================================================
# 文件: data/env/common.sh
# 用途: sea-weir 数据阶段环境脚本的公共函数库(被 data/env/<env>.sh source)
# 提供: 连接参数解析(Nacos 唯一来源 database.dsn)、psql 封装、DDL/种子执行、验证
#
# 连接参数唯一来源:Nacos 配置 `sea-weir.yaml@sea-weir@loong` 的 database.dsn,
# 不接受 DB_* 环境变量覆盖。仅本地调试可用 NACOS_SKIP=1 + DB_DSN(见下)。
# =============================================================================
set -euo pipefail

COMMON_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="$(cd "${COMMON_DIR}/.." && pwd)"

# shellcheck source=../lib/nacos.sh
source "${DATA_DIR}/lib/nacos.sh"

DDL_SQL="${DATA_DIR}/ddl.sql"
SEED_BASE_SQL="${DATA_DIR}/seed/seed-base.sql"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
ok()   { printf "  ${GREEN}[OK]${NC}   %s\n" "$1"; }
warn() { printf "  ${YELLOW}[WARN]${NC} %s\n" "$1"; }
err()  { printf "  ${RED}[ERR]${NC}  %s\n" "$1"; }
fail() { err "$1"; exit 1; }

# =============================================================================
# parse_db_params: 解析连接参数到 DB_DSN/DB_HOST/DB_PORT/DB_USER/DB_NAME/DB_PASSWORD
#   NACOS_SKIP=1 时改用 DB_DSN 环境变量(仅调试,生产禁止)。
# =============================================================================
parse_db_params() {
    if [ "${NACOS_SKIP:-}" = "1" ]; then
        [ -n "${DB_DSN:-}" ] || fail "NACOS_SKIP=1 但未提供 DB_DSN(仅调试用,生产/流水线禁止)"
        warn "NACOS_SKIP=1: 使用 DB_DSN 环境变量(调试模式,生产禁止)"
        DB_DSN_VALUE="$DB_DSN"
    else
        if ! command -v python3 >/dev/null 2>&1; then
            fail "缺少 python3,无法解析 Nacos 配置"
        fi
        nacos_fetch_config || fail "无法从 Nacos 获取配置(${NACOS_DATA_ID:-$DEFAULT_NACOS_DATA_ID}@${NACOS_GROUP:-$DEFAULT_NACOS_GROUP}@${NACOS_NAMESPACE:-$DEFAULT_NACOS_NAMESPACE});已输出逐层诊断,请检查 Nacos 配置与网络"
        DB_DSN_VALUE="$(nacos_dsn_from_content "$content" | head -1)"
        [ -n "$DB_DSN_VALUE" ] || fail "Nacos 配置的 database.dsn 为空(必填项)"
    fi

    # 解析 postgres:// URL → host/port/user/db/password(口令 URL 解码)
    local parsed
    parsed="$(DB_DSN="$DB_DSN_VALUE" python3 -c '
import os, urllib.parse as u
p = u.urlsplit(os.environ["DB_DSN"])
print(p.hostname or "")
print(p.port or 5432)
print(u.unquote(p.username or ""))
print(u.unquote(p.password or ""))
print((p.path or "/").lstrip("/"))
')" || fail "database.dsn 解析失败: ${DB_DSN_VALUE}"
    DB_HOST="$(printf '%s' "$parsed" | sed -n 1p)"
    DB_PORT="$(printf '%s' "$parsed" | sed -n 2p)"
    DB_USER="$(printf '%s' "$parsed" | sed -n 3p)"
    DB_PASSWORD="$(printf '%s' "$parsed" | sed -n 4p)"
    DB_NAME="$(printf '%s' "$parsed" | sed -n 5p)"

    [ -n "$DB_HOST" ] && [ -n "$DB_USER" ] && [ -n "$DB_NAME" ] \
        || fail "database.dsn 不完整(需含 host/user/password/db): ${DB_DSN_VALUE}"
    export PGPASSWORD="$DB_PASSWORD"
    ok "数据库连接: ${DB_HOST}:${DB_PORT}/${DB_NAME} (user=${DB_USER})"
}

# =============================================================================
# psql 执行封装
# =============================================================================
psql_run()   { psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -q "$@"; }
psql_loose() { psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -q "$@"; }

check_db_connectivity() {
    echo "[Step] 检查数据库连通性: ${DB_HOST}:${DB_PORT}/${DB_NAME}"
    if psql_loose -c "SELECT 1;" >/dev/null 2>&1; then
        ok "数据库连接成功"
    else
        local psql_err
        psql_err="$(psql_loose -c "SELECT 1;" 2>&1)" || true
        err "无法连接 ${DB_HOST}:${DB_PORT}/${DB_NAME},详情:"
        printf '    %s\n' "$psql_err" | sed 's/^[[:space:]]*/    /' | head -6
        fail "数据库连接失败(连接参数来自 Nacos database.dsn),请检查配置内容与网络"
    fi
}

# ensure_database: 目标库不存在时用同账号尝试创建(需建库权限;失败仅告警)。
ensure_database() {
    echo "[Step] 确认数据库存在: ${DB_NAME}"
    local exists
    exists="$(psql_loose -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname='${DB_NAME}';" 2>/dev/null || true)"
    if [ "$exists" = "1" ]; then
        ok "数据库已存在"
        return 0
    fi
    if psql_loose -d postgres -c "CREATE DATABASE \"${DB_NAME}\";" >/dev/null 2>&1; then
        ok "数据库已创建: ${DB_NAME}"
    else
        warn "数据库 ${DB_NAME} 不存在且当前账号无权创建;请由 DBA 执行 data/00-init-database.sql"
    fi
}

# apply_ddl: 幂等 DDL(仅 CREATE ... IF NOT EXISTS + COMMENT,无 DROP/TRUNCATE)
apply_ddl() {
    echo "[Step] 应用 DDL(幂等,无损): ${DDL_SQL}"
    [ -f "$DDL_SQL" ] || fail "缺少 DDL 文件: ${DDL_SQL}"
    psql_run -f "$DDL_SQL" >/dev/null && ok "表结构已就绪(缺失表/索引已补建)"
}

# apply_seed <file> <label>: 执行种子 SQL(自带幂等判断)
apply_seed() {
    local file="$1" label="$2"
    local -a psql_vars=("${@:3}")
    echo "[Step] 应用 ${label}: ${file}"
    [ -f "$file" ] || fail "缺少数据文件: ${file}"
    psql_run "${psql_vars[@]}" -f "$file" >/dev/null && ok "${label} 完成"
}

count_rows() {
    local table="$1"
    psql_loose -tAc "SELECT COUNT(*) FROM \"${table}\";" 2>/dev/null || echo "0"
}

verify_count() {
    local table="$1" expected="$2" label="$3" count
    count="$(count_rows "$table")"
    if [ "$count" -ge "$expected" ] 2>/dev/null; then
        ok "${label}: ${count} 条 (期望 >= ${expected})"
    else
        warn "${label}: ${count} 条 (期望 >= ${expected})"
    fi
}

# truncate_business_tables: 仅 dev 使用 —— 清空业务表后重灌种子。
# 依赖顺序:先清子表(abilities/tokens/... )再清 users/channels。
truncate_business_tables() {
    echo "[Step] dev:清空业务表(重建数据)"
    psql_run <<'SQL' >/dev/null
TRUNCATE TABLE
    logs, quota_data, checkins, top_ups, redemptions,
    subscription_pre_consume_records, subscription_orders, user_subscriptions,
    tasks, midjourneys, file_objects, two_fa_backup_codes, two_fas,
    passkey_credentials, user_oauth_bindings, tokens, abilities
RESTART IDENTITY CASCADE;
SQL
    psql_run -c 'TRUNCATE TABLE users, channels, vendors, models, subscription_plans RESTART IDENTITY CASCADE;' >/dev/null
    ok "业务表已清空(options/setups 保留)"
}

# apply_base_seed: 应用基础种子(setups 首装标记 + root 用户)。
#   root 口令哈希由本机 python3(bcrypt)实时生成,仓库内不存任何默认口令;
#   无 bcrypt 时仅写 setups,并提示由 /api/setup 首装向导创建 root。
apply_base_seed() {
    local hash="" aff="rootaff"
    if python3 -c 'import bcrypt' >/dev/null 2>&1; then
        hash="$(python3 -c 'import bcrypt,sys; print(bcrypt.hashpw(sys.argv[1].encode(), bcrypt.gensalt(rounds=10)).decode())' "${SEED_ROOT_PASSWORD:-Admin@123456}")"
    else
        warn "未安装 python3 bcrypt,跳过 root 用户创建(可改用 /api/setup 首装向导,或 pip install bcrypt 后重跑)"
    fi
    apply_seed "$SEED_BASE_SQL" "基础种子(setups + root)" \
        -v "root_password_hash=${hash}" -v "root_aff_code=${aff}"
}
