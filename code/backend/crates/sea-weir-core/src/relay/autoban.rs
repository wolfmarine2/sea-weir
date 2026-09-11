//! 渠道自动禁用与重试判定。对应 C4 组件 `autoban`。见 SEQ-005。
//!
//! ## 重试判定
//! - 渠道类错误(`channel:*`)→ 重试
//! - `skip_retry` 标记 / 亲和性失败跳过 / `specific_channel_id` → 不重试
//! - 2xx → 不重试;状态码超出 100..=599 → 重试
//! - 其余按可配置区间;默认排除 **400 / 408 / 504 / 524**
//! - 最大尝试次数 = `retry_times + 1`
//!
//! ## 自动禁用判定
//! - 命中禁用状态码(**默认仅 401**)或关键词
//! - 单 key 渠道 → 整渠道置自动禁用并从索引摘除
//! - 多 key 渠道 → 按 key 粒度禁用
//! - 禁用后通知 root
//! - 渠道测试通过(手动/定时)且开启自动恢复 → 重新启用

use sea_weir_types::NewApiError;

/// 重试判定上下文。
pub struct RetryContext {
    pub retry_times_left: u32,
    pub has_specific_channel: bool,
    pub affinity_skip_retry: bool,
}

/// 是否应重试。
///
/// 判定顺序(严格短路,顺序即语义,见 TEST-VECTORS §4.1):
/// 1. `local_error` → 否(我方解析问题,不是渠道的锅)
/// 2. 亲和性失败跳过 → 否(优先级最高,粘性路由语义)
/// 3. 渠道类错误(`channel:*`)→ **是**(优先于次数判定)
/// 4. `skip_retry` 标记 → 否
/// 5. 指定渠道(`sk-xxx-{id}`)→ 否
/// 6. 次数耗尽 → 否
/// 7. 状态码区间判定
pub fn should_retry(err: &NewApiError, ctx: &RetryContext) -> bool {
    use sea_weir_types::constants::retry_policy::{ALWAYS_SKIP_RETRY, DEFAULT_RETRY_RANGES};

    if err.local_error {
        return false;
    }
    if ctx.affinity_skip_retry {
        return false;
    }
    if err.is_channel_error() {
        return true;
    }
    if err.skip_retry {
        return false;
    }
    if ctx.has_specific_channel {
        return false;
    }
    if ctx.retry_times_left == 0 {
        return false;
    }
    let code = err.status_code;
    // 超出 100..=599 视为网络层异常,重试。
    if !(100..=599).contains(&code) {
        return true;
    }
    // 2xx 一律不重试。
    if (200..=299).contains(&code) {
        return false;
    }
    // 永不重试清单优先级高于区间配置。
    if ALWAYS_SKIP_RETRY.contains(&code) {
        return false;
    }
    DEFAULT_RETRY_RANGES
        .iter()
        .any(|(start, end)| code >= *start && code <= *end)
}

/// 是否应禁用渠道。
///
/// - `auto_ban=false` 时恒为 false(渠道级保护开关)
/// - `local_error=true` 不禁用(避免误禁好渠道)
/// - 命中默认禁用状态码区间(**仅 401**)时禁用
pub fn should_disable(err: &NewApiError, auto_ban: bool) -> bool {
    use sea_weir_types::constants::retry_policy::DEFAULT_DISABLE_RANGES;

    if !auto_ban || err.local_error {
        return false;
    }
    DEFAULT_DISABLE_RANGES
        .iter()
        .any(|(start, end)| err.status_code >= *start && err.status_code <= *end)
}

/// 禁用粒度。
pub enum DisableScope {
    /// 单 key 渠道:整渠道禁用。
    Channel,
    /// 多 key 渠道:仅禁用出错的 key。
    Key(usize),
}

#[cfg(test)]
mod tests {
    // TDD 入口(表驱动,逐个状态码断言 —— 这是审查中核实过的契约):
    // - [ ] should_retry:400 → false、408 → false、504 → false、524 → false
    // - [ ] should_retry:429 → true、500 → true、502 → true
    // - [ ] should_retry:2xx → false
    // - [ ] should_retry:状态码 < 100 或 > 599 → true
    // - [ ] should_retry:retry_times_left == 0 时,非渠道错误一律 false
    // - [ ] should_retry:渠道错误优先级高于次数判定
    // - [ ] should_retry:has_specific_channel → false
    // - [ ] should_disable:401 → true(默认区间)
    // - [ ] should_disable:auto_ban=false 时恒 false
    // - [ ] local_error=true 的错误既不重试也不禁用
    // - [ ] 多 key 渠道禁用后其余 key 仍可被选中
}
