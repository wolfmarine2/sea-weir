/**
 * axios 封装。对应 C4 组件 `api_layer`。
 *
 * 契约要点(doc/architecture/CONTRACTS.md):
 * 1. 受保护端点必须携带 `New-Api-User: <user_id>` 头(防串号),值取自 userStore;
 * 2. 管理面业务错误是 **HTTP 200 + `success:false`** —— 必须在响应拦截器里
 *    把 `success === false` 转成 reject,否则调用方会把失败当成功;
 * 3. HTTP 401 → 清理 userStore → 跳转 /login;
 * 4. GET 请求去重(相同 url + params 的并发请求复用同一个 promise)。
 */
import axios, { type AxiosRequestConfig } from 'axios';
import { useUserStore } from '@/stores/user';

export const client = axios.create({
  baseURL: import.meta.env.VITE_API_BASE ?? '/',
  timeout: 30_000,
  // 会话走 Set-Cookie;同源反代下本已携带,显式开启以兼容独立域部署。
  withCredentials: true,
});

/**
 * 统一响应包裹。与后端 `ApiResponse<T>` 对应。
 * 注意字段为 snake_case —— 全站 DTO 均遵循此约定。
 */
export interface ApiResponse<T> {
  success: boolean;
  message: string;
  data?: T;
}

/** 分页响应。页码 **1 起**(与后端 `PageQuery` 一致)。 */
export interface PageInfo<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
}

/** 业务错误(HTTP 200 + success:false)。与网络/HTTP 错误区分开,见 TC-UNI-FE-005。 */
export class ApiBusinessError extends Error {
  readonly body: unknown;
  constructor(message: string, body: unknown) {
    super(message);
    this.name = 'ApiBusinessError';
    this.body = body;
  }
}

/** HTTP 层错误(非 2xx,且非业务错误包裹)。 */
export class ApiHttpError extends Error {
  readonly status: number;
  constructor(message: string, status: number) {
    super(message);
    this.name = 'ApiHttpError';
    this.status = status;
  }
}

/** 网络层错误(无响应:超时、断网、DNS 等)。 */
export class ApiNetworkError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ApiNetworkError';
  }
}

let unauthorizedHandler: () => void = () => {
  if (typeof window !== 'undefined') {
    window.location.href = '/login';
  }
};

/** 允许测试替换 401 的跳转行为(jsdom 不支持真实导航)。 */
export function setUnauthorizedHandler(handler: () => void): void {
  unauthorizedHandler = handler;
}

// 请求拦截器:注入 New-Api-User 防串号头。
client.interceptors.request.use((config) => {
  const { userId } = useUserStore.getState();
  if (userId !== null) {
    config.headers.set('New-Api-User', String(userId));
  }
  return config;
});

// 响应拦截器:success:false 转 reject;401 清态跳登录。
client.interceptors.response.use(
  (response) => {
    const body: unknown = response.data;
    if (isApiEnvelope(body) && body.success === false) {
      return Promise.reject(new ApiBusinessError(body.message || '请求失败', body));
    }
    return response;
  },
  (error: unknown) => {
    if (error instanceof Error && 'name' in error) {
      const name = error.name;
      if (name === 'ApiBusinessError' || name === 'ApiHttpError' || name === 'ApiNetworkError') {
        return Promise.reject(error);
      }
    }
    const axiosError = error as { response?: { status: number; data?: unknown }; message?: string };
    if (axiosError.response) {
      const status = axiosError.response.status;
      if (status === 401) {
        useUserStore.getState().logout();
        unauthorizedHandler();
      }
      const data = axiosError.response.data;
      const message =
        isApiEnvelope(data) && data.message ? data.message : `请求失败(HTTP ${status})`;
      return Promise.reject(new ApiHttpError(message, status));
    }
    return Promise.reject(new ApiNetworkError(axiosError.message ?? '网络错误'));
  },
);

function isApiEnvelope(value: unknown): value is ApiResponse<unknown> {
  return typeof value === 'object' && value !== null && 'success' in value;
}

// ── GET 去重 ──
const inFlight = new Map<string, Promise<unknown>>();

function dedupKey(url: string, config?: AxiosRequestConfig): string {
  const params = config?.params ? JSON.stringify(config.params) : '';
  return `${url}?${params}`;
}

/**
 * GET 请求去重:相同 url + params 的并发请求复用同一个 promise。
 *
 * 返回响应体 `data` 字段(已剥离统一包裹)。
 */
export function dedupeGet<T>(url: string, config?: AxiosRequestConfig): Promise<T> {
  const key = dedupKey(url, config);
  const pending = inFlight.get(key);
  if (pending) {
    return pending as Promise<T>;
  }
  const promise = client
    .get<ApiResponse<T>>(url, config)
    .then((response) => response.data.data as T)
    .finally(() => {
      inFlight.delete(key);
    });
  inFlight.set(key, promise);
  return promise;
}

/** POST/PUT/DELETE 的统一调用,返回响应体 `data`。 */
export async function request<T>(config: AxiosRequestConfig): Promise<T> {
  const response = await client.request<ApiResponse<T>>(config);
  return response.data.data as T;
}
