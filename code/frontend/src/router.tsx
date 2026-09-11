/**
 * 路由注册。静态注册 + 守卫组件(Private / Admin / Root)。
 *
 * 覆盖 new-api 全部 24 个页面域(doc/system-design.md §1.3):
 * - auth ×3、console ×8、admin ×7、public ×6(含兜底)、chat ×2、pricing、playground、setup
 *
 * 侧边导航的模块可见性由 status store(服务端 `/api/status`)驱动,
 * 不再像 new-api 那样从 localStorage 读取。
 */
import { createBrowserRouter } from 'react-router-dom';

export const router = createBrowserRouter([
  // TODO(TDD): 按上述分组注册。建议结构:
  // { path: '/', element: <AppLayout/>, children: [ ...console, ...admin ] }
  // { path: '/login', element: <AuthLayout/>, children: [ ...auth ] }
  // { path: '*', element: <NotFound/> }
]);

/**
 * 权限守卫。role 不足时重定向到 /403,而非渲染空白。
 * 契约:role 语义 0 guest / 1 common / 10 admin / 100 root。
 */
export function RequireRole(_props: { minRole: number; children: React.ReactNode }) {
  // TODO(TDD): 从 userStore 读取 role;未登录跳 /login,角色不足跳 /403
  return null;
}
