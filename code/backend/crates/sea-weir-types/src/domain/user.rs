//! 用户领域模型。对应 `users` 表(doc/system-design.md §7.2)。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    /// bcrypt 哈希。**永不出现在任何 API 响应中**。
    ///
    /// `default` 使缓存往返安全:序列化时跳过,反序列化(如从 Valkey 读回)默认为空串。
    #[serde(skip_serializing, default)]
    pub password: String,
    pub display_name: String,
    /// 0 guest / 1 common / 10 admin / 100 root
    pub role: i32,
    pub status: i32,
    pub email: String,
    pub github_id: String,
    pub discord_id: String,
    pub oidc_id: String,
    pub wechat_id: String,
    pub telegram_id: String,
    pub linux_do_id: String,
    /// 系统访问令牌(32 位),非 sk- 令牌。
    pub access_token: Option<String>,
    /// 钱包剩余额度。
    pub quota: i64,
    pub used_quota: i64,
    pub request_count: i64,
    pub group: String,
    pub aff_code: String,
    pub aff_count: i64,
    pub aff_quota: i64,
    pub aff_history_quota: i64,
    pub inviter_id: Option<i64>,
    pub stripe_customer: String,
    /// JSONB:语言/通知/计费偏好。
    pub setting: serde_json::Value,
    pub remark: Option<String>,
    pub created_at: i64,
    pub last_login_at: i64,
}

impl User {
    pub fn is_admin(&self) -> bool {
        self.role >= crate::constants::role::ADMIN
    }
    pub fn is_root(&self) -> bool {
        self.role >= crate::constants::role::ROOT
    }
    pub fn is_enabled(&self) -> bool {
        self.status == crate::constants::status::ENABLED
    }
}

/// 计费偏好。溯源:new-api `common/str.go:112`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingPreference {
    WalletOnly,
    SubscriptionOnly,
    WalletFirst,
    SubscriptionFirst,
}

#[cfg(test)]
mod tests {
    // TDD 入口:
    // - [ ] 序列化后 password 字段不存在
    // - [ ] 字段名集合与 new-api `GET /api/user/self` 响应逐字段一致(snake_case)
    // - [ ] is_admin/is_root 的角色边界(9/10/99/100)
    // - [ ] BillingPreference 四个值的序列化文本与 Go 侧字符串一致
}
