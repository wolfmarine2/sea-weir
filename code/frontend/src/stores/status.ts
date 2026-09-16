/**
 * 全站配置。数据源**唯一**:服务端 `GET /api/status`。
 *
 * 内容:系统名、logo、OAuth 开关、支付开关、导航模块可见性等。
 * **不持久化** —— 每次启动重新拉取,避免配置变更后前端拿旧值
 * (这正是 new-api 把配置塞 localStorage 带来的问题)。
 */
import { create } from 'zustand';
import { request } from '@/api/client';

export interface StatusState {
  loaded: boolean;
  systemName: string;
  logo: string;
  /** OAuth / 支付 / 各功能模块的开关集合 */
  features: Record<string, boolean>;
  /** 数据库不可用原因(可用时为空串);用于首页直接展示,免去翻日志 */
  dbError: string;

  fetch: () => Promise<void>;
}

/** 后端 `/api/status` 的 data 载荷(字段 snake_case)。 */
interface StatusPayload {
  system_name?: string;
  logo?: string;
  db_error?: string | null;
  [key: string]: unknown;
}

/** 从 status 载荷中提取布尔开关集合。 */
function extractFeatures(payload: StatusPayload): Record<string, boolean> {
  const features: Record<string, boolean> = {};
  for (const [key, value] of Object.entries(payload)) {
    if (typeof value === 'boolean') {
      features[key] = value;
    }
  }
  return features;
}

export const useStatusStore = create<StatusState>()((set) => ({
  loaded: false,
  systemName: '',
  logo: '',
  features: {},
  dbError: '',
  fetch: async () => {
    try {
      const payload = await request<StatusPayload>({ url: '/api/status', method: 'get' });
      set({
        loaded: true,
        systemName: payload.system_name ?? '',
        logo: payload.logo ?? '',
        features: extractFeatures(payload),
        dbError: payload.db_error ?? '',
      });
    } catch {
      // 失败不阻塞首屏:以默认值降级,标记已加载。
      set({ loaded: true });
    }
  },
}));

// 注意:**不得**为该 store 添加 persist(TC-UNI-FE-022 有断言防回归)。
// - [x] fetch 失败时不阻塞首屏渲染,用默认值降级
// - [x] 该 store 不写 localStorage
