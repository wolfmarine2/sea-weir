//! 定价视图:从 `options` 表合成各模型/分组的倍率,进程内 1 分钟缓存。
//!
//! 依据:doc/architecture/CONTRACTS.md §6、ADR-007(定价视图进程内缓存,60s 兜底)。
//!
//! options 键(与 new-api 命名一致,值均为 JSON):
//!   `ModelRatio` / `GroupRatio` / `CompletionRatio` / `CacheRatio` /
//!   `CacheCreationRatio` / `CacheCreation5mRatio` / `CacheCreation1hRatio` /
//!   `ImageRatio` / `AudioRatio` / `ModelPrice` / `AudioInputPrice`
//!
//! 值可以是 `{"gpt-4o": 2.5}` 形式的对象,也可以是单个数字(对全部生效,存为 `*`)。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rust_decimal::Decimal;
use sea_weir_core::relay::billing::PriceData;
use sea_weir_repository::OptionRepository;
use tokio::sync::RwLock;

/// 缓存 TTL(ADR-007:60s 兜底)。
const TTL: Duration = Duration::from_secs(60);

/// 合成后的倍率视图。
#[derive(Debug, Default)]
pub struct PricingView {
    model_ratio: HashMap<String, Decimal>,
    group_ratio: HashMap<String, Decimal>,
    completion_ratio: HashMap<String, Decimal>,
    cache_ratio: HashMap<String, Decimal>,
    create_cache_ratio: HashMap<String, Decimal>,
    creation_5m_ratio: HashMap<String, Decimal>,
    creation_1h_ratio: HashMap<String, Decimal>,
    image_ratio: HashMap<String, Decimal>,
    audio_ratio: HashMap<String, Decimal>,
    audio_input_price: HashMap<String, Decimal>,
    model_price: HashMap<String, Decimal>,
}

fn parse_decimal(value: &serde_json::Value) -> Option<Decimal> {
    match value {
        serde_json::Value::Number(n) => n.to_string().parse().ok(),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn parse_map(value: &serde_json::Value) -> HashMap<String, Decimal> {
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .filter_map(|(k, v)| parse_decimal(v).map(|d| (k.clone(), d)))
            .collect(),
        // 标量:对全部键生效,存为 `*`。
        other => parse_decimal(other)
            .map(|d| HashMap::from([("*".to_string(), d)]))
            .unwrap_or_default(),
    }
}

impl PricingView {
    /// 从 options 的全量 KV 合成(TODO: 后续可并入 models/vendors 维度)。
    pub fn from_options(pairs: &[(String, String)]) -> Self {
        let mut view = PricingView::default();
        for (key, raw) in pairs {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
                continue;
            };
            match key.as_str() {
                "ModelRatio" => view.model_ratio = parse_map(&value),
                "GroupRatio" => view.group_ratio = parse_map(&value),
                "CompletionRatio" => view.completion_ratio = parse_map(&value),
                "CacheRatio" => view.cache_ratio = parse_map(&value),
                "CacheCreationRatio" => view.create_cache_ratio = parse_map(&value),
                "CacheCreation5mRatio" => view.creation_5m_ratio = parse_map(&value),
                "CacheCreation1hRatio" => view.creation_1h_ratio = parse_map(&value),
                "ImageRatio" => view.image_ratio = parse_map(&value),
                "AudioRatio" => view.audio_ratio = parse_map(&value),
                "AudioInputPrice" => view.audio_input_price = parse_map(&value),
                "ModelPrice" => view.model_price = parse_map(&value),
                _ => {}
            }
        }
        view
    }

    fn lookup(map: &HashMap<String, Decimal>, key: &str, default: Decimal) -> Decimal {
        map.get(key)
            .or_else(|| map.get("*"))
            .copied()
            .unwrap_or(default)
    }

    /// 合成某模型在某分组下的计价输入。
    ///
    /// 缺省:模型/分组/补全/缓存倍率均为 1,音频单价为 0(与 new-api 默认一致)。
    pub fn price_for(&self, model: &str, group: &str) -> PriceData {
        PriceData {
            model_ratio: Self::lookup(&self.model_ratio, model, Decimal::ONE),
            group_ratio: Self::lookup(&self.group_ratio, group, Decimal::ONE),
            completion_ratio: Self::lookup(&self.completion_ratio, model, Decimal::ONE),
            cache_ratio: Self::lookup(&self.cache_ratio, model, Decimal::ONE),
            create_cache_ratio: Self::lookup(&self.create_cache_ratio, model, Decimal::ONE),
            cache_creation_5m_ratio: Self::lookup(&self.creation_5m_ratio, model, Decimal::ONE),
            cache_creation_1h_ratio: Self::lookup(&self.creation_1h_ratio, model, Decimal::ONE),
            image_ratio: Self::lookup(&self.image_ratio, model, Decimal::ONE),
            audio_ratio: Self::lookup(&self.audio_ratio, model, Decimal::ONE),
            audio_input_price: Self::lookup(&self.audio_input_price, model, Decimal::ZERO),
            model_price: self.model_price.get(model).copied(),
            tiered_expr: None,
            other_ratios: Vec::new(),
        }
    }

    /// 快照(供 `/api/ratio_config` 输出)。
    pub fn snapshot(&self) -> serde_json::Value {
        let dump = |m: &HashMap<String, Decimal>| {
            serde_json::to_value(m.iter().map(|(k, v)| (k.clone(), v.to_string())).collect::<HashMap<_, _>>())
                .unwrap_or(serde_json::Value::Null)
        };
        serde_json::json!({
            "model_ratio": dump(&self.model_ratio),
            "group_ratio": dump(&self.group_ratio),
            "completion_ratio": dump(&self.completion_ratio),
            "cache_ratio": dump(&self.cache_ratio),
            "cache_creation_ratio": dump(&self.create_cache_ratio),
            "cache_creation_5m_ratio": dump(&self.creation_5m_ratio),
            "cache_creation_1h_ratio": dump(&self.creation_1h_ratio),
            "image_ratio": dump(&self.image_ratio),
            "audio_ratio": dump(&self.audio_ratio),
            "audio_input_price": dump(&self.audio_input_price),
            "model_price": dump(&self.model_price),
        })
    }
}

/// 定价视图缓存(读多写少,60s TTL;取不到 options 时退化为全 1)。
#[derive(Default)]
pub struct PricingCache {
    inner: RwLock<Option<(Instant, Arc<PricingView>)>>,
}

impl PricingCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取当前视图(命中缓存则直接返回;过期或未初始化则重新合成)。
    pub async fn get(&self, options: Option<&dyn OptionRepository>) -> Arc<PricingView> {
        {
            let guard = self.inner.read().await;
            if let Some((at, view)) = guard.as_ref() {
                if at.elapsed() < TTL {
                    return view.clone();
                }
            }
        }

        let pairs = match options {
            Some(repo) => repo.load_all().await.unwrap_or_default(),
            None => Vec::new(),
        };
        let view = Arc::new(PricingView::from_options(&pairs));
        let mut guard = self.inner.write().await;
        *guard = Some((Instant::now(), view.clone()));
        view
    }

    /// 强制失效(倍率变更后调用)。
    pub async fn invalidate(&self) {
        let mut guard = self.inner.write().await;
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Vec<(String, String)> {
        vec![
            ("ModelRatio".into(), r#"{"gpt-4o": 1}"#.into()),
            ("GroupRatio".into(), r#"{"default": 1}"#.into()),
            ("CompletionRatio".into(), r#"{"gpt-4o": 1}"#.into()),
            ("CacheRatio".into(), r#"{"gpt-4o": 0.1}"#.into()),
        ]
    }

    /// 对齐 TEST-VECTORS TV-BILL-001:quota == 798。
    #[test]
    fn tv_bill_001_pricing_view() {
        use sea_weir_core::relay::billing::calculate_quota;
        use sea_weir_types::dto::Usage;

        let view = PricingView::from_options(&options());
        let price = view.price_for("gpt-4o", "default");
        assert_eq!(price.model_ratio, Decimal::ONE);
        assert_eq!(price.cache_ratio, Decimal::from_str_exact("0.1").unwrap());

        let usage = Usage {
            prompt_tokens: 2604,
            completion_tokens: 383,
            total_tokens: 2987,
            cached_tokens: 2432,
            ..Usage::default()
        };
        assert_eq!(calculate_quota(&usage, &price), 798);
    }

    /// 未知模型/分组回落默认倍率 1,不 panic。
    #[test]
    fn unknown_model_falls_back_to_one() {
        let view = PricingView::from_options(&options());
        let price = view.price_for("unknown", "unknown");
        assert_eq!(price.model_ratio, Decimal::ONE);
        assert_eq!(price.group_ratio, Decimal::ONE);
    }

    /// 标量形式(如 CacheRatio 写成单个数字)对全部模型生效。
    #[test]
    fn scalar_ratio_applies_to_all() {
        let view = PricingView::from_options(&[("CacheRatio".into(), "5".into())]);
        assert_eq!(view.price_for("any", "default").cache_ratio, Decimal::from(5));
    }
}
