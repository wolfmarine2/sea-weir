/**
 * L1 — 额度换算与展示。用例:`test/cases/14-frontend.md`(TC-UNI-FE-030~032)。
 */
import { describe, expect, it } from 'vitest';
import { QUOTA_PER_UNIT, formatQuota, quotaToUsd } from './quota';

describe('quota 工具', () => {
  it('TC-UNI-FE-030-POS:500000 quota === $1.00', () => {
    expect(QUOTA_PER_UNIT).toBe(500_000);
    expect(formatQuota(500_000)).toBe('$1.00');
    expect(quotaToUsd(500_000)).toBe(1);
  });

  it('TC-UNI-FE-031-BND:0 与负数', () => {
    expect(formatQuota(0)).toBe('$0.00');
    expect(formatQuota(250_000)).toBe('$0.50');
    // 退款场景:负数必须正确显示。
    expect(formatQuota(-250_000)).toBe('-$0.50');
  });

  it('TC-UNI-FE-031-BND:大额不出现科学计数法', () => {
    const out = formatQuota(500_000 * 1_000_000);
    expect(out).not.toMatch(/e\+/i);
    expect(out).toBe('$1,000,000.00');
  });

  it('TC-UNI-FE-032-POS:取整方向与后端一致(half away from zero)', () => {
    // 0.005 美元 = 2500 quota → $0.01(halfExpand),不是 $0.00(banker's)
    expect(formatQuota(2_500)).toBe('$0.01');
    // 0.015 美元 = 7500 quota → $0.02
    expect(formatQuota(7_500)).toBe('$0.02');
  });
});
