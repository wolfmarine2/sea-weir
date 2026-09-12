//! 令牌(API key)领域模型。对应 `tokens` 表。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub id: i64,
    pub user_id: i64,
    /// 48 位随机串,`sk-` 前缀不入库。列表接口按 `MaskTokenKey` 脱敏。
    #[serde(skip_serializing)]
    pub key: String,
    /// 1 启用 / 2 禁用 / 3 过期 / 4 额度耗尽
    pub status: i32,
    pub name: String,
    pub created_time: i64,
    pub accessed_time: i64,
    /// -1 表示永不过期。
    pub expired_time: i64,
    pub remain_quota: i64,
    pub unlimited_quota: bool,
    pub model_limits_enabled: bool,
    pub model_limits: String,
    pub allow_ips: Option<String>,
    pub used_quota: i64,
    /// 非空则覆盖用户分组。
    pub group: String,
    /// 跨分组重试,仅 auto 分组有效。
    pub cross_group_retry: bool,
}

/// 新建令牌的入参(id / 时间戳 / 已用量由仓库层生成)。
#[derive(Debug, Clone)]
pub struct NewToken {
    pub user_id: i64,
    /// 48 位随机串,`sk-` 前缀不入库。
    pub key: String,
    pub name: String,
    /// -1 表示永不过期。
    pub expired_time: i64,
    pub remain_quota: i64,
    pub unlimited_quota: bool,
    pub model_limits_enabled: bool,
    pub model_limits: String,
    pub allow_ips: Option<String>,
    /// 非空则覆盖用户分组。
    pub group: String,
    pub cross_group_retry: bool,
}

/// 令牌 key 脱敏。溯源:new-api `model/token.go` `MaskTokenKey`。
///
/// 规则:len ≤ 4 全掩码;len ≤ 8 → `前2 + "****" + 后2`;否则 `前4 + "**********" + 后4`。
pub fn mask_token_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let n = chars.len();
    if n == 0 {
        return String::new();
    }
    if n <= 4 {
        return "*".repeat(n);
    }
    if n <= 8 {
        let head: String = chars[..2].iter().collect();
        let tail: String = chars[n - 2..].iter().collect();
        return format!("{head}****{tail}");
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[n - 4..].iter().collect();
    format!("{head}**********{tail}")
}

#[cfg(test)]
mod tests {
    // TDD 入口(脱敏规则是契约,必须逐字节对齐):
    // - [x] mask_token_key("") == ""
    // - [x] len<=4 全掩码;len<=8 走 前2+****+后2;len>8 走 前4+**********+后4
    // - [ ] expired_time == -1 视为永不过期(校验逻辑待 relayer 落地)
    // - [x] 序列化不含 key 明文
    use super::*;

    #[test]
    fn mask_empty_is_empty() {
        assert_eq!(mask_token_key(""), "");
    }

    #[test]
    fn mask_short_is_all_stars() {
        assert_eq!(mask_token_key("abc"), "***");
        assert_eq!(mask_token_key("abcd"), "****");
    }

    #[test]
    fn mask_medium_is_head2_tail2() {
        assert_eq!(mask_token_key("abcdefgh"), "ab****gh");
    }

    #[test]
    fn mask_long_is_head4_tail4() {
        let key = "SFEwABCDEFGHIJKLMNOPQRSTUVWXYZbYt7";
        assert_eq!(mask_token_key(key), "SFEw**********bYt7");
    }

    #[test]
    fn serialized_token_hides_key() {
        let token = Token {
            id: 1,
            user_id: 1,
            key: "supersecret".into(),
            status: 1,
            name: "n".into(),
            created_time: 0,
            accessed_time: 0,
            expired_time: -1,
            remain_quota: 0,
            unlimited_quota: false,
            model_limits_enabled: false,
            model_limits: String::new(),
            allow_ips: None,
            used_quota: 0,
            group: String::new(),
            cross_group_retry: false,
        };
        let json = serde_json::to_value(&token).expect("可序列化");
        assert!(json.get("key").is_none(), "key 明文不得出现在响应中");
    }
}
