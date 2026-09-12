/**
 * 登录。账密对接 `POST /api/user/login`;成功后写入 userStore 并跳转首页。
 *
 * 待补:2FA、Passkey、Turnstile(见 handlers/user.rs 的对应端点)。
 */
import { useState } from 'react';
import { Alert, Button, Card, Form, Input, Typography } from 'antd';
import { Link, useNavigate } from 'react-router-dom';

import { api } from '@/api';
import { useStatusStore } from '@/stores/status';
import { useUserStore } from '@/stores/user';

interface LoginForm {
  username: string;
  password: string;
}

export default function Login() {
  const navigate = useNavigate();
  const fetchStatus = useStatusStore((s) => s.fetch);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const onFinish = async (values: LoginForm) => {
    setLoading(true);
    setError('');
    try {
      const user = await api.user.login(values);
      useUserStore.getState().login({
        userId: user.id,
        username: user.username,
        role: user.role,
        group: user.group,
      });
      await fetchStatus();
      navigate('/');
    } catch (e) {
      setError(e instanceof Error ? e.message : '登录失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    <Card>
      <Typography.Title level={4} style={{ textAlign: 'center', marginTop: 0 }}>
        sea-weir 登录
      </Typography.Title>

      {error && <Alert type="error" showIcon message={error} style={{ marginBottom: 16 }} />}

      <Form layout="vertical" onFinish={onFinish} disabled={loading} requiredMark={false}>
        <Form.Item name="username" label="用户名" rules={[{ required: true, message: '请输入用户名' }]}>
          <Input autoComplete="username" />
        </Form.Item>
        <Form.Item name="password" label="口令" rules={[{ required: true, message: '请输入口令' }]}>
          <Input.Password autoComplete="current-password" />
        </Form.Item>
        <Button type="primary" htmlType="submit" block loading={loading}>
          登录
        </Button>
      </Form>

      <Typography.Paragraph style={{ textAlign: 'center', marginTop: 16, marginBottom: 0 }}>
        <Link to="/setup">首次部署?前往首装</Link>
      </Typography.Paragraph>
    </Card>
  );
}
