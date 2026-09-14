/** `user` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';
import type { PageInfo, User } from '@/types';

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

// ───────────────────────── 管理面:用户访问控制(AdminAuth)─────────────────────────

/** `GET /api/user/`:用户分页列表。 */
export function listUsers(params: { p: number; page_size: number }): Promise<PageInfo<User>> {
  return request<PageInfo<User>>({ url: '/api/user/', method: 'get', params });
}

export interface CreateUserPayload {
  username: string;
  password: string;
  role?: number | undefined;
  display_name?: string | undefined;
  group?: string | undefined;
}

/** `POST /api/user/`:创建用户(额度为 0)。 */
export function createUser(payload: CreateUserPayload): Promise<User> {
  return request<User>({ url: '/api/user/', method: 'post', data: payload });
}

export interface UpdateUserPayload {
  id: number;
  role?: number | undefined;
  status?: number | undefined;
  display_name?: string | undefined;
  group?: string | undefined;
  password?: string | undefined;
}

/** `PUT /api/user/`:更新角色/状态/显示名/分组/密码。 */
export function updateUser(payload: UpdateUserPayload): Promise<User> {
  return request<User>({ url: '/api/user/', method: 'put', data: payload });
}

/** `DELETE /api/user/:id`:软删除用户。 */
export function deleteUser(id: number): Promise<unknown> {
  return request<unknown>({ url: `/api/user/${id}`, method: 'delete' });
}
