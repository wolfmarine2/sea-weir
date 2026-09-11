/**
 * L1 — API 层三条契约。用例:`test/cases/14-frontend.md`(TC-UNI-FE-001~005)。
 *
 * 用注入 axios adapter 的方式打桩,不需要起服务;响应体取自
 * `test/cases/fixtures/baseline/` 的真实录制形状。
 */
import type { AxiosResponse, InternalAxiosRequestConfig } from 'axios';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ApiBusinessError,
  ApiHttpError,
  ApiNetworkError,
  client,
  dedupeGet,
  setUnauthorizedHandler,
} from './client';
import { useUserStore } from '@/stores/user';

type MockReply = { status?: number; data?: unknown; delayMs?: number };

let reply: (config: InternalAxiosRequestConfig) => MockReply | Promise<MockReply>;
let calls: InternalAxiosRequestConfig[];

function installAdapter() {
  client.defaults.adapter = (async (config: InternalAxiosRequestConfig) => {
    calls.push(config);
    const result = await reply(config);
    const status = result.status ?? 200;
    if (result.delayMs) {
      await new Promise((resolve) => setTimeout(resolve, result.delayMs));
    }
    const response = {
      data: result.data,
      status,
      statusText: String(status),
      headers: {},
      config,
    } as AxiosResponse;
    if (status < 200 || status >= 300) {
      throw Object.assign(new Error(`Request failed with status code ${status}`), {
        response,
        config,
        isAxiosError: true,
      });
    }
    return response;
  }) as never;
}

beforeEach(() => {
  localStorage.clear();
  useUserStore.getState().logout();
  calls = [];
  reply = () => ({ status: 200, data: { success: true, message: '', data: {} } });
  installAdapter();
  setUnauthorizedHandler(() => {});
});

describe('api/client 契约', () => {
  it('TC-UNI-FE-001-POS:请求注入 New-Api-User 防串号头', async () => {
    useUserStore.getState().login({ userId: 42, username: 'u', role: 1, group: 'default' });
    reply = () => ({ status: 200, data: { success: true, message: '', data: { id: 42 } } });

    await dedupeGet('/api/user/self');

    expect(calls).toHaveLength(1);
    expect(calls[0]!.headers.get('New-Api-User')).toBe('42');
  });

  it('TC-UNI-FE-002-POS:HTTP 200 + success:false 必须转 reject ★', async () => {
    reply = () => ({ status: 200, data: { success: false, message: '余额不足' } });

    await expect(dedupeGet('/api/user/self')).rejects.toBeInstanceOf(ApiBusinessError);
    await expect(dedupeGet('/api/user/self')).rejects.toMatchObject({
      message: '余额不足',
    });
  });

  it('TC-UNI-FE-003-POS:HTTP 401 清理登录态并跳登录', async () => {
    const onUnauthorized = vi.fn();
    setUnauthorizedHandler(onUnauthorized);
    useUserStore.getState().login({ userId: 7, username: 'u', role: 1, group: 'default' });
    reply = () => ({ status: 401, data: { success: false, message: '未认证' } });

    await expect(dedupeGet('/api/user/self')).rejects.toBeInstanceOf(ApiHttpError);

    expect(onUnauthorized).toHaveBeenCalledTimes(1);
    expect(useUserStore.getState().isLoggedIn).toBe(false);
    expect(useUserStore.getState().userId).toBeNull();
  });

  it('TC-UNI-FE-004-POS:GET 去重 —— 并发同 URL 只发一次请求', async () => {
    reply = () => ({ status: 200, data: { success: true, message: '', data: { n: 1 } }, delayMs: 20 });

    const [a, b] = await Promise.all([dedupeGet('/api/status'), dedupeGet('/api/status')]);

    expect(calls).toHaveLength(1);
    expect(a).toEqual({ n: 1 });
    expect(b).toEqual({ n: 1 });
  });

  it('TC-UNI-FE-004-POS:请求完成后不再复用(可发起新请求)', async () => {
    reply = () => ({ status: 200, data: { success: true, message: '', data: {} } });
    await dedupeGet('/api/status');
    await dedupeGet('/api/status');
    expect(calls).toHaveLength(2);
  });

  it('TC-UNI-FE-005-NEG:网络错误与业务错误可区分', async () => {
    reply = () => {
      throw new Error('Network Error');
    };
    const networkErr = await dedupeGet('/api/status').catch((e: unknown) => e);
    expect(networkErr).toBeInstanceOf(ApiNetworkError);

    reply = () => ({ status: 200, data: { success: false, message: '参数错误' } });
    const bizErr = await dedupeGet('/api/status').catch((e: unknown) => e);
    expect(bizErr).toBeInstanceOf(ApiBusinessError);

    expect(networkErr).not.toBeInstanceOf(ApiBusinessError);
    expect(bizErr).not.toBeInstanceOf(ApiNetworkError);
  });

  it('非 2xx 且非 401 时抛出带状态码的 ApiHttpError', async () => {
    reply = () => ({ status: 500, data: { success: false, message: '服务器错误' } });
    const err = await dedupeGet('/api/status').catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiHttpError);
    expect((err as ApiHttpError).status).toBe(500);
  });
});
