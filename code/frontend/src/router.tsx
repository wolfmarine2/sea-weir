/**
 * 路由注册。静态注册 + 守卫组件(Private / Admin / Root)。
 *
 * 覆盖 new-api 全部 24 个页面域(doc/system-design.md §1.3):
 * - auth ×3、console ×8、admin ×7、public ×6(含兜底)、chat ×2、pricing、playground、setup
 *
 * 侧边导航的模块可见性由 status store(服务端 `/api/status`)驱动,
 * 不再像 new-api 那样从 localStorage 读取。
 *
 * 现状(骨架阶段):已注册「布局 + 首页 + 首装 + 登录 + 404」,
 * 其余页面域待 TDD 逐个补齐。
 */
import type { ReactNode } from 'react';
import { Navigate, createBrowserRouter } from 'react-router-dom';
import AppLayout from '@/layouts/AppLayout';
import AuthLayout from '@/layouts/AuthLayout';
import ChannelPage from '@/pages/admin/ChannelPage';
import GroupPage from '@/pages/admin/GroupPage';
import SettingPage from '@/pages/admin/SettingPage';
import UserPage from '@/pages/admin/UserPage';
import Login from '@/pages/auth/Login';
import Dashboard from '@/pages/console/Dashboard';
import LogPage from '@/pages/console/LogPage';
import TokenPage from '@/pages/console/TokenPage';
import Home from '@/pages/public/Home';
import NotFound from '@/pages/public/NotFound';
import Pricing from '@/pages/pricing/Pricing';
import Setup from '@/pages/setup/Setup';
import { useUserStore } from '@/stores/user';

export const router = createBrowserRouter([
  {
    path: '/',
    element: <AppLayout />,
    children: [
      { index: true, element: <Home /> },
      {
        path: 'console/token',
        element: (
          <RequireRole minRole={1}>
            <TokenPage />
          </RequireRole>
        ),
      },
      {
        path: 'console/log',
        element: (
          <RequireRole minRole={1}>
            <LogPage />
          </RequireRole>
        ),
      },
      {
        path: 'console/dashboard',
        element: (
          <RequireRole minRole={1}>
            <Dashboard />
          </RequireRole>
        ),
      },
      // 模型广场:公开(可匿名)
      { path: 'pricing', element: <Pricing /> },
      {
        path: 'admin/channel',
        element: (
          <RequireRole minRole={10}>
            <ChannelPage />
          </RequireRole>
        ),
      },
      {
        path: 'admin/group',
        element: (
          <RequireRole minRole={10}>
            <GroupPage />
          </RequireRole>
        ),
      },
      {
        path: 'admin/setting',
        element: (
          <RequireRole minRole={100}>
            <SettingPage />
          </RequireRole>
        ),
      },
      {
        path: 'admin/user',
        element: (
          <RequireRole minRole={10}>
            <UserPage />
          </RequireRole>
        ),
      },
    ],
  },
  {
    // 认证类页面共用无侧边栏布局
    element: <AuthLayout />,
    children: [
      { path: '/setup', element: <Setup /> },
      { path: '/login', element: <Login /> },
    ],
  },
  // TODO(TDD): 继续注册
  //   - 公开:/pricing、/about、/register、/oauth/*
  //   - 登录后:/console/*、/admin/*、/chat/*(用 <RequireRole> 包裹)
  { path: '*', element: <NotFound /> },
]);

/**
 * 权限守卫。role 不足时重定向到 /403,而非渲染空白。
 * 契约:role 语义 0 guest / 1 common / 10 admin / 100 root。
 */
export function RequireRole({ minRole, children }: { minRole: number; children: ReactNode }) {
  const isLoggedIn = useUserStore((s) => s.isLoggedIn);
  const role = useUserStore((s) => s.role);

  if (!isLoggedIn) {
    return <Navigate to="/login" replace />;
  }
  if (role < minRole) {
    return <Navigate to="/403" replace />;
  }
  return <>{children}</>;
}
