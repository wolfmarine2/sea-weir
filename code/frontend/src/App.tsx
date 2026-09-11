/**
 * 应用外壳。对应 C4 组件 `app_shell`。
 *
 * 职责:主题(亮/暗,antd algorithm)、i18n locale 同步、路由注册、全局错误边界。
 *
 * 注意(ADR-009):路由守卫**仅为体验层**。真正的权限以服务端逐端点鉴权为准,
 * 前端守卫的作用是避免用户进入注定 403 的页面,不承担安全职责。
 */
import { RouterProvider } from 'react-router-dom';
import { router } from './router';

export default function App() {
  // TODO(TDD): ConfigProvider 包裹(theme.algorithm 由 themeStore 驱动,locale 由 i18n 驱动)
  return <RouterProvider router={router} />;
}
