/**
 * 主题(亮/暗)。zustand + persist(属于用户偏好,允许持久化)。
 * 驱动 antd 的 theme.algorithm 与图表配色。
 */
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type ThemeMode = 'light' | 'dark';

export interface ThemeState {
  mode: ThemeMode;
  toggle: () => void;
}

export const THEME_PERSIST_KEY = '__SEAWEIR__theme';

export const useThemeStore = create<ThemeState>()(
  persist(
    (set) => ({
      mode: 'light',
      toggle: () => set((state) => ({ mode: state.mode === 'light' ? 'dark' : 'light' })),
    }),
    { name: THEME_PERSIST_KEY },
  ),
);
