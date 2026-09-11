# sea-weir 数据层目录

数据库结构(DDL)、分环境数据阶段脚本、基础种子与引导脚本。
组织方式沿用 [service-auth `data/`](https://forgejo.prod.dhzq.cn/loong/service-auth) 约定。

## 目录清单

| 文件/目录 | 用途 |
|---|---|
| `ddl.sql` | **权威 DDL**(26 张表 + 索引,openGauss;全部 `CREATE ... IF NOT EXISTS`,幂等且**不含 DROP/TRUNCATE**) |
| `00-init-database.sql` | 超级用户一次性引导:创建角色与数据库(`\gexec` 按需执行) |
| `cicd.sh` | **CD 数据阶段统一入口**:按 `ENV` 分发到 `env/<env>.sh`(与 LoongCICD 调用约定一致) |
| `env/common.sh` | 公共函数库:Nacos 连接参数解析(唯一来源 `database.dsn`)+ psql 封装 + DDL/种子执行 + 验证 |
| `lib/nacos.sh` | Nacos 拉取/解析共享函数库(供 `env/common.sh`、`seed/seed-all.sh` 复用) |
| `seed/seed-base.sql` | 基础种子(幂等):`setups` 首装标记 + `root` 账号 |
| `seed/seed-all.sh` | 本地一键初始化:连通性 → 建库/建表 → 基础种子 → 校验 |
| `env/dev.sh` / `test.sh` / `prod.sh` | 分环境数据策略 |

## 分环境数据策略

| 环境 | 脚本 | 策略 |
|---|---|---|
| dev | `env/dev.sh` | 补建缺失表/索引 → **清空业务表重灌基础种子**(`options`/`setups` 保留) |
| test | `env/test.sh` | **无损升级**:补建缺失表/索引 + 仅在缺失时补基础种子,绝不覆盖/清空已有数据 |
| prod | `env/prod.sh` | **无损升级**:补建缺失表/索引;仅在 `users` 为空时创建 root,绝不覆盖/清空已有数据;不加载测试数据 |

`ddl.sql` 只做增量(`IF NOT EXISTS`),因此 test/prod 的「无损」由 DDL 与脚本双重保证;
破坏性重建**只**出现在 dev 的 `truncate_business_tables`。

## 使用方式

```bash
# 流水线(LoongCICD 数据阶段注入 NAMESPACE/PROJECT/ENV)
NAMESPACE=loong-service-sea-weir ENV=dev  bash data/cicd.sh
NAMESPACE=loong-service-sea-weir ENV=test bash data/cicd.sh
NAMESPACE=loong-service-sea-weir ENV=prod bash data/cicd.sh
```

> ⚠ **Nacos 唯一来源**:数据库连接参数唯一来源是 Nacos 配置
> `sea-weir.yaml@sea-weir@loong` 的 `database.dsn`(PostgreSQL URL),
> 不接受 `DB_*` 环境变量覆盖,仓库内也没有任何默认口令。
> Nacos 账号缺省 `loong/loong`(与部署 Secret 一致),可用
> `NACOS_USERNAME`/`NACOS_PASSWORD` 覆盖,置空视为匿名访问。
> 脚本先匿名拉取,401/403 时登录换 token 再拉;失败即数据阶段失败。
> 唯一例外:`NACOS_SKIP=1` + `DB_DSN`(仅本地调试,README 与脚本均标注禁用生产)。

## 首次部署(数据库引导)

```bash
# 1) 由 DBA 以超级用户创建角色与库(口令通过 psql 变量注入,不入仓库)
psql -h <host> -p 5432 -U <superuser> -d postgres \
     -v app_password="$SEAWEIR_DB_PASSWORD" -f data/00-init-database.sql

# 2) 建表 + 基础种子
NAMESPACE=loong-service-sea-weir ENV=prod bash data/cicd.sh
```

## 本地一键初始化

```bash
# 走 Nacos
NAMESPACE=loong-service-sea-weir bash data/seed/seed-all.sh

# 直连本地库(绕过 Nacos)
bash data/seed/seed-all.sh -h 127.0.0.1 -p 5432 -d sea_weir -u sea_weir -w 'db-pass'
```

初始化后账号 `root` / 口令 `Admin@123456`(`SEED_ROOT_PASSWORD` 可覆盖;bcrypt 哈希
运行时生成,仓库不存明文)。详见 `seed/README.md`。

## 存量数据迁移(new-api → openGauss)

按要求迁移前的结构差异与处理:

1. **自增列类型**:new-api 的 `INTEGER` 主键按 ADR-003 提升为 `BIGINT`;
   迁移时用 `ALTER TABLE ... ALTER COLUMN id TYPE BIGINT;` 并同步序列。
2. **JSON 文本列**:`setting`/`channel_info`/`properties`/`private_data`/`items`/`other`
   等由 `TEXT` 导入后转 `JSONB`:`ALTER TABLE ... ALTER COLUMN setting TYPE JSONB USING setting::jsonb;`
3. **软删除唯一约束**:`users.username`、`tokens.key`、`models.model_name`、
   `vendors.name`、`prefill_groups.name` 等需改为**部分唯一索引**
   (`WHERE deleted_at IS NULL`),见 `ddl.sql`。
4. **保留字列**:`"group"`、`"key"` 迁移脚本中需双引号引用。
5. **乐观校验**:迁移后核对 `users`/`tokens`/`channels`/`abilities` 行数与
   `SUM(quota)` 聚合值。

> 迁移工具建议复用公司 `dbswitch`;一次性脚本可参照 service-auth 的
> `data/migrate_mariadb_to_opengauss.sh` 结构(按表映射 + 行数校验)。

## 与 service-auth 的差异

- **配置段**:service-auth 用 `db.host/port/db-name/username/password`,
  sea-weir 用 `database.dsn`(PostgreSQL URL)+ 可选 `database.log_dsn`(日志分库)。
- **DDL 幂等性**:service-auth 的 `ddl.sql` 头部含 `DROP TABLE ... CASCADE`;
  sea-weir 的 `ddl.sql` **不含任何破坏性语句**,无损语义更严格,dev 的重建显式写在
  `env/dev.sh` 的 `truncate_business_tables`。
- **日志库**:sea-weir 支持日志表独立库(`log_dsn`);本目录脚本只初始化主库,
  日志库结构复用同一份 `ddl.sql` 中 `logs` 表,可由同一脚本对该库重跑(DDL 幂等)。
