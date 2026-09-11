//! L1 单元 — 令牌脱敏。用例文档:`cases/01-unit-types.md`
//!
//! 脱敏格式是**契约**:前端列表直接展示这个字符串。
//! 审查中发现设计文档曾写成 `sk-****` 前缀形式,与实际实现不符。
//!
//! 溯源:new-api `model/token.go:MaskTokenKey`;TEST-VECTORS.md §2

use pretty_assertions::assert_eq;
use sea_weir_types::domain::token::mask_token_key;

/// TC-UNI-TYP-010-BND:脱敏三段规则的全部边界。
///
/// 规则:len ≤ 4 → 全掩码;len ≤ 8 → 前2+"****"+后2;否则 → 前4+"**********"+后4
#[rstest::rstest]
#[case("",          "")]
#[case("a",         "*")]
#[case("ab",        "**")]
#[case("abcd",      "****")]
#[case("abcde",     "ab****de")]
#[case("abcdefgh",  "ab****gh")]
#[case("abcdefghi", "abcd**********fghi")]
fn tc_uni_typ_010_bnd_mask_token_key(#[case] input: &str, #[case] expected: &str) {
    assert_eq!(mask_token_key(input), expected, "input={input:?}");
}

/// TC-UNI-TYP-010-POS:真实长度(48 位)令牌的脱敏结果长度恒为 18。
#[test]
fn tc_uni_typ_010_pos_real_length_key() {
    let key = "a".repeat(44) + "wxyz";
    assert_eq!(key.len(), 48);
    let masked = mask_token_key(&key);
    assert_eq!(masked.len(), 4 + 10 + 4, "前4 + 10星 + 后4 = 18");
    assert!(masked.starts_with("aaaa"));
    assert!(masked.ends_with("wxyz"));
    assert!(!masked.contains(&key), "脱敏结果不得包含完整原文");
}

/// TC-UNI-TYP-011-NEG:脱敏结果不得泄漏中间段。
#[test]
fn tc_uni_typ_011_neg_no_middle_leak() {
    let key = "PREFIX_SECRET_MIDDLE_PART_SUFFIX";
    let masked = mask_token_key(key);
    assert!(!masked.contains("SECRET"), "中间段必须被完全掩盖:{masked}");
    assert!(!masked.contains("MIDDLE"), "中间段必须被完全掩盖:{masked}");
}
