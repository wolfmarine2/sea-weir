/**
 * 首页。展示全站状态(取自 statusStore,即 `GET /api/status`)与当前登录态,
 * 并给出首装 / 登录 / 登出入口。
 *
 * 待补:运营文案(`/api/home_page_content`)、控制台概览。
 */
import { useState } from 'react';
import { Button, Card, Descriptions, Space, Tag } from 'antd';
import { Link, useNavigate } from 'react-router-dom';

import { api } from '@/api';
import { useStatusStore } from '@/stores/status';
import { useUserStore } from '@/stores/user';

export default function Home() {
  const navigate = useNavigate();
  const loaded = useStatusStore((s) => s.loaded);
  const systemName = useStatusStore((s) => s.systemName);
  const setup = useStatusStore((s) => s.features.setup ?? false);
  const dbReady = useStatusStore((s) => s.features.db_ready ?? false);
  const isLoggedIn = useUserStore((s) => s.isLoggedIn);
  const username = useUserStore((s) => s.username);
  const [busy, setBusy] = useState(false);

  const onLogout = async () => {
    setBusy(true);
    try {
      await api.user.logout();
    } catch {
      // 服务端登出失败也要清本地态(契约:logout 幂等)
    }
    useUserStore.getState().logout();
    setBusy(false);
    navigate('/login');
  };

  return (
    <Card title={systemName || 'sea-weir'} loading={!loaded} style={{ maxWidth: 640 }}>
      <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
        <Descriptions column={1} size="small">
          <Descriptions.Item label="系统名称">{systemName || 'sea-weir'}</Descriptions.Item>
          <Descriptions.Item label="数据库">
            <Tag color={dbReady ? 'green' : 'orange'}>{dbReady ? '已连接' : '未连接'}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label="首装">
            <Tag color={setup ? 'green' : 'orange'}>{setup ? '已完成' : '待初始化'}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label="登录态">
            {isLoggedIn ? `已登录:${username}` : '未登录'}
          </Descriptions.Item>
        </Descriptions>

        {!setup && (
          <Link to="/setup">
            <Button type="primary">开始首装</Button>
          </Link>
        )}
        {setup && !isLoggedIn && (
          <Link to="/login">
            <Button type="primary">登录</Button>
          </Link>
        )}
        {isLoggedIn && (
          <Button loading={busy} onClick={onLogout}>
            退出登录
          </Button>
        )}
      </Space>
    </Card>
  );
}
