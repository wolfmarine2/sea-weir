/**
 * 控制台-令牌。列表 / 创建 / 查看 Key / 删除(UserAuth)。
 *
 * 契约:列表 key 一律脱敏;完整 key 仅经 `POST /api/token/:id/key` 获取,
 * 且只在创建后与手动点击时各展示一次。编辑(模型限制/IP 白名单)待补。
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
import type { TokenItem } from '@/types';

type TokenFilters = Record<string, never>;

interface CreateForm {
  name: string;
  unlimited_quota: boolean;
  remain_quota: number;
}

const STATUS: Record<number, { color: string; text: string }> = {
  1: { color: 'green', text: '启用' },
  2: { color: 'default', text: '禁用' },
  3: { color: 'orange', text: '过期' },
  4: { color: 'red', text: '额度耗尽' },
};

function fmtTime(ts: number): string {
  return ts > 0 ? dayjs.unix(ts).format('YYYY-MM-DD HH:mm') : '-';
}

export default function TokenPage() {
  const { message, modal } = AntApp.useApp();
  const [form] = Form.useForm<CreateForm>();
  const [modalOpen, setModalOpen] = useState(false);
  const [creating, setCreating] = useState(false);

  const fetcher = useCallback(
    ({ p, page_size }: { p: number; page_size: number }) => api.token.list({ p, page_size }),
    [],
  );
  const table = useTableData<TokenItem, TokenFilters>({ fetcher, initialFilters: {} });

  const showKey = useCallback(
    (key: string) => {
      modal.info({
        title: '令牌 Key(请及时复制,关闭后不再完整展示)',
        width: 560,
        content: <Typography.Text code copyable>{key}</Typography.Text>,
      });
    },
    [modal],
  );

  const onCreate = async (values: CreateForm) => {
    setCreating(true);
    try {
      const created = await api.token.create({
        name: values.name,
        unlimited_quota: values.unlimited_quota,
        remain_quota: values.remain_quota,
      });
      setModalOpen(false);
      form.resetFields();
      table.refresh();
      const { key } = await api.token.revealKey(created.id);
      showKey(key);
    } catch (e) {
      message.error(e instanceof Error ? e.message : '创建失败');
    } finally {
      setCreating(false);
    }
  };

  const onReveal = async (id: number) => {
    try {
      const { key } = await api.token.revealKey(id);
      showKey(key);
    } catch (e) {
      message.error(e instanceof Error ? e.message : '获取 Key 失败');
    }
  };

  const onDelete = async (id: number) => {
    try {
      await api.token.remove(id);
      message.success('已删除');
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '删除失败');
    }
  };

  const columns: TableColumnsType<TokenItem> = [
    { title: 'ID', dataIndex: 'id', width: 70 },
    { title: '名称', dataIndex: 'name' },
    {
      title: 'Key',
      dataIndex: 'key',
      render: (value: string) => <Typography.Text code>{value}</Typography.Text>,
    },
    {
      title: '状态',
      dataIndex: 'status',
      width: 90,
      render: (value: number) => {
        const s = STATUS[value] ?? { color: 'default', text: String(value) };
        return <Tag color={s.color}>{s.text}</Tag>;
      },
    },
    {
      title: '额度',
      dataIndex: 'remain_quota',
      render: (value: number, record) => (record.unlimited_quota ? '无限' : value),
    },
    {
      title: '过期',
      dataIndex: 'expired_time',
      render: (value: number) => (value === -1 ? '永不过期' : fmtTime(value)),
    },
    { title: '创建时间', dataIndex: 'created_time', render: (value: number) => fmtTime(value) },
    {
      title: '操作',
      key: 'actions',
      render: (_value, record) => (
        <Space>
          <Button size="small" onClick={() => void onReveal(record.id)}>
            查看 Key
          </Button>
          <Popconfirm title="确认删除该令牌?" onConfirm={() => void onDelete(record.id)}>
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
      title="令牌"
      extra={
        <Button type="primary" onClick={() => setModalOpen(true)}>
          创建令牌
        </Button>
      }
    >
      <DataTable<TokenItem>
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
        title="创建令牌"
        open={modalOpen}
        onCancel={() => setModalOpen(false)}
        onOk={() => form.submit()}
        confirmLoading={creating}
        destroyOnClose
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={onCreate}
          initialValues={{ unlimited_quota: true, remain_quota: 0 }}
        >
          <Form.Item name="name" label="名称" rules={[{ required: true, message: '请输入名称' }]}>
            <Input placeholder="例如 default" />
          </Form.Item>
          <Form.Item name="unlimited_quota" label="无限额度" valuePropName="checked">
            <Switch />
          </Form.Item>
          <Form.Item name="remain_quota" label="额度(quota)">
            <InputNumber min={0} style={{ width: '100%' }} />
          </Form.Item>
        </Form>
      </Modal>
    </Card>
  );
}
