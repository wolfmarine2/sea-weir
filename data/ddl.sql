-- =============================================================================
-- sea-weir 权威 DDL(openGauss)
--
-- 依据:
--   doc/architecture/er-diagram.puml   实体与关系
--   doc/system-design.md §7            类型映射与数据特征
--   doc/architecture/adr/ADR-003       数据库选型与迁移(int→BIGINT、JSON→JSONB、
--                                      软删唯一约束用部分索引)
--
-- 约定:
--   1. 幂等:全部 CREATE TABLE/INDEX IF NOT EXISTS + COMMENT ON,可重复执行,
--      不含任何 DROP / TRUNCATE —— test/prod 的“无损升级”依赖这一点。
--   2. 表名/列名与 new-api 同名同语义,便于存量数据迁移与行为比对。
--   3. 额度/时间戳统一 BIGINT(unix 秒);JSON 列统一 JSONB。
--   4. 软删除表(deleted_at)的唯一约束一律使用部分索引 WHERE deleted_at IS NULL。
--   5. `"group"` 与 `"key"` 是 SQL 保留字,统一双引号引用。
--   6. 金额:NUMERIC(10,6)(订阅定价) / NUMERIC(12,2)(实付)。
-- =============================================================================

-- ============================ 1. users ============================
CREATE TABLE IF NOT EXISTS users (
    id                 BIGSERIAL     PRIMARY KEY,
    username           VARCHAR(64)   NOT NULL,
    password           VARCHAR(255)  NOT NULL DEFAULT '',
    display_name       VARCHAR(64)   NOT NULL DEFAULT '',
    role               BIGINT        NOT NULL DEFAULT 1,   -- 0 guest / 1 common / 10 admin / 100 root
    status             BIGINT        NOT NULL DEFAULT 1,   -- 1 启用 / 2 禁用
    email              VARCHAR(128)  NOT NULL DEFAULT '',
    github_id          VARCHAR(64)   NOT NULL DEFAULT '',
    discord_id         VARCHAR(64)   NOT NULL DEFAULT '',
    oidc_id            VARCHAR(64)   NOT NULL DEFAULT '',
    wechat_id          VARCHAR(64)   NOT NULL DEFAULT '',
    telegram_id        VARCHAR(64)   NOT NULL DEFAULT '',
    linux_do_id        VARCHAR(64)   NOT NULL DEFAULT '',
    access_token       CHAR(32),                            -- 系统访问令牌(非 sk- 令牌)
    quota              BIGINT        NOT NULL DEFAULT 0,    -- 钱包剩余额度
    used_quota         BIGINT        NOT NULL DEFAULT 0,
    request_count      BIGINT        NOT NULL DEFAULT 0,
    "group"            VARCHAR(64)   NOT NULL DEFAULT 'default',
    aff_code           VARCHAR(32),
    aff_count          BIGINT        NOT NULL DEFAULT 0,
    aff_quota          BIGINT        NOT NULL DEFAULT 0,
    aff_history_quota  BIGINT        NOT NULL DEFAULT 0,
    inviter_id         BIGINT        NOT NULL DEFAULT 0,
    setting            JSONB,                               -- 语言 / 通知 / 计费偏好
    remark             VARCHAR(255)  NOT NULL DEFAULT '',
    stripe_customer    VARCHAR(64)   NOT NULL DEFAULT '',
    created_at         BIGINT        NOT NULL DEFAULT 0,
    last_login_at      BIGINT        NOT NULL DEFAULT 0,
    deleted_at         TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_users_username     ON users (username)      WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uk_users_access_token ON users (access_token)  WHERE access_token IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uk_users_aff_code     ON users (aff_code)      WHERE aff_code IS NOT NULL;
CREATE INDEX        IF NOT EXISTS idx_users_email       ON users (email);
CREATE INDEX        IF NOT EXISTS idx_users_inviter     ON users (inviter_id);

COMMENT ON TABLE  users             IS '用户表:钱包额度、分组、邀请返利与 OAuth 绑定列';
COMMENT ON COLUMN users.quota       IS '钱包剩余额度(500000 quota = $1)';
COMMENT ON COLUMN users."group"     IS '计费分组;令牌分组非空时覆盖之';

-- ============================ 2. tokens ============================
CREATE TABLE IF NOT EXISTS tokens (
    id                   BIGSERIAL    PRIMARY KEY,
    user_id              BIGINT       NOT NULL,
    key                  CHAR(48)     NOT NULL,             -- 48 位随机串,sk- 前缀不入库
    status               BIGINT       NOT NULL DEFAULT 1,   -- 1 启用/2 禁用/3 过期/4 额度耗尽
    name                 VARCHAR(128) NOT NULL DEFAULT '',
    created_time         BIGINT       NOT NULL DEFAULT 0,
    accessed_time        BIGINT       NOT NULL DEFAULT 0,
    expired_time         BIGINT       NOT NULL DEFAULT -1,  -- -1 永不过期
    remain_quota         BIGINT       NOT NULL DEFAULT 0,
    unlimited_quota      BOOLEAN      NOT NULL DEFAULT FALSE,
    model_limits_enabled BOOLEAN      NOT NULL DEFAULT FALSE,
    model_limits         TEXT         NOT NULL DEFAULT '',
    allow_ips            TEXT,
    used_quota           BIGINT       NOT NULL DEFAULT 0,
    "group"              VARCHAR(64)  NOT NULL DEFAULT '',
    cross_group_retry    BOOLEAN      NOT NULL DEFAULT FALSE,
    deleted_at           TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_tokens_key  ON tokens (key) WHERE deleted_at IS NULL;
CREATE INDEX        IF NOT EXISTS idx_tokens_user ON tokens (user_id);

COMMENT ON TABLE  tokens              IS 'API 令牌(sk-token):模型白名单、IP 白名单、分组与额度';
COMMENT ON COLUMN tokens.expired_time IS '-1 表示永不过期';

-- ============================ 3. channels ============================
CREATE TABLE IF NOT EXISTS channels (
    id                    BIGSERIAL      PRIMARY KEY,
    type                  BIGINT         NOT NULL,              -- 渠道类型(ChannelType)→ ApiType
    key                   TEXT           NOT NULL,              -- 多 key 换行分隔(加密存储)
    openai_organization   VARCHAR(255)   NOT NULL DEFAULT '',
    test_model            VARCHAR(255)   NOT NULL DEFAULT '',
    status                BIGINT         NOT NULL DEFAULT 1,    -- 1 启用/2 手动禁用/3 自动禁用
    name                  VARCHAR(128)   NOT NULL DEFAULT '',
    weight                BIGINT         NOT NULL DEFAULT 0,    -- 同优先级内加权随机
    created_time          BIGINT         NOT NULL DEFAULT 0,
    test_time             BIGINT         NOT NULL DEFAULT 0,
    response_time         BIGINT         NOT NULL DEFAULT 0,    -- 最近一次测试耗时(ms)
    base_url              VARCHAR(255)   NOT NULL DEFAULT '',
    other                 TEXT           NOT NULL DEFAULT '',
    balance               DOUBLE PRECISION NOT NULL DEFAULT 0,  -- 上游余额(USD)
    balance_updated_time  BIGINT         NOT NULL DEFAULT 0,
    models                TEXT           NOT NULL DEFAULT '',   -- 逗号分隔
    "group"               VARCHAR(64)    NOT NULL DEFAULT 'default', -- 逗号分隔多分组
    used_quota            BIGINT         NOT NULL DEFAULT 0,
    model_mapping         JSONB,
    status_code_mapping   VARCHAR(1024)  NOT NULL DEFAULT '',
    priority              BIGINT         NOT NULL DEFAULT 0,
    auto_ban              BIGINT         NOT NULL DEFAULT 1,
    other_info            TEXT           NOT NULL DEFAULT '',   -- 禁用原因/时间等
    tag                   VARCHAR(64),
    setting               JSONB,                                -- 渠道级参数(Azure 版本等)
    param_override        JSONB,
    header_override       JSONB,
    remark                VARCHAR(255)   NOT NULL DEFAULT '',
    channel_info          JSONB,                                -- multi-key 状态与轮询下标
    settings              JSONB,                                -- 其他渠道设置
    created_at            BIGINT         NOT NULL DEFAULT 0,
    updated_at            BIGINT         NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_channels_tag  ON channels (tag);
CREATE INDEX IF NOT EXISTS idx_channels_name ON channels (name);

COMMENT ON TABLE  channels            IS '上游渠道:密钥、模型清单、分组、优先级/权重与 multi-key 状态';
COMMENT ON COLUMN channels."group"    IS '逗号分隔多分组;与 model 展开为 abilities 三元组';
COMMENT ON COLUMN channels.status     IS '3=自动禁用(可被渠道测试恢复);2=手动禁用(不可自动恢复)';

-- ============================ 4. abilities ============================
CREATE TABLE IF NOT EXISTS abilities (
    "group"      VARCHAR(64)  NOT NULL,
    model        VARCHAR(255) NOT NULL,
    channel_id   BIGINT       NOT NULL,
    enabled      BOOLEAN      NOT NULL DEFAULT TRUE,
    priority     BIGINT       NOT NULL DEFAULT 0,
    weight       BIGINT       NOT NULL DEFAULT 0,
    tag          VARCHAR(64),
    channel_type BIGINT       NOT NULL DEFAULT 0,
    PRIMARY KEY ("group", model, channel_id)
);
CREATE INDEX IF NOT EXISTS idx_abilities_priority ON abilities (priority);
CREATE INDEX IF NOT EXISTS idx_abilities_weight   ON abilities (weight);
CREATE INDEX IF NOT EXISTS idx_abilities_tag      ON abilities (tag);

COMMENT ON TABLE abilities IS '渠道能力三元组(分组×模型×渠道),选路的核心索引;由渠道变更同步维护';

-- ============================ 5. logs ============================
CREATE TABLE IF NOT EXISTS logs (
    id                BIGSERIAL    PRIMARY KEY,
    user_id           BIGINT       NOT NULL,
    created_at        BIGINT       NOT NULL DEFAULT 0,
    type              BIGINT       NOT NULL DEFAULT 0,   -- 0未知/1充值/2消费/3管理/4系统/5错误/6退款
    content           TEXT         NOT NULL DEFAULT '',
    username          VARCHAR(64)  NOT NULL DEFAULT '',
    token_name        VARCHAR(128) NOT NULL DEFAULT '',
    model_name        VARCHAR(128) NOT NULL DEFAULT '',
    quota             BIGINT       NOT NULL DEFAULT 0,
    prompt_tokens     BIGINT       NOT NULL DEFAULT 0,
    completion_tokens BIGINT       NOT NULL DEFAULT 0,
    use_time          BIGINT       NOT NULL DEFAULT 0,
    is_stream         BOOLEAN      NOT NULL DEFAULT FALSE,
    channel_id        BIGINT       NOT NULL DEFAULT 0,
    token_id          BIGINT       NOT NULL DEFAULT 0,
    "group"           VARCHAR(64)  NOT NULL DEFAULT '',
    ip                VARCHAR(64)  NOT NULL DEFAULT '',
    request_id        VARCHAR(64)  NOT NULL DEFAULT '',
    other             JSONB,                              -- 倍率明细/流状态等
    request_body      TEXT         NOT NULL DEFAULT '',
    response_body     TEXT         NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_logs_created_at_id ON logs (created_at, id);
CREATE INDEX IF NOT EXISTS idx_logs_user_id_id    ON logs (user_id, id);
CREATE INDEX IF NOT EXISTS idx_logs_request_id    ON logs (request_id);

COMMENT ON TABLE logs IS '日志(可独立日志库):消费/充值/管理/系统/错误/退款';

-- ============================ 6. options ============================
CREATE TABLE IF NOT EXISTS options (
    key   VARCHAR(128) PRIMARY KEY,
    value TEXT         NOT NULL DEFAULT ''
);
COMMENT ON TABLE options IS '系统配置 KV;`xxx_setting.yyy` 键走注册式分层配置组';

-- ============================ 7. redemptions ============================
CREATE TABLE IF NOT EXISTS redemptions (
    id            BIGSERIAL    PRIMARY KEY,
    user_id       BIGINT       NOT NULL,                 -- 创建者
    key           CHAR(32)     NOT NULL,
    status        BIGINT       NOT NULL DEFAULT 1,       -- 1 可用/2 禁用/3 已用
    name          VARCHAR(128) NOT NULL DEFAULT '',
    quota         BIGINT       NOT NULL DEFAULT 100,
    created_time  BIGINT       NOT NULL DEFAULT 0,
    redeemed_time BIGINT       NOT NULL DEFAULT 0,
    count         BIGINT       NOT NULL DEFAULT 0,
    used_user_id  BIGINT       NOT NULL DEFAULT 0,
    expired_time  BIGINT       NOT NULL DEFAULT 0,       -- 0 表示不过期
    deleted_at    TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_redemptions_key ON redemptions (key) WHERE deleted_at IS NULL;

-- ============================ 8. top_ups ============================
CREATE TABLE IF NOT EXISTS top_ups (
    id               BIGSERIAL     PRIMARY KEY,
    user_id          BIGINT        NOT NULL,
    amount           BIGINT        NOT NULL DEFAULT 0,   -- 入账额度
    money            NUMERIC(12,2) NOT NULL DEFAULT 0,   -- 实付金额
    trade_no         VARCHAR(255)  NOT NULL,             -- 支付流水号(幂等键)
    payment_method   VARCHAR(50)   NOT NULL DEFAULT '',
    payment_provider VARCHAR(50)   NOT NULL DEFAULT '',  -- epay/stripe/creem/waffo/waffo_pancake
    create_time      BIGINT        NOT NULL DEFAULT 0,
    complete_time    BIGINT        NOT NULL DEFAULT 0,
    status           VARCHAR(20)   NOT NULL DEFAULT 'pending' -- pending/success/failed/expired
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_top_ups_trade_no ON top_ups (trade_no);
CREATE INDEX        IF NOT EXISTS idx_top_ups_user     ON top_ups (user_id);

-- ============================ 9. checkins ============================
CREATE TABLE IF NOT EXISTS checkins (
    id            BIGSERIAL   PRIMARY KEY,
    user_id       BIGINT      NOT NULL,
    checkin_date  VARCHAR(10) NOT NULL,                  -- YYYY-MM-DD
    quota_awarded BIGINT      NOT NULL DEFAULT 0,
    created_at    BIGINT      NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_checkins_user_date ON checkins (user_id, checkin_date);

-- ============================ 10. two_fas ============================
CREATE TABLE IF NOT EXISTS two_fas (
    id              BIGSERIAL    PRIMARY KEY,
    user_id         BIGINT       NOT NULL,
    secret          VARCHAR(255) NOT NULL DEFAULT '',    -- TOTP 密钥,不出 API
    is_enabled      BOOLEAN      NOT NULL DEFAULT FALSE,
    failed_attempts BIGINT       NOT NULL DEFAULT 0,
    locked_until    TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ,
    deleted_at      TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_two_fas_user ON two_fas (user_id) WHERE deleted_at IS NULL;

-- ==================== 11. two_fa_backup_codes ====================
CREATE TABLE IF NOT EXISTS two_fa_backup_codes (
    id         BIGSERIAL    PRIMARY KEY,
    user_id    BIGINT       NOT NULL,
    code_hash  VARCHAR(255) NOT NULL,
    is_used    BOOLEAN      NOT NULL DEFAULT FALSE,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_two_fa_backup_user ON two_fa_backup_codes (user_id);

-- ==================== 12. passkey_credentials ====================
CREATE TABLE IF NOT EXISTS passkey_credentials (
    id               BIGSERIAL    PRIMARY KEY,
    user_id          BIGINT       NOT NULL,
    credential_id    VARCHAR(512) NOT NULL,
    public_key       TEXT         NOT NULL DEFAULT '',
    attestation_type VARCHAR(64)  NOT NULL DEFAULT '',
    aaguid           VARCHAR(64)  NOT NULL DEFAULT '',
    sign_count       BIGINT       NOT NULL DEFAULT 0,
    clone_warning    BOOLEAN      NOT NULL DEFAULT FALSE,
    user_present     BOOLEAN      NOT NULL DEFAULT FALSE,
    user_verified    BOOLEAN      NOT NULL DEFAULT FALSE,
    backup_eligible  BOOLEAN      NOT NULL DEFAULT FALSE,
    backup_state     BOOLEAN      NOT NULL DEFAULT FALSE,
    transports       TEXT         NOT NULL DEFAULT '',
    attachment       VARCHAR(32)  NOT NULL DEFAULT '',
    last_used_at     TIMESTAMPTZ,
    created_at       TIMESTAMPTZ,
    updated_at       TIMESTAMPTZ,
    deleted_at       TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_passkey_credential_id ON passkey_credentials (credential_id) WHERE deleted_at IS NULL;
CREATE INDEX        IF NOT EXISTS idx_passkey_user          ON passkey_credentials (user_id);

-- ==================== 13. user_oauth_bindings ====================
CREATE TABLE IF NOT EXISTS user_oauth_bindings (
    id               BIGSERIAL    PRIMARY KEY,
    user_id          BIGINT       NOT NULL,
    provider_id      BIGINT       NOT NULL,             -- 0 表示内置提供方(GitHub 等)
    provider_user_id VARCHAR(256) NOT NULL,
    created_at       TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_user_oauth_user_provider  ON user_oauth_bindings (user_id, provider_id);
CREATE UNIQUE INDEX IF NOT EXISTS uk_user_oauth_provider_user  ON user_oauth_bindings (provider_id, provider_user_id);

-- ==================== 14. custom_oauth_providers ====================
CREATE TABLE IF NOT EXISTS custom_oauth_providers (
    id                    BIGSERIAL    PRIMARY KEY,
    name                  VARCHAR(64)  NOT NULL,
    slug                  VARCHAR(64)  NOT NULL,
    icon                  VARCHAR(128) NOT NULL DEFAULT '',
    enabled               BOOLEAN      NOT NULL DEFAULT FALSE,
    client_id             VARCHAR(256) NOT NULL DEFAULT '',
    client_secret         VARCHAR(512) NOT NULL DEFAULT '',
    authorization_endpoint VARCHAR(512) NOT NULL DEFAULT '',
    token_endpoint        VARCHAR(512) NOT NULL DEFAULT '',
    user_info_endpoint    VARCHAR(512) NOT NULL DEFAULT '',
    scopes                VARCHAR(256) NOT NULL DEFAULT 'openid profile email',
    user_id_field         VARCHAR(128) NOT NULL DEFAULT 'sub',
    username_field        VARCHAR(128) NOT NULL DEFAULT 'preferred_username',
    display_name_field    VARCHAR(128) NOT NULL DEFAULT 'name',
    email_field           VARCHAR(128) NOT NULL DEFAULT 'email',
    well_known            VARCHAR(512) NOT NULL DEFAULT '',
    auth_style            BIGINT       NOT NULL DEFAULT 0,   -- 0 自动/1 params/2 header
    access_policy         JSONB,                             -- 访问条件树
    access_denied_message VARCHAR(512) NOT NULL DEFAULT '',
    created_at            TIMESTAMPTZ,
    updated_at            TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_custom_oauth_slug ON custom_oauth_providers (slug);

-- ==================== 15. prefill_groups ====================
CREATE TABLE IF NOT EXISTS prefill_groups (
    id           BIGSERIAL    PRIMARY KEY,
    name         VARCHAR(64)  NOT NULL,
    type         VARCHAR(32)  NOT NULL,                 -- model/tag/endpoint
    items        JSONB,
    description  VARCHAR(255) NOT NULL DEFAULT '',
    created_time BIGINT       NOT NULL DEFAULT 0,
    updated_time BIGINT       NOT NULL DEFAULT 0,
    deleted_at   TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_prefill_groups_name ON prefill_groups (name) WHERE deleted_at IS NULL;

-- ============================ 16. models ============================
CREATE TABLE IF NOT EXISTS models (
    id            BIGSERIAL    PRIMARY KEY,
    model_name    VARCHAR(128) NOT NULL,
    description   TEXT         NOT NULL DEFAULT '',
    icon          VARCHAR(128) NOT NULL DEFAULT '',
    tags          VARCHAR(255) NOT NULL DEFAULT '',
    vendor_id     BIGINT       NOT NULL DEFAULT 0,
    endpoints     JSONB,                                 -- 自定义端点覆盖
    status        BIGINT       NOT NULL DEFAULT 1,
    sync_official BIGINT       NOT NULL DEFAULT 0,
    name_rule     BIGINT       NOT NULL DEFAULT 0,       -- 0 精确/1 前缀/2 包含/3 后缀
    created_time  BIGINT       NOT NULL DEFAULT 0,
    updated_time  BIGINT       NOT NULL DEFAULT 0,
    deleted_at    TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_models_name    ON models (model_name) WHERE deleted_at IS NULL;
CREATE INDEX        IF NOT EXISTS idx_models_vendor ON models (vendor_id);

-- ============================ 17. vendors ============================
CREATE TABLE IF NOT EXISTS vendors (
    id           BIGSERIAL    PRIMARY KEY,
    name         VARCHAR(128) NOT NULL,
    description  TEXT         NOT NULL DEFAULT '',
    icon         VARCHAR(128) NOT NULL DEFAULT '',
    status       BIGINT       NOT NULL DEFAULT 1,
    created_time BIGINT       NOT NULL DEFAULT 0,
    updated_time BIGINT       NOT NULL DEFAULT 0,
    deleted_at   TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_vendors_name ON vendors (name) WHERE deleted_at IS NULL;

-- ============================ 18. quota_data ============================
CREATE TABLE IF NOT EXISTS quota_data (
    id          BIGSERIAL    PRIMARY KEY,
    user_id     BIGINT       NOT NULL,
    username    VARCHAR(64)  NOT NULL DEFAULT '',
    model_name  VARCHAR(128) NOT NULL DEFAULT '',
    created_at  BIGINT       NOT NULL DEFAULT 0,         -- 按小时取整
    token_used  BIGINT       NOT NULL DEFAULT 0,
    count       BIGINT       NOT NULL DEFAULT 0,
    quota       BIGINT       NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_quota_data_model_user ON quota_data (model_name, user_id);
CREATE INDEX IF NOT EXISTS idx_quota_data_created    ON quota_data (created_at);

-- ============================ 19. file_objects ============================
CREATE TABLE IF NOT EXISTS file_objects (
    id                BIGSERIAL    PRIMARY KEY,
    created_at        BIGINT       NOT NULL DEFAULT 0,
    updated_at        BIGINT       NOT NULL DEFAULT 0,
    user_id           BIGINT       NOT NULL DEFAULT 0,
    request_id        VARCHAR(64)  NOT NULL DEFAULT '',
    task_id           VARCHAR(64)  NOT NULL DEFAULT '',
    channel_id        BIGINT       NOT NULL DEFAULT 0,
    model_name        VARCHAR(128) NOT NULL DEFAULT '',
    direction         VARCHAR(16)  NOT NULL DEFAULT '',  -- request/response
    source_kind       VARCHAR(32)  NOT NULL DEFAULT '',
    source_hash       CHAR(64),
    media_type        VARCHAR(64)  NOT NULL DEFAULT '',
    mime_type         VARCHAR(128) NOT NULL DEFAULT '',
    original_filename VARCHAR(255) NOT NULL DEFAULT '',
    original_url      TEXT         NOT NULL DEFAULT '',
    bucket            VARCHAR(128) NOT NULL DEFAULT '',
    object_key        VARCHAR(512) NOT NULL DEFAULT '',
    etag              VARCHAR(128) NOT NULL DEFAULT '',
    sha256            CHAR(64)     NOT NULL DEFAULT '',
    size              BIGINT       NOT NULL DEFAULT 0,
    status            VARCHAR(32)  NOT NULL DEFAULT '',
    error_message     TEXT         NOT NULL DEFAULT '',
    extra             JSONB
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_file_objects_identity
    ON file_objects (task_id, direction, source_kind, source_hash)
    WHERE source_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_file_objects_user    ON file_objects (user_id);
CREATE INDEX IF NOT EXISTS idx_file_objects_request ON file_objects (request_id);

-- ============================ 20. setups ============================
CREATE TABLE IF NOT EXISTS setups (
    id             BIGSERIAL   PRIMARY KEY,
    version        VARCHAR(50) NOT NULL DEFAULT '',
    initialized_at BIGINT      NOT NULL DEFAULT 0
);
COMMENT ON TABLE setups IS '首装标记;存在记录即视为已初始化(见 /api/setup)';

-- ============================ 21. midjourneys ============================
CREATE TABLE IF NOT EXISTS midjourneys (
    id           BIGSERIAL    PRIMARY KEY,
    code         BIGINT       NOT NULL DEFAULT 0,
    user_id      BIGINT       NOT NULL,
    action       VARCHAR(40)  NOT NULL DEFAULT '',
    mj_id        VARCHAR(191) NOT NULL DEFAULT '',
    prompt       TEXT         NOT NULL DEFAULT '',
    prompt_en    TEXT         NOT NULL DEFAULT '',
    state        VARCHAR(32)  NOT NULL DEFAULT '',
    submit_time  BIGINT       NOT NULL DEFAULT 0,
    start_time   BIGINT       NOT NULL DEFAULT 0,
    finish_time  BIGINT       NOT NULL DEFAULT 0,
    image_url    TEXT         NOT NULL DEFAULT '',
    video_url    TEXT         NOT NULL DEFAULT '',
    video_urls   JSONB,
    status       VARCHAR(32)  NOT NULL DEFAULT '',
    progress     VARCHAR(32)  NOT NULL DEFAULT '',
    fail_reason  TEXT         NOT NULL DEFAULT '',
    channel_id   BIGINT       NOT NULL DEFAULT 0,
    quota        BIGINT       NOT NULL DEFAULT 0,
    buttons      JSONB,
    properties   JSONB
);
CREATE INDEX IF NOT EXISTS idx_midjourneys_user ON midjourneys (user_id);
CREATE INDEX IF NOT EXISTS idx_midjourneys_mj   ON midjourneys (mj_id);

-- ============================ 22. tasks ============================
CREATE TABLE IF NOT EXISTS tasks (
    id           BIGSERIAL    PRIMARY KEY,
    created_at   BIGINT       NOT NULL DEFAULT 0,
    updated_at   BIGINT       NOT NULL DEFAULT 0,
    task_id      VARCHAR(191) NOT NULL DEFAULT '',      -- 对外任务 id
    platform     VARCHAR(30)  NOT NULL DEFAULT '',
    user_id      BIGINT       NOT NULL DEFAULT 0,
    "group"      VARCHAR(64)  NOT NULL DEFAULT '',
    channel_id   BIGINT       NOT NULL DEFAULT 0,
    quota        BIGINT       NOT NULL DEFAULT 0,
    action       VARCHAR(40)  NOT NULL DEFAULT '',
    status       VARCHAR(20)  NOT NULL DEFAULT 'NOT_START',
    fail_reason  TEXT         NOT NULL DEFAULT '',
    submit_time  BIGINT       NOT NULL DEFAULT 0,
    start_time   BIGINT       NOT NULL DEFAULT 0,
    finish_time  BIGINT       NOT NULL DEFAULT 0,
    progress     VARCHAR(32)  NOT NULL DEFAULT '',
    properties   JSONB,                                  -- input / 模型名
    private_data JSONB,                                  -- 敏感:上游 key、计费快照
    data         JSONB,
    username     VARCHAR(64)  NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_tasks_task_id    ON tasks (task_id);
CREATE INDEX IF NOT EXISTS idx_tasks_platform   ON tasks (platform);
CREATE INDEX IF NOT EXISTS idx_tasks_user       ON tasks (user_id);
CREATE INDEX IF NOT EXISTS idx_tasks_channel    ON tasks (channel_id);
CREATE INDEX IF NOT EXISTS idx_tasks_action     ON tasks (action);
CREATE INDEX IF NOT EXISTS idx_tasks_finish     ON tasks (finish_time);

COMMENT ON TABLE  tasks              IS '异步任务(视频/音乐等);终态经 CAS 迁移,补差/退款';
COMMENT ON COLUMN tasks.private_data IS '敏感数据(上游密钥、计费上下文),查询接口不得返回';

-- ==================== 23. subscription_plans ====================
CREATE TABLE IF NOT EXISTS subscription_plans (
    id                        BIGSERIAL     PRIMARY KEY,
    title                     VARCHAR(128)  NOT NULL,
    subtitle                  VARCHAR(255)  NOT NULL DEFAULT '',
    price_amount              NUMERIC(10,6) NOT NULL DEFAULT 0,
    currency                  VARCHAR(8)    NOT NULL DEFAULT 'USD',
    duration_unit             VARCHAR(16)   NOT NULL DEFAULT 'month',
    duration_value            BIGINT        NOT NULL DEFAULT 1,
    custom_seconds            BIGINT        NOT NULL DEFAULT 0,
    enabled                   BOOLEAN       NOT NULL DEFAULT TRUE,   -- false 时不对外展示
    sort_order                BIGINT        NOT NULL DEFAULT 0,
    stripe_price_id           VARCHAR(128)  NOT NULL DEFAULT '',
    creem_product_id          VARCHAR(128)  NOT NULL DEFAULT '',
    max_purchase_per_user     BIGINT        NOT NULL DEFAULT 0,      -- 0 不限
    upgrade_group             VARCHAR(64)   NOT NULL DEFAULT '',
    total_amount              BIGINT        NOT NULL DEFAULT 0,      -- 0 表示无限
    quota_reset_period        VARCHAR(16)   NOT NULL DEFAULT 'never',
    quota_reset_custom_seconds BIGINT       NOT NULL DEFAULT 0,
    created_at                BIGINT        NOT NULL DEFAULT 0,
    updated_at                BIGINT        NOT NULL DEFAULT 0,
    deleted_at                TIMESTAMPTZ
);

-- ==================== 24. subscription_orders ====================
CREATE TABLE IF NOT EXISTS subscription_orders (
    id               BIGSERIAL     PRIMARY KEY,
    user_id          BIGINT        NOT NULL,
    plan_id          BIGINT        NOT NULL,
    money            NUMERIC(12,2) NOT NULL DEFAULT 0,
    trade_no         VARCHAR(255)  NOT NULL,              -- 幂等键
    payment_method   VARCHAR(50)   NOT NULL DEFAULT '',
    payment_provider VARCHAR(50)   NOT NULL DEFAULT '',
    status           VARCHAR(20)   NOT NULL DEFAULT 'pending',
    create_time      BIGINT        NOT NULL DEFAULT 0,
    complete_time    BIGINT        NOT NULL DEFAULT 0,
    provider_payload TEXT          NOT NULL DEFAULT ''
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_sub_orders_trade_no ON subscription_orders (trade_no);
CREATE INDEX        IF NOT EXISTS idx_sub_orders_user    ON subscription_orders (user_id);
CREATE INDEX        IF NOT EXISTS idx_sub_orders_plan    ON subscription_orders (plan_id);

-- ==================== 25. user_subscriptions ====================
CREATE TABLE IF NOT EXISTS user_subscriptions (
    id              BIGSERIAL    PRIMARY KEY,
    user_id         BIGINT       NOT NULL,
    plan_id         BIGINT       NOT NULL,
    amount_total    BIGINT       NOT NULL DEFAULT 0,
    amount_used     BIGINT       NOT NULL DEFAULT 0,
    start_time      BIGINT       NOT NULL DEFAULT 0,
    end_time        BIGINT       NOT NULL DEFAULT 0,
    status          VARCHAR(32)  NOT NULL DEFAULT 'active', -- active/expired/cancelled
    source          VARCHAR(32)  NOT NULL DEFAULT 'order',  -- order/admin
    last_reset_time BIGINT       NOT NULL DEFAULT 0,
    next_reset_time BIGINT       NOT NULL DEFAULT 0,
    upgrade_group   VARCHAR(64)  NOT NULL DEFAULT '',
    prev_user_group VARCHAR(64)  NOT NULL DEFAULT '',
    created_at      BIGINT       NOT NULL DEFAULT 0,
    updated_at      BIGINT       NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_user_sub_active      ON user_subscriptions (user_id, status, end_time);
CREATE INDEX IF NOT EXISTS idx_user_sub_next_reset  ON user_subscriptions (next_reset_time);

COMMENT ON TABLE user_subscriptions IS '用户订阅;预扣按 end_time 升序逐个消耗(先到期先用)';

-- ============ 26. subscription_pre_consume_records ============
CREATE TABLE IF NOT EXISTS subscription_pre_consume_records (
    id                   BIGSERIAL   PRIMARY KEY,
    request_id           VARCHAR(64) NOT NULL,            -- 幂等键(与中继 request_id 一致)
    user_id              BIGINT      NOT NULL,
    user_subscription_id BIGINT      NOT NULL,
    pre_consumed         BIGINT      NOT NULL DEFAULT 0,
    status               VARCHAR(32) NOT NULL DEFAULT 'consumed', -- consumed/refunded
    created_at           BIGINT      NOT NULL DEFAULT 0,
    updated_at           BIGINT      NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_sub_pre_consume_request ON subscription_pre_consume_records (request_id);
CREATE INDEX        IF NOT EXISTS idx_sub_pre_consume_user   ON subscription_pre_consume_records (user_id);
CREATE INDEX        IF NOT EXISTS idx_sub_pre_consume_sub    ON subscription_pre_consume_records (user_subscription_id);
CREATE INDEX        IF NOT EXISTS idx_sub_pre_consume_updated ON subscription_pre_consume_records (updated_at);

COMMENT ON TABLE subscription_pre_consume_records IS '订阅预扣台账;崩溃对账据此退款(ADR-005)';

-- =============================================================================
-- 迁移提示(存量 new-api → openGauss):
--   new-api 自增列为 INTEGER,需按 ADR-003 提升为 BIGINT;JSON 文本列导入后
--   用 `USING col::jsonb` 转为 JSONB;软删唯一约束需重建为部分索引。
--   详见 data/README.md「存量数据迁移」。
-- =============================================================================
