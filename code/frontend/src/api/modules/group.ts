/** `group` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';
import type { GroupItem, GroupPayload } from '@/types';

/** `GET /api/group/`:分组列表(AdminAuth)。 */
export function list(): Promise<{ items: GroupItem[]; total: number }> {
  return request<{ items: GroupItem[]; total: number }>({
    url: '/api/group/',
    method: 'get',
  });
}

/** `POST /api/group/`:新增分组。 */
export function create(payload: GroupPayload): Promise<{ name: string }> {
  return request<{ name: string }>({ url: '/api/group/', method: 'post', data: payload });
}

/** `PUT /api/group/`:修改分组(名称即身份,不可改名)。 */
export function update(payload: GroupPayload): Promise<{ name: string }> {
  return request<{ name: string }>({ url: '/api/group/', method: 'put', data: payload });
}

/** `DELETE /api/group/:name`:删除分组(使用中/ default 会被后端拒绝)。 */
export function remove(name: string): Promise<{ name: string }> {
  return request<{ name: string }>({
    url: `/api/group/${encodeURIComponent(name)}`,
    method: 'delete',
  });
}
