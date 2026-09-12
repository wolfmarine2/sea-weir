/**
 * 首装向导。未初始化时创建 root,对接 `POST /api/setup`,成功后自动登录。
 *
 * 契约:已初始化时后端返回业务错误并拒绝;口令至少 6 位。
 */
import { useState } from 'react';
import { Alert, Button, Card, Form, Input, Typography } from 'antd';
import { useNavigate } from 'react-router-dom';

import { api } from '@/api';
import { useStatusStore } from '@/stores/status';
import { useUserStore } from '@/stores/user';

interface SetupForm {
  username: string;
  password: string;
  confirm: string;
}

export default function Setup() {
  const navigate = useNavigate();
  const fetchStatus = useStatusStore((s) => s.fetch);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const onFinish = async (values: SetupForm) => {
    setLoading(true);
    setError('');
    try {
      await api.system.postSetup({ username: values.username, password: values.password });
      const user = await api.user.login({ username: values.username, password: values.password });
      useUserStore.getState().login({
        userId: user.id,
        username: user.username,
        role: user.role,
        group: user.group,
      });
      await fetchStatus();
      navigate('/');
    } catch (e) {
      setError(e instanceof Error ? e.message : '初始化失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    <Card>
      <Typography.Title level={4} style={{ textAlign: 'center', marginTop: 0 }}>
        sea-weir 首装
      </Typography.Title>
      <Typography.Paragraph type="secondary" style={{ textAlign: 'center' }}>
        创建第一个管理员账号(root)
      </Typography.Paragraph>

      {error && <Alert type="error" showIcon message={error} style={{ marginBottom: 16 }} />}

      <Form layout="vertical" onFinish={onFinish} disabled={loading} requiredMark={false}>
        <Form.Item
          name="username"
          label="管理员用户名"
          rules={[{ required: true, message: '请输入用户名' }]}
        >
          <Input autoComplete="username" placeholder="root" />
        </Form.Item>
        <Form.Item
          name="password"
          label="口令"
          rules={[{ required: true, min: 6, message: '口令至少 6 位' }]}
        >
          <Input.Password autoComplete="new-password" />
        </Form.Item>
        <Form.Item
          name="confirm"
          label="确认口令"
          dependencies={['password']}
          rules={[
            { required: true, message: '请再次输入口令' },
            ({ getFieldValue }) => ({
              validator(_rule, value: string) {
                if (!value || getFieldValue('password') === value) {
                  return Promise.resolve();
                }
                return Promise.reject(new Error('两次口令不一致'));
              },
            }),
          ]}
        >
          <Input.Password autoComplete="new-password" />
        </Form.Item>
        <Button type="primary" htmlType="submit" block loading={loading}>
          初始化并登录
        </Button>
      </Form>
    </Card>
  );
}
