-- =============================================================================
-- sea-weir 基础种子(幂等)
--
-- 调用方需传入 psql 变量:
--   :root_password_hash  root 口令的 bcrypt 哈希(空串则跳过 root 创建)
--   :root_aff_code       邀请码(默认 rootaff)
--
-- 幂等性:全部 INSERT ... WHERE NOT EXISTS,可重复执行不产生重复数据。
-- 说明:业务配置(ModelRatio / 分组 / OAuth 开关等)由应用启动时写入 options
--       默认值,本文件不重复;此处只保证「已初始化标记」与「可登录的 root 账号」。
-- =============================================================================

-- 1. 首装标记:存在即视为已初始化,避免应用再走 /api/setup 向导
INSERT INTO setups (id, version, initialized_at)
SELECT 1, 'v0.1.0', EXTRACT(EPOCH FROM now())::BIGINT
WHERE NOT EXISTS (SELECT 1 FROM setups WHERE id = 1);

-- 2. root 账号(仅当提供了哈希且不存在同名用户时创建)
--    role=100(RootAuth),quota=100000000(与 new-api 首装初始额度一致)
INSERT INTO users (
    username, password, display_name, role, status, email,
    quota, used_quota, request_count, "group", aff_code, created_at, last_login_at
)
SELECT
    'root', :'root_password_hash', 'Root', 100, 1, '',
    100000000, 0, 0, 'default', :'root_aff_code',
    EXTRACT(EPOCH FROM now())::BIGINT, 0
WHERE :'root_password_hash' <> ''
  AND NOT EXISTS (SELECT 1 FROM users WHERE username = 'root' AND deleted_at IS NULL);

-- 3. 输出结果(便于脚本日志确认)
\echo '  seed-base: setups 与 root 已就绪(已存在则保持不变)'
