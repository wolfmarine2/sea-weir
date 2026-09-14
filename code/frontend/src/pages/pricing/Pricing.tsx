/**
 * 模型广场 / 定价。对接 `GET /api/pricing`(公开,可匿名)。
 *
 * 展示可用模型及其倍率(模型倍率/补全倍率/缓存倍率/按次价格)与分组倍率表。
 * 计费口径:quota = tokens × ModelRatio × GroupRatio(500000 quota = $1)。
 */
import { useEffect, useState } from 'react';
import { Card, Descriptions, Space, Table, Tag, Typography } from 'antd';
import type { TableColumnsType } from 'antd';

import { api } from '@/api';
import type { PricingModel, PricingView } from '@/api/modules/pricing';

export default function Pricing() {
  const [loading, setLoading] = useState(true);
  const [view, setView] = useState<PricingView | null>(null);

  useEffect(() => {
    let alive = true;
    api.pricing
      .getPricing()
      .then((v) => {
        if (alive) setView(v);
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, []);

  const columns: TableColumnsType<PricingModel> = [
    { title: '模型', dataIndex: 'model_name' },
    { title: '模型倍率', dataIndex: 'model_ratio', width: 110 },
    { title: '补全倍率', dataIndex: 'completion_ratio', width: 110 },
    { title: '缓存倍率', dataIndex: 'cache_ratio', width: 110 },
    {
      title: '按次价格',
      dataIndex: 'model_price',
      width: 110,
      render: (value: string | null) => value ?? '-',
    },
  ];

  return (
    <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
      <Card loading={loading} title={<Typography.Text strong>模型广场</Typography.Text>}>
        <Table<PricingModel>
          columns={columns}
          dataSource={view?.models ?? []}
          rowKey="model_name"
          size="small"
          pagination={false}
          locale={{ emptyText: '暂无可用模型(请先配置渠道)' }}
        />
      </Card>

      <Card loading={loading} title="分组倍率">
        <Descriptions column={1} size="small">
          {Object.entries(view?.group_ratio ?? {}).map(([group, ratio]) => (
            <Descriptions.Item key={group} label={group}>
              <Tag>{ratio}</Tag>
            </Descriptions.Item>
          ))}
          {Object.keys(view?.group_ratio ?? {}).length === 0 && (
            <Descriptions.Item label="-">未配置分组倍率</Descriptions.Item>
          )}
        </Descriptions>
        <Typography.Paragraph type="secondary" style={{ marginBottom: 0 }}>
          计费口径:quota = tokens × 模型倍率 × 分组倍率;{view?.quota_per_unit ?? 500000} quota = $1。
        </Typography.Paragraph>
      </Card>
    </Space>
  );
}
