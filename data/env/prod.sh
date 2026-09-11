#!/bin/bash
# =============================================================================
# 文件: data/env/prod.sh
# 用途: prod 环境数据阶段 —— **无损升级**
#
# 语义:只补建缺失表/索引;仅在完全没有用户时创建 root(避免覆盖线上账号);
#       绝不 DROP / TRUNCATE / 覆盖已有数据;不加载任何测试/性能数据。
#       口令哈希不落仓库,由 SEED_ROOT_PASSWORD 注入(缺省仅用于首次初始化)。
# =============================================================================
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

echo "==> sea-weir 数据阶段:prod(无损升级,ENV=${NAMESPACE:-<未设置>})"
parse_db_params
ensure_database
check_db_connectivity
apply_ddl

# 仅当 users 表为空时才写基础种子,避免在线上重置/新增账号。
if [ "$(count_rows users)" = "0" ]; then
    warn "users 表为空(首次部署):写入 setups 与 root 初始账号"
    apply_base_seed
else
    ok "users 表非空:跳过基础种子,保持现有账号不变"
fi

echo "[Step] 校验"
verify_count setups 1 "首装标记"
ok "prod 数据阶段完成(已有数据未被改动)"
