/**
 * 主布局:顶栏 + 侧边导航 + 内容区。
 *
 * 侧边导航项的显隐由 statusStore 的 features 驱动(服务端下发),
 * 可见性判断与角色守卫是两件事:features 控制「有没有这个模块」,
 * role 控制「这个用户能不能进」。
 *
 * 现状(骨架阶段):先给顶栏 + 内容区,保证应用可渲染;
 * 侧边导航、主题/语言切换、用户菜单待 TDD 补齐。
 */
import { useEffect } from 'react';
import { Layout, Typography } from 'antd';
import { Outlet } from 'react-router-dom';
import { useStatusStore } from '@/stores/status';

const { Header, Content } = Layout;

export default function AppLayout() {
  const systemName = useStatusStore((s) => s.systemName);
  const fetchStatus = useStatusStore((s) => s.fetch);

  useEffect(() => {
    void fetchStatus();
  }, [fetchStatus]);

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Header style={{ display: 'flex', alignItems: 'center' }}>
        <Typography.Title level={4} style={{ color: '#fff', margin: 0 }}>
          {systemName || 'sea-weir'}
        </Typography.Title>
      </Header>
      <Content style={{ padding: 24 }}>
        <Outlet />
      </Content>
    </Layout>
  );
}
