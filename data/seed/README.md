# sea-weir 种子数据目录

本目录只放**基础种子**(幂等):`setups` 首装标记与可登录的 `root` 账号。
业务配置(ModelRatio / 分组倍率 / OAuth 与支付开关等)由应用启动时写入 `options`
默认值,不在此重复维护,避免与代码默认值双份漂移。

## 文件

| 文件 | 用途 |
|---|---|
| `seed-base.sql` | 幂等 SQL:写 `setups`,并按需创建 `root`(role=100) |
| `seed-all.sh` | 本地一键初始化:连通性 → 建库/建表 → 基础种子 → 校验 |

## 口令处理

- 仓库内**不存任何默认口令明文**;`root` 口令的 bcrypt 哈希在 `seed-all.sh` /
  `data/env/*.sh` 运行时由本机 `python3 -c 'import bcrypt'` 实时生成。
- 口令来源:`SEED_ROOT_PASSWORD`(缺省 `Admin@123456`,仅用于首次初始化)。
- 缺少 `python3 bcrypt` 时脚本仅写 `setups`,并提示改用 `/api/setup` 首装向导创建 root。
- 首次登录后请立即修改 root 口令。

## 使用

```bash
# 走 Nacos(集群内或可连通 Nacos)
NAMESPACE=loong-service-sea-weir bash data/seed/seed-all.sh

# 直连本地库(绕过 Nacos,调试用)
bash data/seed/seed-all.sh -h 127.0.0.1 -p 5432 -d sea_weir -u sea_weir -w 'your-password'

# 覆盖 root 初始口令
SEED_ROOT_PASSWORD='MyStr0ngPass!' bash data/seed/seed-all.sh -h 127.0.0.1 -w 'db-pass' -u sea_weir -d sea_weir
```

## 扩展

需要业务演示数据(渠道/令牌/模型等)时,在本目录新增 `seed-demo.sql` 并在
`seed-all.sh` 校验步骤前追加 `apply_seed "$SEED_DEMO_SQL" "演示数据"`。
演示数据必须同样幂等,且**不得进入 prod 路径**(`data/env/prod.sh` 不引用)。
