/** `prefill_group` 域 API(合约归在分组域)。端点清单见 CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';
import type { PrefillGroupItem, PrefillGroupPayload } from '@/types';

/** `GET /api/prefill_group`:列表(AdminAuth)。 */
export function list(): Promise<{ items: PrefillGroupItem[]; total: number }> {
  return request<{ items: PrefillGroupItem[]; total: number }>({
    url: '/api/prefill_group',
    method: 'get',
  });
}

/** `POST /api/prefill_group`:新增。 */
export function create(payload: PrefillGroupPayload): Promise<{ id: number }> {
  return request<{ id: number }>({ url: '/api/prefill_group', method: 'post', data: payload });
}

/** `PUT /api/prefill_group`:修改(id 在 body)。 */
export function update(payload: PrefillGroupPayload): Promise<{ id: number }> {
  return request<{ id: number }>({ url: '/api/prefill_group', method: 'put', data: payload });
}

/** `DELETE /api/prefill_group/:id`:软删除。 */
export function remove(id: number): Promise<{ id: number }> {
  return request<{ id: number }>({ url: `/api/prefill_group/${id}`, method: 'delete' });
}
