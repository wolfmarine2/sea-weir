#!/bin/bash
# =============================================================================
# 文件: data/env/test.sh
# 用途: test 环境数据阶段 —— **无损升级**
#
# 语义:只补建缺失表/索引,只在缺失时补基础种子(setups / root);
#       绝不 DROP / TRUNCATE / 覆盖已有数据。业务测试数据由测试用例自行准备。
# =============================================================================
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

echo "==> sea-weir 数据阶段:test(无损升级,ENV=${NAMESPACE:-<未设置>})"
parse_db_params
ensure_database
check_db_connectivity
apply_ddl
apply_base_seed

echo "[Step] 校验"
verify_count setups 1 "首装标记"
verify_count users  1 "root 用户"
ok "test 数据阶段完成(已有数据未被改动)"
