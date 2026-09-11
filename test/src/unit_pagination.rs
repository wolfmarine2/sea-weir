//! L1 单元 — 分页。用例文档:`cases/01-unit-types.md`
//!
//! **审查发现的 P1 偏差**:v1.0 文档写「`p` 页码 0 起」,而 new-api 实际是 1 起
//! (`start_idx = (page-1)*page_size`,且 `p < 1` 归一为 1)。
//! 本文件负责永久守住这条,防止改回去。
//!
//! 溯源:new-api `common/page_info.go:41-71`;TEST-VECTORS.md §3

use pretty_assertions::assert_eq;
use sea_weir_types::dto::common::PageQuery;

fn q(p: i64, size: i64) -> PageQuery {
    PageQuery { page: p, page_size: size }
}

/// TC-UNI-TYP-020-BND:页码归一化与偏移计算。★
#[rstest::rstest]
#[case(0,  10, 1, 0)]   // 缺省/0 → 第 1 页
#[case(1,  10, 1, 0)]
#[case(2,  10, 2, 10)]
#[case(3,  20, 3, 40)]
#[case(-5, 10, 1, 0)]   // 负数归一
fn tc_uni_typ_020_bnd_page_base_is_one(
    #[case] p: i64,
    #[case] size: i64,
    #[case] expect_page: i64,
    #[case] expect_start: i64,
) {
    let pq = q(p, size);
    assert_eq!(pq.normalized_page(), expect_page, "p={p} 的归一化页码");
    assert_eq!(
        pq.start_idx(),
        expect_start,
        "p={p} 的偏移。若得到 {p}*{size},说明按 0 起实现了,与 new-api 不符"
    );
}

/// TC-UNI-TYP-021-POS:`page_size` 别名 `ps` / `size` 均可反序列化。
///
/// new-api 为兼容历史前端保留了三个参数名,客户端可能用任一个。
#[rstest::rstest]
#[case(r#"{"p":2,"page_size":25}"#, 25)]
#[case(r#"{"p":2,"ps":25}"#,        25)]
#[case(r#"{"p":2,"size":25}"#,      25)]
fn tc_uni_typ_021_pos_page_size_aliases(#[case] json: &str, #[case] expected: i64) {
    let pq: PageQuery = serde_json::from_str(json).expect("反序列化失败");
    assert_eq!(pq.page_size, expected);
}

/// TC-UNI-TYP-022-POS:分页响应形状恒为 `{items,total,page,page_size}`。
#[test]
fn tc_uni_typ_022_pos_page_info_shape() {
    use sea_weir_types::dto::common::PageInfo;
    let pi = PageInfo { items: vec![1, 2, 3], total: 42, page: 2, page_size: 3 };
    let v = serde_json::to_value(&pi).expect("序列化失败");
    let obj = v.as_object().expect("应为对象");

    let mut keys: Vec<_> = obj.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["items", "page", "page_size", "total"]);
    assert_eq!(obj["page"], 2);
    assert_eq!(obj["page_size"], 3, "字段名是 page_size,不是 pageSize");
}
