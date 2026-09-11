/**
 * 登录态。zustand + persist。
 *
 * 持久化范围(ADR-009 决策):**仅登录态与用户偏好**。
 * 服务端配置一律走 statusStore,不再像 new-api 那样平铺 20+ 键进 localStorage。
 */
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

/** localStorage 键。ADR-009:全站持久化键总数 ≤ 5。 */
export const USER_PERSIST_KEY = '__SEAWEIR__user';

export interface UserState {
  userId: number | null;
  username: string;
  /** 0 guest / 1 common / 10 admin / 100 root */
  role: number;
  group: string;
  isLoggedIn: boolean;

  login: (user: { userId: number; username: string; role: number; group: string }) => void;
  logout: () => void;
}

const EMPTY = {
  userId: null,
  username: '',
  role: 0,
  group: '',
  isLoggedIn: false,
} as const;

export const useUserStore = create<UserState>()(
  persist(
    (set) => ({
      ...EMPTY,
      login: (user) =>
        set({
          userId: user.userId,
          username: user.username,
          role: user.role,
          group: user.group,
          isLoggedIn: true,
        }),
      logout: () => {
        set({ ...EMPTY });
        // persist 中间件会在 set 时重写键,这里显式移除,确保登出后无残留。
        if (typeof localStorage !== 'undefined') {
          localStorage.removeItem(USER_PERSIST_KEY);
        }
      },
    }),
    {
      name: USER_PERSIST_KEY,
      // 仅持久化这 5 个字段(TC-UNI-FE-021 的 ≤5 约束)。
      partialize: (state) => ({
        userId: state.userId,
        username: state.username,
        role: state.role,
        group: state.group,
        isLoggedIn: state.isLoggedIn,
      }),
    },
  ),
);

// TDD 要点:
// - [x] logout() 清空全部字段并移除 persist 项
// - [x] persist 的键集合 ≤ 5(ADR-009 的收敛目标,写成断言防回归)
// - [ ] role 变更后 RequireRole 守卫立即生效(路由守卫落地时补)
