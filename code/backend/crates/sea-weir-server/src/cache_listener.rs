//! 缓存失效广播订阅(ADR-007):多节点经 Valkey pub/sub 即时失效进程内缓存。
//!
//! 现状:订阅 `cache:invalidate:{option,channel,token,user}` 四个频道,
//! 收到任意消息即失效本进程的定价视图缓存(其余缓存在落地时接入同一入口)。
//! 60s TTL 兜底仍保留(见 pricing::PricingCache)。

use std::sync::Arc;

use fred::prelude::*;

use crate::app_state::ServerState;

/// 失效广播频道(与 repository::cache::channels 保持一致)。
const CHANNELS: &[&str] = &[
    "cache:invalidate:option",
    "cache:invalidate:channel",
    "cache:invalidate:token",
    "cache:invalidate:user",
];

/// 启动订阅任务(缓存未配置时为空操作)。
pub fn spawn(state: Arc<ServerState>) {
    let Some(cache) = state.cache.clone() else {
        return;
    };

    tokio::spawn(async move {
        let subscriber = match cache.subscriber().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "订阅缓存失效广播失败(降级:仅靠 TTL 兜底)");
                return;
            }
        };
        // 断线重连后自动重新订阅。
        let _ = subscriber.manage_subscriptions();
        for channel in CHANNELS {
            if let Err(e) = subscriber.subscribe((*channel).to_string()).await {
                tracing::warn!(error = %e, channel, "订阅失效频道失败");
            }
        }
        tracing::info!("缓存失效广播订阅就绪");

        let mut rx = subscriber.message_rx();
        while let Ok(message) = rx.recv().await {
            tracing::info!(channel = ?message.channel, "收到缓存失效广播,失效定价缓存");
            state.pricing.invalidate().await;
        }
        tracing::warn!("缓存失效广播订阅结束");
    });
}
