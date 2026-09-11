# 数据库迁移

openGauss(PostgreSQL 系语义)DDL,由 `sqlx migrate` 管理。

## 约定

- 文件名 `{timestamp}_{描述}.sql`,只增不改 —— 已发布的迁移禁止编辑
- 保留字 `"group"`、`"key"` 统一双引号引用
- 软删除表的唯一约束一律用**部分索引**:`... WHERE deleted_at IS NULL`
- 额度、unix 秒时间戳统一 `BIGINT`;JSON 列统一 `JSONB`
- 仅软删除列用 `TIMESTAMPTZ`

## 表清单(26 张)

与 new-api `model/main.go` 的 AutoMigrate 清单一一对应,便于存量数据迁移与回滚。
核心 8 表 DDL 见 doc/system-design.md §7.2,其余 18 表见 doc/architecture/er-diagram.puml。

| 组 | 表 |
|---|---|
| 核心(8) | users, tokens, channels, abilities, logs, options, redemptions, top_ups |
| 订阅(4) | subscription_plans, subscription_orders, user_subscriptions, subscription_pre_consume_records |
| 任务(2) | tasks, midjourneys |
| 模型(2) | models, vendors |
| 认证(4) | passkey_credentials, two_fas, two_fa_backup_codes, user_oauth_bindings |
| 其他(6) | checkins, prefill_groups, custom_oauth_providers, quota_data, file_objects, setups |

## TDD 顺序

先写 Repository 的集成测试(testcontainers 起 openGauss),测试红了再补对应迁移与 SQL。
