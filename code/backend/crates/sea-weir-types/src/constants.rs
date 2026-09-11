//! 业务常量。取值全部与 new-api 现行实现一一对应,改动即破坏契约兼容。
//!
//! 溯源见 doc/architecture/adr-review-report.md「源码复核明细」。

/// 额度单位:500000 quota = $1(模型倍率 1 = $0.002/1K tokens)。
/// 溯源:new-api `common/constants.go:22`。
pub const QUOTA_PER_UNIT: f64 = 500_000.0;

/// 用户角色。溯源:new-api `common/constants.go:148-151`。
pub mod role {
    pub const GUEST: i32 = 0;
    pub const COMMON: i32 = 1;
    pub const ADMIN: i32 = 10;
    pub const ROOT: i32 = 100;

    /// 角色闸门:UserAuth ≥ 1、AdminAuth ≥ 10、RootAuth ≥ 100。
    pub fn is_valid(role: i32) -> bool {
        matches!(role, GUEST | COMMON | ADMIN | ROOT)
    }
}

/// 日志类型。溯源:new-api `model/log.go:46-52`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(i32)]
pub enum LogType {
    Unknown = 0,
    Topup = 1,
    Consume = 2,
    Manage = 3,
    System = 4,
    Error = 5,
    Refund = 6,
}

/// 通用启停状态。
pub mod status {
    pub const ENABLED: i32 = 1;
    pub const DISABLED: i32 = 2;
    /// 渠道专用:自动禁用(与手动禁用区分,影响自动恢复)。
    pub const AUTO_DISABLED: i32 = 3;
}

/// 令牌状态。
pub mod token_status {
    pub const ENABLED: i32 = 1;
    pub const DISABLED: i32 = 2;
    pub const EXPIRED: i32 = 3;
    pub const EXHAUSTED: i32 = 4;
}

/// 兑换码状态。
pub mod redemption_status {
    pub const UNUSED: i32 = 1;
    pub const DISABLED: i32 = 2;
    pub const USED: i32 = 3;
}

/// 令牌 key 长度(不含 `sk-` 前缀,前缀不入库)。
/// 溯源:new-api `common/utils.go:251-253`。
pub const TOKEN_KEY_LEN: usize = 48;

/// 默认时间与频率(全部可配,此处为 fallback 默认值)。
pub mod defaults {
    /// 渠道索引/Option 同步兜底轮询。溯源:`common/init.go:102`(SYNC_FREQUENCY=60)。
    pub const SYNC_FREQUENCY_SECS: u64 = 60;
    /// 异步任务轮询周期。溯源:`service/task_polling.go:93`。
    pub const TASK_POLL_INTERVAL_SECS: u64 = 15;
    /// SSE ping 保活。溯源:`relay/helper/stream_scanner.go:27`。
    pub const SSE_PING_INTERVAL_SECS: u64 = 10;
    /// 缓存 TTL 兜底(用户/令牌)。
    pub const CACHE_TTL_SECS: u64 = 60;
    /// 邮箱验证码有效期。
    pub const EMAIL_CODE_TTL_SECS: u64 = 600;
    /// 敏感操作凭证有效期。
    pub const SECURE_CREDENTIAL_TTL_SECS: u64 = 600;
    /// 会话有效期。
    pub const SESSION_TTL_DAYS: i64 = 30;
    /// 渠道亲和性缓存默认 TTL。
    pub const AFFINITY_TTL_SECS: u64 = 3600;
}

/// 重试与自动禁用的默认状态码规则。
/// 溯源:new-api `setting/operation_setting/status_code_ranges.go:17-33`。
pub mod retry_policy {
    /// 默认自动禁用状态码区间:仅 401。
    pub const DEFAULT_DISABLE_RANGES: &[(u16, u16)] = &[(401, 401)];

    /// 默认可重试状态码区间 —— 等价于「1xx/3xx/4xx(除 400、408)/5xx(除 504、524),2xx 不重试」。
    pub const DEFAULT_RETRY_RANGES: &[(u16, u16)] = &[
        (100, 199),
        (300, 399),
        (401, 407),
        (409, 499),
        (500, 503),
        (505, 523),
        (525, 599),
    ];

    /// 永不重试的状态码(优先级高于区间配置)。
    pub const ALWAYS_SKIP_RETRY: &[u16] = &[504, 524];
}

#[cfg(test)]
mod tests {
    // TDD 入口:常量取值回归。
    // 这些测试的价值在于「锁死契约」—— 一旦有人改动取值,测试必须红。
    //
    // 待补:
    // - [ ] role::is_valid 对全部四个角色返回 true、对任意其他值返回 false
    // - [ ] LogType 的 i32 表示与 new-api 0..=6 完全一致
    // - [ ] DEFAULT_RETRY_RANGES 展开后不含 400/408/504/524,且不含任何 2xx
    // - [ ] QUOTA_PER_UNIT 换算:500000 quota == $1
}
