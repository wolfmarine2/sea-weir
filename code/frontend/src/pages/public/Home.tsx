/**
 * 首页。运营文案,对接 /api/home_page_content。
 *
 * 现状(骨架阶段):先渲染一个真实页面,数据取自 statusStore(即 `GET /api/status`),
 * 用于验证「浏览器 → nginx 反代 → 后端」这条链路通;运营文案待接入。
 */
import { Alert, Card, Descriptions, Space, Tag } from 'antd';
import { useStatusStore } from '@/stores/status';

export default function Home() {
  const loaded = useStatusStore((s) => s.loaded);
  const systemName = useStatusStore((s) => s.systemName);
  const dbReady = useStatusStore((s) => s.features.db_ready ?? false);

  return (
    <Card title={systemName || 'sea-weir'} loading={!loaded} style={{ maxWidth: 720 }}>
      <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
        <Descriptions column={1} size="small">
          <Descriptions.Item label="系统名称">{systemName || 'sea-weir'}</Descriptions.Item>
          <Descriptions.Item label="数据库">
            <Tag color={dbReady ? 'green' : 'orange'}>{dbReady ? '已连接' : '未连接'}</Tag>
          </Descriptions.Item>
        </Descriptions>
        <Alert
          type="info"
          showIcon
          message="骨架阶段"
          description="应用外壳已可渲染。登录、控制台、管理端等页面待 TDD 阶段实现。"
        />
      </Space>
    </Card>
  );
}
