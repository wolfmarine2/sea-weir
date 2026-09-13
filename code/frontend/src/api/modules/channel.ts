/** `channel` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';
import type { ChannelItem, ChannelPayload, PageInfo } from '@/types';

/** `GET /api/channel/`:渠道分页(key 脱敏,AdminAuth)。 */
export function list(params: { p: number; page_size: number }): Promise<PageInfo<ChannelItem>> {
  return request<PageInfo<ChannelItem>>({ url: '/api/channel/', method: 'get', params });
}

/** `POST /api/channel/`:创建渠道并同步 abilities。 */
export function create(payload: ChannelPayload): Promise<ChannelItem> {
  return request<ChannelItem>({ url: '/api/channel/', method: 'post', data: payload });
}

/** `PUT /api/channel/`:更新渠道(id 在 body)。key 省略则沿用原密钥。 */
export function update(payload: ChannelPayload & { id: number }): Promise<ChannelItem> {
  return request<ChannelItem>({ url: '/api/channel/', method: 'put', data: payload });
}

/** `DELETE /api/channel/:id`:删除渠道并清理 abilities。 */
export function remove(id: number): Promise<unknown> {
  return request<unknown>({ url: `/api/channel/${id}`, method: 'delete' });
}
