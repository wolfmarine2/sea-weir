/**
 * 首页。展示全站状态(取自 statusStore,即 `GET /api/status`)、站内公告与快速上手引导。
 *
 * 待补:运营文案(`/api/home_page_content`)展示区。
 */
import { useEffect, useState } from 'react';
import { Alert, Button, Card, Descriptions, Space, Steps, Tag, Typography } from 'antd';
import { Link, useNavigate } from 'react-router-dom';

import { api } from '@/api';
import { useStatusStore } from '@/stores/status';
import { useUserStore } from '@/stores/user';
import { ROLE } from '@/types';

export default function Home() {
  const navigate = useNavigate();
  const loaded = useStatusStore((s) => s.loaded);
  const systemName = useStatusStore((s) => s.systemName);
  const setup = useStatusStore((s) => s.features.setup ?? false);
  const dbReady = useStatusStore((s) => s.features.db_ready ?? false);
  const isLoggedIn = useUserStore((s) => s.isLoggedIn);
  const username = useUserStore((s) => s.username);
  const role = useUserStore((s) => s.role);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');

  useEffect(() => {
    let alive = true;
    api.system
      .getNotice()
      .then((text) => {
        if (alive) setNotice(text);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);

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

  const currentStep = !setup ? 0 : !isLoggedIn ? 1 : 2;

  return (
    <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
      {notice.trim() !== '' && <Alert type="info" showIcon message={notice} />}

      <Card title={systemName || 'sea-weir'} loading={!loaded} style={{ maxWidth: 720 }}>
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

      <Card title="快速上手" style={{ maxWidth: 720 }}>
        <Steps
          direction="vertical"
          size="small"
          current={currentStep}
          items={[
            {
              title: '完成首装',
              description: setup ? '已创建 root 管理员' : '未初始化,点击「开始首装」创建管理员',
            },
            {
              title: '登录控制台',
              description: isLoggedIn ? `已登录:${username}` : '登录后即可创建令牌',
            },
            {
              title: '配置渠道与创建令牌',
              description: (
                <Space direction="vertical" size={4}>
                  <span>
                    管理员在「渠道」页添加上游平台(填 Base URL 与密钥),系统自动同步 abilities;
                    在「令牌」页创建 sk-token。
                  </span>
                  <Space>
                    {role >= ROLE.ADMIN && <Link to="/admin/channel">去配置渠道</Link>}
                    <Link to="/console/token">去创建令牌</Link>
                  </Space>
                </Space>
              ),
            },
            {
              title: '调用网关',
              description: (
                <Typography.Paragraph style={{ marginBottom: 0 }}>
                  <Typography.Text code copyable>
                    {'curl http://<网关地址>/v1/chat/completions -H "Authorization: Bearer sk-xxx" -H "Content-Type: application/json" -d \'{"model":"gpt-4o","messages":[{"role":"user","content":"hi"}]}\''}
                  </Typography.Text>
                </Typography.Paragraph>
              ),
            },
          ]}
        />
      </Card>
    </Space>
  );
}
