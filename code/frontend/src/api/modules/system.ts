/** `system` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';
import type { SetupInfo, SystemStatus } from '@/types';

/** `GET /api/status`(公开):全站配置,status store 的唯一来源。 */
export function getStatus(): Promise<SystemStatus> {
  return request<SystemStatus>({ url: '/api/status', method: 'get' });
}

/** `GET /api/setup`(公开):首装状态。 */
export function getSetup(): Promise<SetupInfo> {
  return request<SetupInfo>({ url: '/api/setup', method: 'get' });
}

/** `POST /api/setup`(公开):未初始化时创建 root。已初始化后端拒绝。 */
export function postSetup(payload: {
  username: string;
  password: string;
}): Promise<{ id: number; username: string }> {
  return request<{ id: number; username: string }>({
    url: '/api/setup',
    method: 'post',
    data: payload,
  });
}

/** `GET /api/notice`(公开):站内公告文案(字符串,可含 Markdown/HTML)。 */
export function getNotice(): Promise<string> {
  return request<string>({ url: '/api/notice', method: 'get' });
}
