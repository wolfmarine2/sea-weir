/**
 * 主布局:顶栏 + 侧边导航 + 内容区。
 *
 * 侧边导航项的显隐由 statusStore 的 features 驱动(服务端下发),
 * 可见性判断与角色守卫是两件事:features 控制「有没有这个模块」,
 * role 控制「这个用户能不能进」。
 *
 * 现状:顶栏导航(首页/令牌)+ 内容区;侧边栏、主题/语言切换、用户菜单待补。
 */
import { useEffect } from 'react';
import { Layout, Menu, Typography } from 'antd';
import { Link, Outlet, useLocation } from 'react-router-dom';
import { useStatusStore } from '@/stores/status';

const { Header, Content } = Layout;

const NAV = [
  { key: '/', label: <Link to="/">首页</Link> },
  { key: '/console/token', label: <Link to="/console/token">令牌</Link> },
];

export default function AppLayout() {
  const systemName = useStatusStore((s) => s.systemName);
  const fetchStatus = useStatusStore((s) => s.fetch);
  const location = useLocation();

  useEffect(() => {
    void fetchStatus();
  }, [fetchStatus]);

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Header style={{ display: 'flex', alignItems: 'center', gap: 24 }}>
        <Typography.Title level={4} style={{ color: '#fff', margin: 0, whiteSpace: 'nowrap' }}>
          {systemName || 'sea-weir'}
        </Typography.Title>
        <Menu
          theme="dark"
          mode="horizontal"
          selectedKeys={[location.pathname]}
          items={NAV}
          style={{ flex: 1, minWidth: 0 }}
        />
      </Header>
      <Content style={{ padding: 24 }}>
        <Outlet />
      </Content>
    </Layout>
  );
}
