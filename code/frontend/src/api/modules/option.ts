/** `option` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';

/** `GET /api/option/`(RootAuth):全部选项的 key → value 映射。 */
export function getOptions(): Promise<Record<string, string>> {
  return request<Record<string, string>>({ url: '/api/option/', method: 'get' });
}

/** `PUT /api/option/`(RootAuth):批量更新选项(值原样存储)。 */
export function updateOptions(map: Record<string, string>): Promise<{ updated: number }> {
  return request<{ updated: number }>({ url: '/api/option/', method: 'put', data: map });
}
