#!/bin/bash
# =============================================================================
# 文件: data/env/dev.sh
# 用途: dev 环境数据阶段 —— 补建表结构 + 清空业务表后重灌基础种子
#
# 语义:开发环境数据可随意重建。options/setups 保留,业务表清空。
# =============================================================================
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

echo "==> sea-weir 数据阶段:dev(重建业务数据,ENV=${NAMESPACE:-<未设置>})"
parse_db_params
ensure_database
check_db_connectivity
check_compatibility
apply_ddl
truncate_business_tables
apply_base_seed

echo "[Step] 校验"
verify_count setups 1 "首装标记"
verify_count users  1 "root 用户"
ok "dev 数据阶段完成"
