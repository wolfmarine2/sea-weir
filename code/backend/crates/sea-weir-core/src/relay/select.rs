//! 渠道选择。对应 C4 组件 `channel_selector`。
//!
//! 规则(doc/system-design.md §6.6、ADR-006):
//! - `(group, model)` → abilities/渠道索引 → **第 `retry` 高优先级档** → 档内**加权随机**
//! - 权重平滑:`weight + 10`
//! - `auto` 分组:按配置的分组列表逐组降级(分组列表 × 优先级 的二维降级)
//! - 令牌 `cross_group_retry` 控制是否允许跨组重试
//! - 亲和性命中优先(见 `affinity`)
//! - `sk-xxx-{channelId}` 指定渠道时跳过选路(仅 admin/root 可用)

use sea_weir_types::{domain::Ability, AppResult};

pub struct SelectRequest<'a> {
    pub group: &'a str,
    pub model: &'a str,
    /// 第几次尝试(0 起)。决定取第几优先级档。
    pub retry: u32,
    pub cross_group_retry: bool,
    /// 用户指定的渠道(admin/root)。存在时直接返回。
    pub specific_channel_id: Option<i64>,
}

/// 选路结果。
pub struct Selection {
    pub channel_id: i64,
    /// multi-key 渠道选中的 key 下标。
    pub key_index: Option<usize>,
    /// 命中亲和性缓存(影响失败后是否跳过重试)。
    pub from_affinity: bool,
}

pub trait ChannelSelector: Send + Sync {
    fn select(&self, req: SelectRequest<'_>) -> AppResult<Option<Selection>>;
}

/// 优先级分档:按 priority 降序分组,取第 `retry` 档。
///
/// 档内保持入参顺序(稳定排序),便于测试与复现。
pub fn priority_tiers(candidates: &[Ability]) -> Vec<Vec<&Ability>> {
    let mut sorted: Vec<&Ability> = candidates.iter().collect();
    // 按 priority 降序;sort_by_key 稳定,同 priority 保持原始相对顺序。
    sorted.sort_by_key(|a| std::cmp::Reverse(a.priority));

    let mut tiers: Vec<Vec<&Ability>> = Vec::new();
    for ability in sorted {
        match tiers.last_mut() {
            Some(tier) if tier.last().map(|a| a.priority) == Some(ability.priority) => {
                tier.push(ability)
            }
            _ => tiers.push(vec![ability]),
        }
    }
    tiers
}

/// 档内加权随机。权重使用 `weight + 10` 平滑,避免 weight=0 的渠道永不被选中。
///
/// 平滑值与 new-api 一致(`weight + 10`);权重下限为 0,总和为 0 时退化为均匀随机。
pub fn weighted_pick<'a>(tier: &[&'a Ability]) -> Option<&'a Ability> {
    if tier.is_empty() {
        return None;
    }
    let weights: Vec<u64> = tier.iter().map(|a| (a.weight + 10).max(0) as u64).collect();
    let total: u64 = weights.iter().sum();
    if total == 0 {
        let idx = rand::Rng::gen_range(&mut rand::thread_rng(), 0..tier.len());
        return Some(tier[idx]);
    }
    let mut roll = rand::Rng::gen_range(&mut rand::thread_rng(), 0..total);
    for (i, w) in weights.iter().enumerate() {
        if roll < *w {
            return Some(tier[i]);
        }
        roll -= *w;
    }
    // 数值上不可达,兜底返回最后一个。
    tier.last().copied()
}

#[cfg(test)]
mod tests {
    // TDD 入口(选路是中继正确性的地基,建议最先覆盖):
    // - [ ] priority_tiers:priority 相同的归为同档,档间按 priority 降序
    // - [ ] retry=0 取最高档;retry 超过档数时返回 None(而非 panic)
    // - [ ] weighted_pick:weight 全为 0 时仍能选中(平滑生效)
    // - [ ] weighted_pick:大样本下选中频率≈(weight+10)占比(统计断言,固定随机种子)
    // - [ ] specific_channel_id 存在时直接返回,不走选路
    // - [ ] auto 分组:第一组无可用渠道时降级到第二组
    // - [ ] cross_group_retry=false 时不跨组
}
