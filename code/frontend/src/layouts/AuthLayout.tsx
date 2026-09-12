/** 认证页布局。登录/注册/OAuth 回调共用,无侧边导航。 */
import { Layout } from 'antd';
import { Outlet } from 'react-router-dom';

export default function AuthLayout() {
  return (
    <Layout
      style={{
        minHeight: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: '#f5f5f5',
      }}
    >
      <div style={{ width: 380 }}>
        <Outlet />
      </div>
    </Layout>
  );
}
