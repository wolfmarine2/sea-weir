/** `dashboard` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';

/** 日志统计(总额度 + 最近 60s rpm/tpm)。 */
export interface LogStat {
  total_quota: number;
  rpm: number;
  tpm: number;
}

/** `GET /api/data/`(AdminAuth):管理面概览。 */
export interface Overview {
  user_count: number;
  channel_count: number;
  token_count: number;
  total_quota: number;
  rpm: number;
  tpm: number;
}

export function getOverview(): Promise<Overview> {
  return request<Overview>({ url: '/api/data/', method: 'get' });
}

/** `GET /api/log/self/stat`(UserAuth):本人统计。 */
export function getSelfStat(): Promise<LogStat> {
  return request<LogStat>({ url: '/api/log/self/stat', method: 'get' });
}
