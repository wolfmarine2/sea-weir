#!/bin/bash
# =============================================================================
# 文件: data/lib/nacos.sh
# 用途: Nacos 配置拉取/解析共享函数库(供 data/env/common.sh 与 seed 脚本 source)
#
# sea-weir 的配置位置(与 doc/system-design.md §9.2 一致):
#   data-id = sea-weir.yaml
#   group   = sea-weir
#   tenant  = loong
# 数据库连接参数唯一来源为该配置的 `database.dsn`(PostgreSQL URL),
# 不接受 DB_* 环境变量覆盖(调试可用 NACOS_SKIP=1 + DB_DSN,见 common.sh)。
#
# 可覆盖项:NACOS_ADDR / NACOS_DATA_ID / NACOS_GROUP / NACOS_NAMESPACE /
#           NACOS_USERNAME / NACOS_PASSWORD(缺省 loong/loong,与部署 Secret 一致)
# =============================================================================
set -euo pipefail

DEFAULT_NACOS_ADDR='nacos-svc.nacos.svc.cluster.local:8848'
DEFAULT_NACOS_DATA_ID='sea-weir.yaml'
DEFAULT_NACOS_GROUP='sea-weir'
DEFAULT_NACOS_NAMESPACE='loong'

# nacos_fetch_config: 拉取配置到全局变量 content
#   策略:先匿名 GET(未开鉴权直接成功);401/403 时用账号登录换 token 再拉。
#   失败输出逐层诊断并返回 1。
nacos_fetch_config() {
    local addr data_id group namespace user pass base
    token=""; content=""; http_code=""; body=""

    addr="${NACOS_ADDR:-$DEFAULT_NACOS_ADDR}"
    data_id="${NACOS_DATA_ID:-$DEFAULT_NACOS_DATA_ID}"
    group="${NACOS_GROUP:-$DEFAULT_NACOS_GROUP}"
    namespace="${NACOS_NAMESPACE:-$DEFAULT_NACOS_NAMESPACE}"
    user="${NACOS_USERNAME-loong}"
    pass="${NACOS_PASSWORD-loong}"
    base="http://${addr}"

    local tmp
    tmp="$(mktemp)"

    _get_config() {
        local code
        code="$(curl -sS --max-time 10 -o "$tmp" -w '%{http_code}' \
                "${base}/nacos/v1/cs/configs" --get \
                --data-urlencode "dataId=${data_id}" \
                --data-urlencode "group=${group}" \
                --data-urlencode "tenant=${namespace}" \
                ${token:+--data-urlencode "accessToken=${token}"} 2>/dev/null)" || return 1
        http_code="$code"
        body="$(cat "$tmp" 2>/dev/null)"
        [ "$code" = "200" ]
    }

    _login() {
        local resp code
        if [ -z "$user" ] || [ -z "$pass" ]; then
            token=""; return 0
        fi
        code="$(curl -sS --max-time 10 -o "$tmp" -w '%{http_code}' \
                -X POST "${base}/nacos/v1/auth/login" \
                --data-urlencode "username=${user}" \
                --data-urlencode "password=${pass}" 2>/dev/null)" || { token=""; return 1; }
        resp="$(cat "$tmp" 2>/dev/null)"
        token="$(printf '%s' "$resp" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("accessToken",""))' 2>/dev/null || true)"
        [ -n "$token" ]
    }

    if _get_config; then
        content="$body"; rm -f "$tmp"; return 0
    fi

    if [ "$http_code" = "401" ] || [ "$http_code" = "403" ]; then
        if ! _login; then
            echo "[ERR] Nacos 匿名拉取被拒(HTTP ${http_code}),且登录失败(user=${user})" >&2
            rm -f "$tmp"; return 1
        fi
        if ! _get_config; then
            echo "[ERR] Nacos 登录成功但拉取配置仍失败(${data_id}@${group}@${namespace}, HTTP ${http_code})" >&2
            rm -f "$tmp"; return 1
        fi
        content="$body"; rm -f "$tmp"; return 0
    fi

    echo "[ERR] Nacos 拉取配置失败(${base}, ${data_id}@${group}@${namespace}, HTTP ${http_code})" >&2
    rm -f "$tmp"
    return 1
}

# nacos_dsn_from_content: 从配置内容解析 database.dsn
#   优先 PyYAML,缺失时用迷你解析器(仅取 database.dsn / database.log_dsn)。
#   输出两行:主库 DSN、日志库 DSN(可为空)。
nacos_dsn_from_content() {
    printf '%s' "$1" | python3 -c '
import sys
raw = sys.stdin.read()
dsn = log_dsn = ""
try:
    import yaml
    cfg = yaml.safe_load(raw) or {}
    db = cfg.get("database") or {}
    dsn = str(db.get("dsn") or "")
    log_dsn = str(db.get("log_dsn") or "")
except ImportError:
    in_db = False
    for line in raw.splitlines():
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        if not line.startswith(" ") and s.endswith(":"):
            in_db = s[:-1].strip() == "database"
            continue
        if in_db and ":" in s:
            k, v = s.split(":", 1)
            v = v.split(" #", 1)[0].strip().strip("\"").strip("\x27")
            if k.strip() == "dsn":
                dsn = v
            elif k.strip() == "log_dsn" and v not in ("null", "~", ""):
                log_dsn = v
print(dsn)
print(log_dsn)
'
}
