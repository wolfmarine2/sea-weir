/**
 * L1 — 状态层与 localStorage 收敛。用例:`test/cases/14-frontend.md`(TC-UNI-FE-020~022)。
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { USER_PERSIST_KEY, useUserStore } from './user';
import { useStatusStore } from './status';
import { useThemeStore } from './theme';

beforeEach(() => {
  localStorage.clear();
  useUserStore.getState().logout();
  localStorage.clear();
  useThemeStore.setState({ mode: 'light' });
  useStatusStore.setState({ loaded: false, systemName: '', logo: '', features: {} });
});

describe('stores', () => {
  it('TC-UNI-FE-020-POS:logout 清空字段并移除 persist 项', () => {
    useUserStore.getState().login({ userId: 9, username: 'alice', role: 10, group: 'vip' });
    expect(useUserStore.getState().isLoggedIn).toBe(true);
    expect(localStorage.getItem(USER_PERSIST_KEY)).not.toBeNull();

    useUserStore.getState().logout();

    const state = useUserStore.getState();
    expect(state.isLoggedIn).toBe(false);
    expect(state.userId).toBeNull();
    expect(state.username).toBe('');
    expect(state.role).toBe(0);
    expect(state.group).toBe('');
    expect(localStorage.getItem(USER_PERSIST_KEY)).toBeNull();
  });

  it('TC-UNI-FE-021-POS:persist 键数 ≤ 5 ★', () => {
    useUserStore.getState().login({ userId: 1, username: 'u', role: 1, group: 'default' });
    useThemeStore.getState().toggle();

    // 持久化只允许用户态 + 主题(ADR-009 的收敛目标)。
    expect(localStorage.length).toBeLessThanOrEqual(5);

    // 用户态持久化字段也不超过 5 个。
    const raw = localStorage.getItem(USER_PERSIST_KEY);
    expect(raw).not.toBeNull();
    const parsed = JSON.parse(raw as string) as { state: Record<string, unknown> };
    expect(Object.keys(parsed.state).length).toBeLessThanOrEqual(5);
  });

  it('TC-UNI-FE-022-NEG:statusStore 不写 localStorage ★', () => {
    useStatusStore.getState().fetch();
    // status 数据源唯一是服务端,任何形式的本地缓存都会导致配置陈旧。
    for (let i = 0; i < localStorage.length; i += 1) {
      const key = localStorage.key(i) as string;
      expect(key.toLowerCase()).not.toContain('status');
    }
  });

  it('theme 为亮暗切换并持久化', () => {
    expect(useThemeStore.getState().mode).toBe('light');
    useThemeStore.getState().toggle();
    expect(useThemeStore.getState().mode).toBe('dark');
    useThemeStore.getState().toggle();
    expect(useThemeStore.getState().mode).toBe('light');
  });
});
