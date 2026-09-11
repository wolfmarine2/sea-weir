#!/bin/bash
# =============================================================================
# 文件: data/seed/seed-all.sh
# 用途: 本地一键初始化 —— 连通性检查 + 建库/建表 + 幂等基础种子 + 验证
#
# 连接参数优先级(与 service-auth 一致):
#   命令行 -h/-p/-d/-u/-w > 环境变量 SEED_* > Nacos 的 database.dsn
# 口令不落库明文;bcrypt 哈希由本机 python3(bcrypt)实时生成。
#
# 用法:
#   # 走 Nacos(集群内/可连通 Nacos):
#   NAMESPACE=loong-service-sea-weir bash data/seed/seed-all.sh
#   # 直连本地库:
#   bash data/seed/seed-all.sh -h 127.0.0.1 -p 5432 -d sea_weir -u sea_weir -w 'xxx'
# =============================================================================
set -euo pipefail

SEED_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="$(cd "${SEED_DIR}/.." && pwd)"

# 命令行参数覆盖(在 parse_db_params 之前生效)
while getopts "h:p:d:u:w:" opt; do
    case "$opt" in
        h) export DB_DSN_HOST="$OPTARG" ;;
        p) export DB_DSN_PORT="$OPTARG" ;;
        d) export DB_DSN_NAME="$OPTARG" ;;
        u) export DB_DSN_USER="$OPTARG" ;;
        w) export SEED_PASS="$OPTARG" ;;
        *) echo "用法: $0 [-h host] [-p port] [-d dbname] [-u user] [-w password]"; exit 2 ;;
    esac
done

# 若提供了显式主机,则绕过 Nacos(本地调试路径)
if [ -n "${DB_DSN_HOST:-}" ]; then
    export NACOS_SKIP=1
    : "${DB_DSN_PORT:=5432}"
    : "${DB_DSN_NAME:=sea_weir}"
    : "${DB_DSN_USER:=sea_weir}"
    : "${SEED_PASS:?使用 -h 直连时必须提供 -w 口令或 SEED_PASS}"
    # 口令/用户名做百分号编码,避免 @ : / 等保留字符破坏 URL
    export DB_DSN
    DB_DSN="$(DB_DSN_USER="$DB_DSN_USER" DB_DSN_PASS="$SEED_PASS" \
        DB_DSN_HOST="$DB_DSN_HOST" DB_DSN_PORT="$DB_DSN_PORT" DB_DSN_NAME="$DB_DSN_NAME" \
        python3 -c 'import os, urllib.parse as u
q = lambda s: u.quote(s, safe="")
print("postgres://%s:%s@%s:%s/%s" % (q(os.environ["DB_DSN_USER"]), q(os.environ["DB_DSN_PASS"]),
      os.environ["DB_DSN_HOST"], os.environ["DB_DSN_PORT"], os.environ["DB_DSN_NAME"]))')"
fi

source "${DATA_DIR}/env/common.sh"

echo "==> sea-weir 本地一键初始化"
parse_db_params
ensure_database
check_db_connectivity
apply_ddl
apply_base_seed

echo "[Step] 校验"
verify_count setups 1 "首装标记"
verify_count users  1 "root 用户"
ok "初始化完成"
echo
echo "  登录账号:root"
echo "  口令:${SEED_ROOT_PASSWORD:-Admin@123456}(可用 SEED_ROOT_PASSWORD 覆盖;首次登录后请立即修改)"
