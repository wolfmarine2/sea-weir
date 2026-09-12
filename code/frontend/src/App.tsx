/**
 * 应用外壳。对应 C4 组件 `app_shell`。
 *
 * 职责:主题(亮/暗,antd algorithm)、i18n locale 同步、路由注册、全局错误边界。
 *
 * 注意(ADR-009):路由守卫**仅为体验层**。真正的权限以服务端逐端点鉴权为准,
 * 前端守卫的作用是避免用户进入注定 403 的页面,不承担安全职责。
 */
import { ConfigProvider, theme as antdTheme } from 'antd';
import zhCN from 'antd/locale/zh_CN';
import { RouterProvider } from 'react-router-dom';
import { router } from './router';
import { useThemeStore } from '@/stores/theme';

export default function App() {
  const mode = useThemeStore((s) => s.mode);

  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        algorithm: mode === 'dark' ? antdTheme.darkAlgorithm : antdTheme.defaultAlgorithm,
      }}
    >
      <RouterProvider router={router} />
    </ConfigProvider>
  );
}
