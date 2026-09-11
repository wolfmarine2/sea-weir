# ADR-003: 数据库选型与迁移

## Metadata

| 项 | 内容 |
|----|------|
| 编号 | ADR-003 |
| 标题 | 数据库选型与迁移(openGauss + 表结构兼容) |
| 状态 | Proposed |
| 日期 | 2026-09-10 |
| 决策者 | Architecture Team |
| 影响范围 | `sea-weir-repository`、migrations、运维迁移工具 |
| 依赖 | ADR-001, ADR-002 |

## Status History

| 日期 | 状态 | 说明 |
|------|------|------|
| 2026-09-10 | Proposed | 初始提议 |

## Context(上下文)

### 问题陈述

new-api 同时支持 SQLite/MySQL/PostgreSQL 三种方言(保留字转义、类型宽度、迁移逻辑均分叉),重构时需要选定唯一目标数据库,并保证存量 MySQL/PG 部署的数据可迁移、表结构语义可对齐验证。

### 约束条件

- 公司标准数据库为 openGauss(PostgreSQL 系)
- 存量部署以 MySQL 为主;重构上线需数据迁移窗口可控
- 额度等关键列在原实现中宽度不一致(Go int vs bigint)

### 驱动力

- 消除方言分支与保留字转义(Go 版 `group`/`key` 列按方言切换引号)
- sqlx 编译期校验要求单一确定的 schema
- 账务列类型统一(消除 int32/int64 混用)
- 迁移可回滚:表名/字段语义与 new-api 对齐

### 目标状态

单一 openGauss 方言;26 张表 DDL 入库(migrations,与 new-api `model/main.go` AutoMigrate 清单一一对应);存量数据迁移工具 + 校验报告;类型映射表文档化。

### 备选方案

#### 方案 A:openGauss 单方言 + 表结构兼容 new-api

**描述**: 表名/列名/语义与 new-api 一致,类型按映射表升级(int→BIGINT、text JSON→JSONB、软删唯一约束→部分索引)。
**优点**:
- 单方言,sqlx 编译期校验可行;消除转义分支
- 存量数据可按表直迁,回滚路径清晰(旧系统库不动)
- 公司 DBA/运维基线现成
**缺点**:
- SQLite 单机部署形态消失(个人用户不友好,本项目面向公司部署可接受)

#### 方案 B:继续三方言(sqlx 编译期校验只对 PG 生效)

**描述**: 保留 MySQL/SQLite 兼容。
**优点**:
- 部署形态不变
**缺点**:
- sqlx 校验只对一种方言可靠;保留字/类型分支继续存在;测试矩阵 ×3

#### 方案 C:重设计表结构(规范化重构)

**描述**: 借机重设 schema(如拆 JSON 列、重命名)。
**优点**:
- 模型更干净
**缺点**:
- 数据迁移与回滚复杂化;与 new-api 行为比对失去基准;契约兼容风险大

### 决策依据

| 维度 | 权重 | A | B | C |
|---|---|---|---|---|
| 公司标准符合 | 高 | ✔ | △ | ✔ |
| 迁移/回滚可控 | 高 | ✔ | △ | ✘ |
| 工程复杂度 | 中 | ✔ | ✘ | ✘ |
| 模型整洁 | 低 | △ | △ | ✔ |

## Decision(决策)

### 选择的方案

在 sea-weir 数据库设计中,面对「方言分支治理 + 存量数据迁移 + 账务类型统一」的关注点,我们选择 **openGauss 单方言 + 表结构与 new-api 同名同语义**,而非继续三方言或重设 schema,以获得编译期校验与可控迁移,接受 SQLite 单机形态的移除。

### 决策理由

1. 单方言是 sqlx 编译期校验与 CI 门禁的前提
2. 同名同语义使新旧系统可并行比对(行为兼容验证、按表迁移)
3. openGauss 为公司标准,DBA/备份/监控基线现成

### 技术架构

类型映射(MySQL/new-api GORM → openGauss/Rust):

| 源 | 目标 | 说明 |
|---|---|---|
| int(Go int,额度) | BIGINT(i64) | 统一额度宽度,消除 user int32 级 vs channel int64 混用 |
| varchar(n) / text | VARCHAR(n) / TEXT | |
| JSON 字符串列 | JSONB | channel_info/setting/properties/param_override 等 |
| datetime(time.Time 表) | TIMESTAMPTZ | passkey/twofa/oauth 绑定等;业务表保留 BIGINT unix 秒兼容存量 |
| gorm.DeletedAt | TIMESTAMPTZ NULL + 部分唯一索引 `WHERE deleted_at IS NULL` | |
| decimal(10,6) | NUMERIC(10,6)(rust_decimal) | 金额 |
| abilities 复合主键 | PRIMARY KEY("group", model, channel_id) | 保留 |

保留字列 `"group"`、`"key"` 统一双引号;日志表独立连接池(`LOG_SQL_DSN`,默认同库)。

### 实施计划

1. migrations 编写 24 表 DDL(sqlx migrate),CI 对 openGauss 兼容实例跑 `sqlx migrate run`
2. 迁移工具:MySQL/PG → openGauss 全量导出导入 + 行数/抽样校验报告(复用公司 dbswitch 能力)
3. 双写期(可选灰度):旧系统只读冻结后切流
4. 验证:迁移前后 `users/tokens/channels/abilities/logs` 行数与关键聚合(额度总和)一致

## Consequences(后果)

### 正面影响

1. 方言分支与保留字转义代码全部消除
2. 额度类型统一 BIGINT,账务口径一致
3. JSONB 支持结构化查询(如按 channel_info 筛 multi-key 渠道)

### 负面影响

1. 失去 SQLite 零依赖部署形态
2. 迁移窗口需要停写或双写协调

### 缓解措施

1. 提供 docker-compose 一体化部署(openGauss 容器化)降低门槛
2. 迁移工具内置校验与回滚说明;灰度期旧库保留只读

### 长期影响

schema 与 new-api 对齐使得后续上游版本特性可参照移植;日志库独立 pool 为将来 ClickHouse 化留口。

## Notes

- `quota_data` 等聚合表按小时取整写入,迁移时按原样搬移即可
- openGauss 兼容 PG 协议,sqlx 用 postgres feature

## References

- `doc/system-design.md` §7
- `doc/architecture/er-diagram.puml`
- 相关 ADR:ADR-005(账务原子性依赖列类型统一)

---

**文档版本**: v1.0
**最后更新**: 2026-09-10
**下次评审**: 2026-12-10(3个月后)
**评审责任人**: Architecture Team Lead
