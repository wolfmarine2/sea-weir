/** `token` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';
import type { PageInfo, TokenItem } from '@/types';

/** `GET /api/token/`:本人令牌分页(key 已脱敏)。 */
export function list(params: { p: number; page_size: number }): Promise<PageInfo<TokenItem>> {
  return request<PageInfo<TokenItem>>({ url: '/api/token/', method: 'get', params });
}

export interface CreateTokenPayload {
  name: string;
  remain_quota?: number;
  unlimited_quota?: boolean;
  expired_time?: number;
  model_limits_enabled?: boolean;
  model_limits?: string;
  allow_ips?: string;
  group?: string;
  cross_group_retry?: boolean;
}

/** `POST /api/token/`:创建令牌(响应 key 已脱敏)。 */
export function create(payload: CreateTokenPayload): Promise<TokenItem> {
  return request<TokenItem>({ url: '/api/token/', method: 'post', data: payload });
}

/** `PUT /api/token/`:全量更新(id 在 body)。 */
export function update(payload: { id: number } & Partial<CreateTokenPayload> & { status?: number }): Promise<TokenItem> {
  return request<TokenItem>({ url: '/api/token/', method: 'put', data: payload });
}

/** `DELETE /api/token/:id`:软删除。 */
export function remove(id: number): Promise<unknown> {
  return request<unknown>({ url: `/api/token/${id}`, method: 'delete' });
}

/** `POST /api/token/:id/key`:揭示完整 key(**唯一**明文出口)。 */
export function revealKey(id: number): Promise<{ key: string }> {
  return request<{ key: string }>({ url: `/api/token/${id}/key`, method: 'post' });
}
