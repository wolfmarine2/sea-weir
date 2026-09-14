-- =============================================================================
-- sea-weir 数据库引导(需超级用户执行一次)
--
-- 用途:创建业务角色与数据库。表结构由 ddl.sql 负责(幂等、无损)。
-- 执行:
--   psql -h <host> -p <port> -U <superuser> -d postgres \
--        -v app_password="$SEAWEIR_DB_PASSWORD" -f data/00-init-database.sql
--
-- 说明:
--   - CREATE DATABASE 不能运行在事务块内,故用 psql 的 \gexec 按需执行;
--   - 口令通过 psql 变量 :'app_password' 注入,不写入仓库;
--   - 角色/库名与 config 中 database.dsn 保持一致(默认 sea_weir);
--   - 必须 PG 兼容模式:'A'(Oracle 兼容)模式下 '' 等同 NULL,写 NOT NULL 列会失败,
--     且兼容模式建库后不可修改,已按 'A' 建的库需重建;
--   - DBCOMPATIBILITY 字面量各版本支持不一:'PG' 与 'D' 均表示 PostgreSQL,
--     若本版本不认 'PG',把下面的 'PG' 换成 'D' 再执行。
-- =============================================================================

\set ON_ERROR_STOP on

-- 1. 业务角色(存在则跳过)
SELECT format('CREATE ROLE sea_weir LOGIN PASSWORD %L', :'app_password')
FROM (VALUES (1)) AS one
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'sea_weir')
\gexec

-- 2. 业务库(存在则跳过)
SELECT format('CREATE DATABASE sea_weir OWNER sea_weir ENCODING %L DBCOMPATIBILITY %L', 'UTF8', 'PG')
FROM (VALUES (1)) AS one
WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'sea_weir')
\gexec

-- 3. 归属确认(库已存在时同样执行,幂等)
SELECT 'ALTER DATABASE sea_weir OWNER TO sea_weir'
FROM pg_database
WHERE datname = 'sea_weir'
\gexec

\echo '数据库引导完成: role=sea_weir db=sea_weir; 随后执行 ddl.sql(可用 data/cicd.sh 自动完成)'
