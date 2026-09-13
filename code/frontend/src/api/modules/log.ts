/** `log` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';
import type { LogItem, PageInfo } from '@/types';

export interface LogQueryParams {
  p: number;
  page_size: number;
  type?: number | undefined;
  model_name?: string | undefined;
  start_timestamp?: number | undefined;
  end_timestamp?: number | undefined;
}

/** `GET /api/log/self`:本人消费日志分页。 */
export function selfLogs(params: LogQueryParams): Promise<PageInfo<LogItem>> {
  return request<PageInfo<LogItem>>({ url: '/api/log/self', method: 'get', params });
}

/** `GET /api/log/`:全量日志分页(AdminAuth)。 */
export function allLogs(params: LogQueryParams): Promise<PageInfo<LogItem>> {
  return request<PageInfo<LogItem>>({ url: '/api/log/', method: 'get', params });
}
