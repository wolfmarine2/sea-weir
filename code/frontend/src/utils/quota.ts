/** 额度换算。QUOTA_PER_UNIT = 500000,即 500000 quota = $1。 */

/** 与后端 `constants::QUOTA_PER_UNIT` 保持一致,改动即破坏前后端一致性。 */
export const QUOTA_PER_UNIT = 500_000;

/** quota → 美元(不做格式化)。 */
export function quotaToUsd(quota: number): number {
  return quota / QUOTA_PER_UNIT;
}

/**
 * 金额展示。
 *
 * 使用 `Intl.NumberFormat` 而非 `toFixed`:
 * - 大额不会退化为科学计数法(1e21.toFixed(2) === "1e+21")
 * - 默认 roundingMode 为 halfExpand(half away from zero),与后端
 *   `rust_decimal` 的 `MidpointAwayFromZero` 一致,避免显示差 1。
 */
const USD_FORMATTER = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

/** quota → 展示字符串,如 500000 → "$1.00"。负数正确显示为 "-$0.50"。 */
export function formatQuota(quota: number): string {
  return USD_FORMATTER.format(quotaToUsd(quota));
}

// TDD 要点:
// - [x] 500000 → "$1.00"
// - [x] 0 → "$0.00";负数(退款场景)正确显示
// - [x] 大额不出现科学计数法
// - [x] 与后端 rust_decimal 的取整方向一致
