/** `user` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';
import type { User } from '@/types';

/** `POST /api/user/login`(公开):账密登录,服务端下发签名会话 Cookie。 */
export function login(payload: { username: string; password: string }): Promise<User> {
  return request<User>({ url: '/api/user/login', method: 'post', data: payload });
}

/** `GET /api/user/logout`:清会话 Cookie(幂等)。 */
export function logout(): Promise<unknown> {
  return request<unknown>({ url: '/api/user/logout', method: 'get' });
}

/**
 * `GET /api/user/self`(UserAuth)。
 * 注意:受保护端点要求 `New-Api-User` 头,由 client 拦截器从 userStore 注入。
 */
export function getSelf(): Promise<User> {
  return request<User>({ url: '/api/user/self', method: 'get' });
}
