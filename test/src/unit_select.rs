//! L1 单元 — 渠道选路。用例文档:`cases/03-unit-select-retry.md`
//!
//! 溯源:ADR-006、TEST-VECTORS.md §5

use pretty_assertions::assert_eq;
use sea_weir_core::relay::select::{priority_tiers, weighted_pick};
use sea_weir_types::domain::Ability;

fn ability(channel_id: i64, priority: i64, weight: i64) -> Ability {
    Ability {
        group: "default".into(),
        model: "gpt-4".into(),
        channel_id,
        enabled: true,
        priority,
        weight,
        tag: None,
    }
}

/// 测试集:A(100,5) B(100,0) C(50,10) D(50,1) E(10,0)
fn candidates() -> Vec<Ability> {
    vec![
        ability(1, 100, 5),
        ability(2, 100, 0),
        ability(3, 50, 10),
        ability(4, 50, 1),
        ability(5, 10, 0),
    ]
}

/// TC-UNI-SEL-001-POS:优先级分档,档间按 priority 降序。
#[test]
fn tc_uni_sel_001_pos_priority_tiers_descending() {
    let c = candidates();
    let tiers = priority_tiers(&c);

    assert_eq!(tiers.len(), 3, "三个不同的 priority 值应聚成三档");
    assert_eq!(tiers[0].iter().map(|a| a.channel_id).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(tiers[1].iter().map(|a| a.channel_id).collect::<Vec<_>>(), vec![3, 4]);
    assert_eq!(tiers[2].iter().map(|a| a.channel_id).collect::<Vec<_>>(), vec![5]);
}

/// TC-UNI-SEL-002-BND:retry 超出档数时返回 None,不得 panic。
///
/// 重试次数配置得比档数多是常见情况,越界必须优雅退出。
#[test]
fn tc_uni_sel_002_bnd_retry_beyond_tier_count() {
    let c = candidates();
    let tiers = priority_tiers(&c);
    assert!(tiers.get(3).is_none(), "第 4 档不存在,应返回 None 而非越界 panic");
}

/// TC-UNI-SEL-003-POS:权重平滑 —— weight=0 的渠道必须有机会被选中。★
///
/// 平滑公式 `weight + 10` 的意义就在于此。若实现直接用 weight 做权重,
/// weight=0 的渠道永远选不中,等于静默下线。
#[test]
fn tc_uni_sel_003_pos_zero_weight_still_selectable() {
    let tier = vec![ability(1, 100, 5), ability(2, 100, 0)];
    let refs: Vec<&Ability> = tier.iter().collect();

    let mut picked_zero_weight = 0;
    for _ in 0..1000 {
        if let Some(a) = weighted_pick(&refs) {
            if a.channel_id == 2 {
                picked_zero_weight += 1;
            }
        }
    }
    assert!(
        picked_zero_weight > 0,
        "weight=0 的渠道一次都没被选中,说明权重平滑(weight+10)没生效"
    );
}

/// TC-UNI-SEL-004-POS:加权随机的分布符合 (weight+10) 占比。
///
/// A(w=5)→15,B(w=0)→10,期望比例 3:2,即 A 占 60%。
/// 统计断言给 ±3% 容差。
#[test]
fn tc_uni_sel_004_pos_weighted_distribution() {
    let tier = vec![ability(1, 100, 5), ability(2, 100, 0)];
    let refs: Vec<&Ability> = tier.iter().collect();

    const N: usize = 10_000;
    let mut a_count = 0usize;
    for _ in 0..N {
        if let Some(a) = weighted_pick(&refs) {
            if a.channel_id == 1 {
                a_count += 1;
            }
        }
    }
    let ratio = a_count as f64 / N as f64;
    assert!(
        (0.57..=0.63).contains(&ratio),
        "A 的选中占比应约 15/(15+10)=0.6,实际 {ratio:.3}"
    );
}

/// TC-UNI-SEL-005-BND:空候选集返回 None。
#[test]
fn tc_uni_sel_005_bnd_empty_tier() {
    let empty: Vec<&Ability> = vec![];
    assert!(weighted_pick(&empty).is_none());
}

/// TC-UNI-SEL-006-BND:单候选恒返回该候选。
#[test]
fn tc_uni_sel_006_bnd_single_candidate() {
    let tier = vec![ability(7, 100, 0)];
    let refs: Vec<&Ability> = tier.iter().collect();
    for _ in 0..100 {
        assert_eq!(weighted_pick(&refs).map(|a| a.channel_id), Some(7));
    }
}

/// TC-UNI-SEL-007-POS:同档内所有渠道最终都会被选到(无饥饿)。
#[test]
fn tc_uni_sel_007_pos_no_starvation_within_tier() {
    let tier: Vec<Ability> = (1..=5).map(|i| ability(i, 100, 0)).collect();
    let refs: Vec<&Ability> = tier.iter().collect();

    let mut seen = std::collections::HashSet::new();
    for _ in 0..5000 {
        if let Some(a) = weighted_pick(&refs) {
            seen.insert(a.channel_id);
        }
    }
    assert_eq!(seen.len(), 5, "等权重下 5 个渠道都应被选到,实际只有 {:?}", seen);
}
