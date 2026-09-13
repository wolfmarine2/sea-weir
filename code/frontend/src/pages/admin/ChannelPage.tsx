/**
 * 管理端-渠道。列表 / 创建 / 启停 / 删除(AdminAuth)。
 *
 * 契约:列表不回传渠道密钥;更新不传 key 时后端沿用原密钥。
 * 待补:批量/标签、多 key 管理、测试渠道、余额探测、上游模型拉取。
 */
import { useCallback, useState } from 'react';
import {
  App as AntApp,
  Button,
  Card,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Space,
  Switch,
  Tag,
  Typography,
} from 'antd';
import type { TableColumnsType } from 'antd';
import dayjs from 'dayjs';

import { api } from '@/api';
import { DataTable } from '@/components/table';
import { useTableData } from '@/hooks';
import type { ChannelItem, ChannelPayload } from '@/types';

type ChannelFilters = Record<string, never>;

interface ChannelForm {
  name: string;
  type: number;
  key?: string;
  models: string;
  group: string;
  base_url?: string;
  priority?: number;
  weight?: number;
  enabled: boolean;
}

const STATUS: Record<number, { color: string; text: string }> = {
  1: { color: 'green', text: '启用' },
  2: { color: 'default', text: '手动禁用' },
  3: { color: 'orange', text: '自动禁用' },
};

export default function ChannelPage() {
  const { message, modal } = AntApp.useApp();
  const [form] = Form.useForm<ChannelForm>();
  const [modalOpen, setModalOpen] = useState(false);
  const [saving, setSaving] = useState(false);

  const fetcher = useCallback(
    ({ p, page_size }: { p: number; page_size: number }) => api.channel.list({ p, page_size }),
    [],
  );
  const table = useTableData<ChannelItem, ChannelFilters>({ fetcher, initialFilters: {} });

  const onCreate = async (values: ChannelForm) => {
    setSaving(true);
    try {
      const payload: ChannelPayload = {
        type: values.type,
        name: values.name,
        models: values.models,
        group: values.group,
        key: values.key,
        base_url: values.base_url,
        priority: values.priority,
        weight: values.weight,
        status: values.enabled ? 1 : 2,
      };
      await api.channel.create(payload);
      setModalOpen(false);
      form.resetFields();
      message.success('渠道已创建');
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '创建失败');
    } finally {
      setSaving(false);
    }
  };

  const onToggle = async (record: ChannelItem) => {
    try {
      await api.channel.update({
        id: record.id,
        type: record.type,
        name: record.name,
        models: record.models,
        group: record.group,
        status: record.status === 1 ? 2 : 1,
      });
      message.success('状态已更新');
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '更新失败');
    }
  };

  const onDelete = async (id: number) => {
    try {
      await api.channel.remove(id);
      message.success('已删除');
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '删除失败');
    }
  };

  const onRefreshBalance = async (record: ChannelItem) => {
    try {
      const { balance } = await api.channel.updateBalance(record.id);
      message.success(`余额已刷新:$${balance.toFixed(2)}`);
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '余额刷新失败');
    }
  };

  const onRefreshAll = async () => {
    try {
      const { results } = await api.channel.updateAllBalances();
      const ok = results.filter((r) => r.error === undefined).length;
      message.success(`已刷新 ${ok}/${results.length} 个渠道余额`);
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '余额刷新失败');
    }
  };

  const onTest = async (record: ChannelItem) => {
    try {
      const r = await api.channel.testChannel(record.id);
      if (r.success) {
        message.success(`测试通过,耗时 ${r.response_time}ms`);
      } else {
        message.warning(`测试失败:${r.message}`);
      }
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '测试失败');
    }
  };

  const onFetchModels = async (record: ChannelItem) => {
    try {
      const { models } = await api.channel.fetchModels(record.id);
      modal.confirm({
        title: `上游模型列表(${models.length} 个)`,
        width: 560,
        content: <Typography.Paragraph copyable={{ text: models.join(',') }}>{models.join(', ')}</Typography.Paragraph>,
        okText: '写入渠道模型',
        cancelText: '关闭',
        onOk: async () => {
          await api.channel.update({
            id: record.id,
            type: record.type,
            name: record.name,
            group: record.group,
            models: models.join(','),
          });
          message.success('已写入渠道模型并重建 abilities');
          table.refresh();
        },
      });
    } catch (e) {
      message.error(e instanceof Error ? e.message : '拉取模型失败');
    }
  };

  const columns: TableColumnsType<ChannelItem> = [
    { title: 'ID', dataIndex: 'id', width: 70 },
    { title: '名称', dataIndex: 'name' },
    { title: '类型', dataIndex: 'type', width: 80 },
    {
      title: '状态',
      dataIndex: 'status',
      width: 100,
      render: (value: number) => {
        const s = STATUS[value] ?? { color: 'default', text: String(value) };
        return <Tag color={s.color}>{s.text}</Tag>;
      },
    },
    { title: '分组', dataIndex: 'group', width: 140 },
    {
      title: '模型',
      dataIndex: 'models',
      render: (value: string) => <Typography.Text ellipsis style={{ maxWidth: 260 }}>{value}</Typography.Text>,
    },
    { title: 'Key 数', dataIndex: 'key_count', width: 80 },
    { title: '优先级', dataIndex: 'priority', width: 80 },
    { title: '权重', dataIndex: 'weight', width: 80 },
    {
      title: '余额(USD)',
      dataIndex: 'balance',
      width: 120,
      render: (value: number, record) => (
        <Typography.Text>
          ${value.toFixed(2)}
          {record.balance_updated_time > 0 && (
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              {' '}
              ({dayjs.unix(record.balance_updated_time as number).format('MM-DD HH:mm')})
            </Typography.Text>
          )}
        </Typography.Text>
      ),
    },
    {
      title: '操作',
      key: 'actions',
      render: (_value, record) => (
        <Space>
          <Button size="small" onClick={() => void onToggle(record)}>
            {record.status === 1 ? '禁用' : '启用'}
          </Button>
          <Button size="small" onClick={() => void onRefreshBalance(record)}>
            刷新余额
          </Button>
          <Button size="small" onClick={() => void onTest(record)}>
            测试
          </Button>
          <Button size="small" onClick={() => void onFetchModels(record)}>
            拉取模型
          </Button>
          <Popconfirm title="确认删除该渠道?" onConfirm={() => void onDelete(record.id)}>
            <Button size="small" danger>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <Card
      title="渠道"
      extra={
        <Space>
          <Button onClick={() => void onRefreshAll()}>刷新全部余额</Button>
          <Button type="primary" onClick={() => setModalOpen(true)}>
            创建渠道
          </Button>
        </Space>
      }
    >
      <DataTable<ChannelItem>
        columns={columns}
        dataSource={table.data}
        total={table.total}
        page={table.page}
        pageSize={table.pageSize}
        loading={table.loading}
        onPageChange={(page, pageSize) => {
          table.setPageSize(pageSize);
          table.setPage(page);
        }}
      />

      <Modal
        title="创建渠道"
        open={modalOpen}
        onCancel={() => setModalOpen(false)}
        onOk={() => form.submit()}
        confirmLoading={saving}
        destroyOnClose
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={onCreate}
          initialValues={{ type: 1, enabled: true, group: 'default', priority: 0, weight: 0 }}
        >
          <Form.Item name="name" label="名称" rules={[{ required: true, message: '请输入名称' }]}>
            <Input />
          </Form.Item>
          <Form.Item name="type" label="类型(ChannelType / ApiType)" rules={[{ required: true }]}>
            <InputNumber min={0} style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item
            name="key"
            label="密钥(多 key 换行分隔)"
            rules={[{ required: true, message: '请输入密钥' }]}
          >
            <Input.TextArea rows={3} placeholder="sk-xxx" />
          </Form.Item>
          <Form.Item
            name="models"
            label="模型(逗号分隔)"
            rules={[{ required: true, message: '请输入模型' }]}
          >
            <Input placeholder="gpt-4o,gpt-4o-mini" />
          </Form.Item>
          <Form.Item name="group" label="分组(逗号分隔)" rules={[{ required: true }]}>
            <Input placeholder="default,vip" />
          </Form.Item>
          <Form.Item name="base_url" label="Base URL">
            <Input placeholder="https://api.openai.com" />
          </Form.Item>
          <Space size="large">
            <Form.Item name="priority" label="优先级">
              <InputNumber />
            </Form.Item>
            <Form.Item name="weight" label="权重">
              <InputNumber />
            </Form.Item>
            <Form.Item name="enabled" label="启用" valuePropName="checked">
              <Switch />
            </Form.Item>
          </Space>
        </Form>
      </Modal>
    </Card>
  );
}
