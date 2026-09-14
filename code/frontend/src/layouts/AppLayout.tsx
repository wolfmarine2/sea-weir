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
import { useUserStore } from '@/stores/user';
import { ROLE } from '@/types';

const { Header, Content } = Layout;

export default function AppLayout() {
  const systemName = useStatusStore((s) => s.systemName);
  const fetchStatus = useStatusStore((s) => s.fetch);
  const role = useUserStore((s) => s.role);
  const location = useLocation();

  useEffect(() => {
    void fetchStatus();
  }, [fetchStatus]);

  // 导航可见性:与路由守卫一样只是体验层,真正权限在服务端。
  const nav = [
    { key: '/', label: <Link to="/">首页</Link> },
    { key: '/console/token', label: <Link to="/console/token">令牌</Link> },
    { key: '/console/log', label: <Link to="/console/log">日志</Link> },
    ...(role >= ROLE.ADMIN
      ? [
          { key: '/admin/channel', label: <Link to="/admin/channel">渠道</Link> },
          { key: '/admin/user', label: <Link to="/admin/user">用户</Link> },
        ]
      : []),
    ...(role >= ROLE.ROOT
      ? [{ key: '/admin/setting', label: <Link to="/admin/setting">设置</Link> }]
      : []),
  ];

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
          items={nav}
          style={{ flex: 1, minWidth: 0 }}
        />
      </Header>
      <Content style={{ padding: 24 }}>
        <Outlet />
      </Content>
    </Layout>
  );
}
