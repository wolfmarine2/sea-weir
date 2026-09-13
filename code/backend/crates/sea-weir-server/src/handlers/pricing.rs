//! `pricing` 域 handler:倍率配置快照(CONTRACTS §6)。
//!
//! 已落地:GET /api/ratio_config(当前倍率快照)。
//! 待补:GET /api/pricing(abilities × models × vendors × 倍率合成的模型广场视图)、
//! 阶梯计费、上游倍率同步(/api/ratio_sync)。

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;

use crate::app_state::ServerState;
use crate::response;

/// `GET /api/ratio_config`:当前全部倍率配置快照(模型/分组/缓存/图片/音频)。
pub async fn ratio_config(State(state): State<Arc<ServerState>>) -> Response {
    let view = state
        .pricing
        .get(state.options.as_ref().map(|o| o.as_ref()))
        .await;
    response::ok(view.snapshot())
}
