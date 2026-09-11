//! 订阅领域模型。对应 `subscription_plans` / `user_subscriptions` /
//! `subscription_orders` / `subscription_pre_consume_records` 四表。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPlan {
    pub id: i64,
    pub name: String,
    pub price: rust_decimal::Decimal,
    pub quota: i64,
    pub period_days: i32,
    /// 订阅期间用户升级到的分组。
    pub group: String,
    pub enabled: bool,
    pub max_purchase_per_user: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSubscription {
    pub id: i64,
    pub user_id: i64,
    pub plan_id: i64,
    pub remain_quota: i64,
    pub start_time: i64,
    /// 预扣时按 end_time 升序 FOR UPDATE 逐个扣减(先到期先用)。
    pub end_time: i64,
    pub status: i32,
}

/// 订阅预扣幂等记录。`request_id` 唯一约束是并发兜底。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPreConsumeRecord {
    pub id: i64,
    pub request_id: String,
    pub user_subscription_id: i64,
    pub amount: i64,
    pub settled: bool,
}
