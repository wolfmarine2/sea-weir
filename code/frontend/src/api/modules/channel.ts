/** `channel` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';
import type { ChannelItem, ChannelPayload, ChannelTypeOption, PageInfo } from '@/types';

/** `GET /api/channel/`:渠道分页(key 脱敏,AdminAuth)。 */
export function list(params: { p: number; page_size: number }): Promise<PageInfo<ChannelItem>> {
  return request<PageInfo<ChannelItem>>({ url: '/api/channel/', method: 'get', params });
}

/** `GET /api/channel/types`:渠道类型目录(编号 + 展示名 + 默认 Base URL)。 */
export function types(): Promise<{ items: ChannelTypeOption[]; total: number }> {
  return request<{ items: ChannelTypeOption[]; total: number }>({
    url: '/api/channel/types',
    method: 'get',
  });
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

/** `GET /api/channel/update_balance/:id`:刷新单渠道余额(对接上游平台)。 */
export function updateBalance(
  id: number,
): Promise<{ id: number; balance: number; balance_updated_time: number }> {
  return request({
    url: `/api/channel/update_balance/${id}`,
    method: 'get',
  });
}

/** `GET /api/channel/update_balance`:刷新全部渠道余额(尽力而为)。 */
export function updateAllBalances(): Promise<{
  results: Array<{ id: number; name: string; balance?: number; error?: string }>;
}> {
  return request({ url: '/api/channel/update_balance', method: 'get' });
}

/** `GET /api/channel/fetch_models/:id`:回源拉取该渠道的上游模型列表。 */
export function fetchModels(id: number): Promise<{ id: number; models: string[] }> {
  return request({ url: `/api/channel/fetch_models/${id}`, method: 'get' });
}

/** `GET /api/channel/test/:id`:测试单渠道连通性。 */
export function testChannel(id: number): Promise<{
  id: number;
  success: boolean;
  response_time: number;
  message: string;
}> {
  return request({ url: `/api/channel/test/${id}`, method: 'get' });
}

/** `GET /api/channel/test`:测试全部渠道。 */
export function testAllChannels(): Promise<{
  results: Array<{ id: number; name: string; success: boolean; response_time?: number; error?: string }>;
}> {
  return request({ url: '/api/channel/test', method: 'get' });
}
