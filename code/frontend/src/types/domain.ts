/**
 * 领域类型。与后端 `sea_weir_types::domain` 逐字段对应。
 *
 * TODO(TDD): User / Token / Channel / Log / Task / SubscriptionPlan 等。
 * 落地时的验收标准:用从 new-api 录制的真实响应做类型断言,字段不缺不改名。
 */

/** 角色。0 guest / 1 common / 10 admin / 100 root。 */
export const ROLE = { GUEST: 0, COMMON: 1, ADMIN: 10, ROOT: 100 } as const;

/** 额度单位:500000 quota = $1。 */
export const QUOTA_PER_UNIT = 500_000;

/** 日志类型。 */
export const LOG_TYPE = {
  UNKNOWN: 0, TOPUP: 1, CONSUME: 2, MANAGE: 3, SYSTEM: 4, ERROR: 5, REFUND: 6,
} as const;

/**
 * 用户。与后端 `sea_weir_types::domain::User` 一致。
 * `password` 永不下发(后端 `#[serde(skip_serializing)]`)。
 */
export interface User {
  id: number;
  username: string;
  display_name: string;
  /** 0 guest / 1 common / 10 admin / 100 root */
  role: number;
  /** 1 启用 / 2 禁用 */
  status: number;
  email: string;
  quota: number;
  used_quota: number;
  request_count: number;
  group: string;
  aff_code: string;
  setting: Record<string, unknown>;
  created_at: number;
  last_login_at: number;
}

/** `GET /api/status` 载荷。前端 status store 的唯一来源。 */
export interface SystemStatus {
  system_name: string;
  logo: string;
  version: string;
  start_time: number;
  setup: boolean;
  db_ready: boolean;
  [key: string]: unknown;
}

/** `GET /api/setup` 载荷。 */
export interface SetupInfo {
  /** 是否已完成初始化 */
  status: boolean;
  /** root 账号是否已存在 */
  root_init: boolean;
  database_type: string;
}

/**
 * 令牌(API key)。与后端 `sea_weir_types::domain::Token` 一致。
 * 列表/创建响应中的 `key` 已脱敏,仅 `POST /api/token/:id/key` 返回明文。
 */
export interface TokenItem {
  id: number;
  user_id: number;
  key: string;
  /** 1 启用 / 2 禁用 / 3 过期 / 4 额度耗尽 */
  status: number;
  name: string;
  created_time: number;
  accessed_time: number;
  /** -1 表示永不过期 */
  expired_time: number;
  remain_quota: number;
  unlimited_quota: boolean;
  model_limits_enabled: boolean;
  model_limits: string;
  allow_ips: string | null;
  used_quota: number;
  group: string;
  cross_group_retry: boolean;
}

/**
 * 渠道。与后端 `sea_weir_types::domain::Channel` 一致;`key` 不下发,
 * 列表额外带 `key_count`(多 key 数量)。
 */
export interface ChannelItem {
  id: number;
  type: number;
  name: string;
  /** 1 启用 / 2 手动禁用 / 3 自动禁用 */
  status: number;
  weight: number;
  priority: number;
  group: string;
  models: string;
  base_url: string | null;
  balance: number;
  used_quota: number;
  auto_ban: number;
  tag: string | null;
  setting: unknown;
  created_time: number;
  balance_updated_time: number;
  test_time: number;
  response_time: number;
  key_count: number;
  [key: string]: unknown;
}

/** 渠道创建/更新入参。`key` 更新时可省略(沿用原密钥)。 */
export interface ChannelPayload {
  type: number;
  name: string;
  models: string;
  group: string;
  key?: string | undefined;
  base_url?: string | undefined;
  status?: number | undefined;
  weight?: number | undefined;
  priority?: number | undefined;
  auto_ban?: number | undefined;
  test_model?: string | undefined;
  tag?: string | undefined;
  /** 上游请求参数改写(见后端 apply_param_override)。 */
  param_override?: unknown;
  /** 上游请求头改写;值中 `{api_key}` 替换为渠道密钥。 */
  header_override?: unknown;
}

/** 消费日志。与后端 `sea_weir_types::domain::Log` 一致。 */
export interface LogItem {
  id: number;
  user_id: number;
  created_at: number;
  /** 0 未知 / 1 充值 / 2 消费 / 3 管理 / 4 系统 / 5 错误 / 6 退款 */
  type: number;
  content: string;
  username: string;
  token_name: string;
  model_name: string;
  quota: number;
  prompt_tokens: number;
  completion_tokens: number;
  use_time: number;
  is_stream: boolean;
  channel_id: number | null;
  token_id: number | null;
  group: string | null;
  ip: string | null;
  request_id: string | null;
}
