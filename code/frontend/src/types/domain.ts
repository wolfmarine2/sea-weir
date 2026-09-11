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
