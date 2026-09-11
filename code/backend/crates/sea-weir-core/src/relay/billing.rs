//! 计费状态机。对应 C4 组件 `billing`。
//!
//! 设计见 doc/architecture/adr/ADR-005-billing-three-phase-and-idempotency.md、SEQ-003。
//!
//! ## 三段式
//! `PreConsumed → Settled | Refunded`,**类型层禁止非法迁移**(如 Settled 后 Refund)。
//!
//! ## 关键不变量
//! 1. 账务路径**同步落库**,无批量异步窗口(去掉了 new-api 的 `BATCH_UPDATE_ENABLED` 取舍);
//! 2. 额度扣减全部走**条件原子表达式**,影响 0 行 = 余额不足;
//! 3. 订阅预扣以 `request_id` 幂等,唯一约束兜底;
//! 4. 失败退款异步幂等;**已结算不退**;
//! 5. 统计口径(used_quota / request_count / quota_data)才允许批量合并。

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sea_weir_types::{dto::Usage, AppResult};

/// 计费会话状态。用类型参数编码,使非法迁移无法通过编译。
pub struct PreConsumed;
pub struct Settled;
pub struct Refunded;

/// 计费会话。
pub struct BillingSession<State> {
    pub request_id: String,
    pub user_id: i64,
    pub token_id: i64,
    pub pre_consumed: i64,
    pub funding: FundingSource,
    /// 信任额度旁路:命中时跳过预扣(仅同步渠道;任务类无旁路)。
    pub trusted: bool,
    _state: std::marker::PhantomData<State>,
}

/// 资金来源。用户计费偏好决定优先级与回退。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FundingSource {
    Wallet,
    Subscription,
}

/// 计价输入。
#[derive(Debug, Clone)]
pub struct PriceData {
    pub model_ratio: Decimal,
    pub group_ratio: Decimal,
    pub completion_ratio: Decimal,
    pub cache_ratio: Decimal,
    /// 基础缓存写入倍率(未分档部分)。
    pub create_cache_ratio: Decimal,
    /// Anthropic 5 分钟 TTL 缓存写入倍率。
    pub cache_creation_5m_ratio: Decimal,
    /// Anthropic 1 小时 TTL 缓存写入倍率。
    pub cache_creation_1h_ratio: Decimal,
    pub image_ratio: Decimal,
    pub audio_ratio: Decimal,
    /// 音频输入单价($/百万 token)。> 0 时 audio 从基数扣减并单独计价。
    pub audio_input_price: Decimal,
    /// 按次计费价格。为 Some 时走按次公式。
    pub model_price: Option<Decimal>,
    /// 阶梯计费表达式。
    pub tiered_expr: Option<String>,
    /// 其他倍率(任务类的时长/分辨率修正)。在主体计算后**连乘**。
    pub other_ratios: Vec<Decimal>,
}

/// 按量计费公式(CONTRACTS.md §6):
///
/// ```text
/// quota = (prompt − cache − cache_creation − image − audio
///          + cache×CacheRatio + cache_creation×CreateCacheRatio
///          + image×ImageRatio + completion×CompletionRatio + audio×AudioRatio)
///         × ModelRatio × GroupRatio + 工具附加费
/// ```
///
/// 精确运算用 `rust_decimal`。
///
/// **取整必须显式指定 half-away-from-zero**:
/// ```ignore
/// q.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
/// ```
/// `rust_decimal` 的 `.round()` 默认是银行家舍入,与 new-api 的
/// shopspring/decimal `Round(0)` 在 `x.5` 处差 1,会造成持续的账目偏差。
/// 由 test/src/unit_billing.rs 的 TC-UNI-BIL-005 守住。
pub fn calculate_quota(usage: &Usage, price: &PriceData) -> i64 {
    use rust_decimal::RoundingStrategy;
    use sea_weir_types::constants::QUOTA_PER_UNIT;
    use sea_weir_types::dto::relay::UsageSemantic;

    /// new-api 用 shopspring/decimal 的 `Round(0)`(half away from zero),
    /// 必须显式指定,不能用 `Decimal::round()`(银行家舍入)。
    fn round_half_away(value: Decimal) -> Decimal {
        value.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
    }

    let factor = price.model_ratio * price.group_ratio;
    let quota_per_unit = Decimal::from(QUOTA_PER_UNIT as i64);

    // ── 按次计费:quota = ModelPrice × QuotaPerUnit × GroupRatio ──
    if let Some(model_price) = price.model_price {
        let mut quota = model_price * quota_per_unit * price.group_ratio;
        for ratio in &price.other_ratios {
            quota *= *ratio;
        }
        let quota = round_half_away(quota);
        return quota.to_i64().unwrap_or(0);
    }

    // ── 按量计费 ──
    // total_tokens 为 0 时不收费(即使倍率非零)。
    if usage.total_tokens == 0 {
        return 0;
    }

    // usage 语义:显式标记 > 遗留 Claude 派生 > 默认 OpenAI。
    // (最终请求格式为 Claude 时链路由 pipeline 显式写入 usage_semantic。)
    let anthropic = matches!(usage.usage_semantic, Some(UsageSemantic::Anthropic))
        || usage.is_legacy_claude_derived();

    let prompt = Decimal::from(usage.prompt_tokens);
    let completion = Decimal::from(usage.completion_tokens);
    let cache = Decimal::from(usage.cached_tokens);
    let cache_creation = Decimal::from(usage.cache_creation_tokens);
    let cc_5m = Decimal::from(usage.claude_cache_creation_5m_tokens);
    let cc_1h = Decimal::from(usage.claude_cache_creation_1h_tokens);
    let image = Decimal::from(usage.image_tokens);
    let audio = Decimal::from(usage.audio_tokens);

    // audio 仅在配置了 audio_input_price 时才从基数扣减并单独计价。
    let audio_billed_separately = price.audio_input_price > Decimal::ZERO;
    let audio_deduction = if audio_billed_separately {
        audio
    } else {
        Decimal::ZERO
    };

    // 1. 基数:OpenAI 语义下 prompt 已含 cache/cache_creation,须扣减;
    //    Claude 语义下 Anthropic 分开上报,prompt 不含,不扣减。
    let base = if anthropic {
        prompt - image - audio_deduction
    } else {
        prompt - cache - cache_creation - image - audio_deduction
    };

    // 2. 分项计价
    let cache_q = cache * price.cache_ratio;
    let image_q = image * price.image_ratio;
    let create_q = if anthropic {
        // 缓存写入分档:剩余部分走基础倍率,5m / 1h 各自独立倍率。
        let remaining = (cache_creation - cc_5m - cc_1h).max(Decimal::ZERO);
        remaining * price.create_cache_ratio
            + cc_5m * price.cache_creation_5m_ratio
            + cc_1h * price.cache_creation_1h_ratio
    } else {
        cache_creation * price.create_cache_ratio
    };
    let compl_q = completion * price.completion_ratio;

    // 3. 主体 × 倍率;audio 在倍率之外单独计价。
    let mut quota = (base + cache_q + image_q + create_q + compl_q) * factor;
    if audio_billed_separately {
        // audio_price 为 $/百万 token。
        quota += price.audio_input_price / Decimal::from(1_000_000)
            * audio
            * price.group_ratio
            * quota_per_unit;
    }
    for ratio in &price.other_ratios {
        quota *= *ratio;
    }

    // 4. 下限与取整
    if factor != Decimal::ZERO && quota <= Decimal::ZERO {
        quota = Decimal::ONE;
    }
    let mut quota = round_half_away(quota);
    if factor != Decimal::ZERO && quota == Decimal::ZERO {
        quota = Decimal::ONE;
    }
    quota.to_i64().unwrap_or(0)
}

impl BillingSession<PreConsumed> {
    /// 预扣。
    ///
    /// 顺序:信任旁路判断 → 扣令牌额度 → 扣资金源
    /// (钱包原子扣减 / 订阅 FOR UPDATE + request_id 幂等)。
    /// **资金源扣减失败必须回滚令牌额度**。
    pub async fn pre_consume(_request_id: &str, _estimated: i64) -> AppResult<Self> {
        todo!("按 SEQ-003 预扣流程实现;失败回滚令牌额度")
    }

    /// 结算:按实际 usage 精确计费,差额补扣或退还。幂等。
    pub async fn settle(self, _actual: i64) -> AppResult<BillingSession<Settled>> {
        todo!("差额补退;幂等;失败不吞异常")
    }

    /// 失败退款。幂等。
    pub async fn refund(self) -> AppResult<BillingSession<Refunded>> {
        todo!("异步幂等退款")
    }
}

#[cfg(test)]
mod tests {
    // TDD 入口(账务正确性,本项目测试优先级最高的模块):
    //
    // 计价:
    // - [ ] calculate_quota 按量公式:逐项对照 new-api 同输入的输出(黄金用例表)
    // - [ ] 按次计费:quota == ModelPrice × QuotaPerUnit × GroupRatio
    // - [ ] 缓存/图片/音频 token 不被重复计入基数
    // - [ ] Decimal 精度:不出现浮点累积误差;取整方向与 Go 一致
    //
    // 状态机:
    // - [ ] 预扣成功 → 结算 → 账目守恒(预扣 == 结算 + 退还)
    // - [ ] 预扣时资金源不足 → 令牌额度已回滚(无悬挂扣减)
    // - [ ] 重复 settle 幂等,额度只变一次
    // - [ ] 已 settle 的会话 refund 不生效(类型层已禁止,补运行时断言)
    // - [ ] 同 request_id 并发预扣只扣一次
    // - [ ] 崩溃后对账任务能捞出悬挂预扣并退款
}
