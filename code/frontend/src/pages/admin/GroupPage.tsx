/**
 * 管理端-分组。两个页签:
 * - 计费分组:存放在选项 `GroupRatio`(倍率)与 `UserUsableGroups`(用户可选与描述),
 *   删除受保护(default 与仍被渠道使用的分组不可删);
 * - 预填分组:表 `prefill_groups`,模型广场按 model / tag / endpoint 预置条目,软删除。
 */
import { useCallback, useEffect, useState } from 'react';
import {
  App as AntApp,
  Button,
  Card,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Select,
  Space,
  Switch,
  Table,
  Tabs,
  Tag,
  Tooltip,
  Typography,
} from 'antd';
import type { TableColumnsType } from 'antd';
import dayjs from 'dayjs';

import { api } from '@/api';
import type { GroupItem, GroupPayload, PrefillGroupItem, PrefillGroupPayload } from '@/types';

export default function GroupPage() {
  return (
    <Card title="分组">
      <Tabs
        items={[
          { key: 'billing', label: '计费分组', children: <BillingGroups /> },
          { key: 'prefill', label: '预填分组', children: <PrefillGroups /> },
        ]}
      />
    </Card>
  );
}

// ───────────────────────── 计费分组 ─────────────────────────

interface GroupForm {
  name: string;
  ratio: number;
  description?: string | undefined;
  usable: boolean;
}

function BillingGroups() {
  const { message } = AntApp.useApp();
  const [form] = Form.useForm<GroupForm>();
  const [items, setItems] = useState<GroupItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<GroupItem | null>(null);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const { items: rows } = await api.group.list();
      setItems(rows);
    } catch (e) {
      message.error(e instanceof Error ? e.message : '加载分组失败');
    } finally {
      setLoading(false);
    }
  }, [message]);

  useEffect(() => {
    void load();
  }, [load]);

  const openCreate = () => {
    setEditing(null);
    form.resetFields();
    form.setFieldsValue({ ratio: 1, usable: true });
    setModalOpen(true);
  };

  const openEdit = (record: GroupItem) => {
    setEditing(record);
    form.setFieldsValue({
      name: record.name,
      ratio: record.ratio,
      description: record.description || undefined,
      usable: record.usable,
    });
    setModalOpen(true);
  };

  const onSubmit = async (values: GroupForm) => {
    setSaving(true);
    try {
      const payload: GroupPayload = {
        name: values.name.trim(),
        ratio: values.ratio,
        description: values.description ?? '',
        usable: values.usable,
      };
      if (editing) {
        await api.group.update(payload);
        message.success('分组已更新');
      } else {
        await api.group.create(payload);
        message.success('分组已创建');
      }
      setModalOpen(false);
      form.resetFields();
      await load();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '保存失败');
    } finally {
      setSaving(false);
    }
  };

  const onDelete = async (record: GroupItem) => {
    try {
      await api.group.remove(record.name);
      message.success('分组已删除');
      await load();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '删除失败');
    }
  };

  const columns: TableColumnsType<GroupItem> = [
    {
      title: '分组',
      dataIndex: 'name',
      render: (name: string) => <Typography.Text strong>{name}</Typography.Text>,
    },
    { title: '倍率', dataIndex: 'ratio', width: 100 },
    { title: '描述', dataIndex: 'description', render: (v: string) => v || '—' },
    {
      title: '用户可选',
      dataIndex: 'usable',
      width: 110,
      render: (v: boolean) => (v ? <Tag color="green">可选</Tag> : <Tag>不可选</Tag>),
    },
    {
      title: '使用中',
      dataIndex: 'in_use',
      width: 110,
      render: (v: boolean) => (v ? <Tag color="blue">渠道使用中</Tag> : <Tag>未使用</Tag>),
    },
    {
      title: '操作',
      key: 'actions',
      width: 160,
      render: (_v, record) => (
        <Space>
          <Button size="small" onClick={() => openEdit(record)}>
            编辑
          </Button>
          {record.deletable ? (
            <Popconfirm title={`确认删除分组 ${record.name}?`} onConfirm={() => void onDelete(record)}>
              <Button size="small" danger>
                删除
              </Button>
            </Popconfirm>
          ) : (
            <Tooltip
              title={record.name === 'default' ? '默认分组不可删除' : '仍被渠道使用,需先在渠道里移除'}
            >
              <Button size="small" danger disabled>
                删除
              </Button>
            </Tooltip>
          )}
        </Space>
      ),
    },
  ];

  return (
    <>
      <Space style={{ marginBottom: 12 }}>
        <Button type="primary" onClick={openCreate}>
          新增分组
        </Button>
      </Space>
      <Table<GroupItem>
        columns={columns}
        dataSource={items}
        rowKey="name"
        size="small"
        loading={loading}
        pagination={false}
      />

      <Modal
        title={editing ? `编辑分组:${editing.name}` : '新增分组'}
        open={modalOpen}
        onCancel={() => setModalOpen(false)}
        onOk={() => form.submit()}
        confirmLoading={saving}
        destroyOnClose
        width={520}
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={onSubmit}
          initialValues={{ ratio: 1, usable: true }}
        >
          <Form.Item
            name="name"
            label="分组名"
            tooltip="渠道/令牌按逗号分隔引用该名称;名称即身份,创建后不可改名"
            rules={[{ required: true, message: '请输入分组名' }]}
          >
            <Input disabled={!!editing} placeholder="vip" />
          </Form.Item>
          <Form.Item
            name="ratio"
            label="计费倍率"
            tooltip="最终扣费 = 模型倍率 × 分组倍率;大于 0"
            rules={[{ required: true, message: '请输入倍率' }]}
          >
            <InputNumber min={0.01} step={0.1} style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="description" label="描述(展示名)">
            <Input placeholder="VIP 分组" />
          </Form.Item>
          <Form.Item
            name="usable"
            label="用户可选"
            valuePropName="checked"
            tooltip="写入 UserUsableGroups:用户/令牌可选择该分组。关闭后已有用户与渠道仍按原名工作。"
          >
            <Switch />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
}

// ───────────────────────── 预填分组 ─────────────────────────

const PREFILL_TYPE_OPTIONS = [
  { value: 'model', label: 'model —— 模型名列表' },
  { value: 'tag', label: 'tag —— 标签列表' },
  { value: 'endpoint', label: 'endpoint —— 端点列表' },
];

interface PrefillForm {
  name: string;
  type: string;
  items?: string | undefined;
  description?: string | undefined;
}

/** 条目 ↔ 多行文本(每行一个)。 */
function itemsToText(items: unknown): string {
  return Array.isArray(items) ? items.map((x) => String(x)).join('\n') : '';
}

function textToItems(text: string | undefined): string[] {
  return (text ?? '')
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean);
}

function PrefillGroups() {
  const { message } = AntApp.useApp();
  const [form] = Form.useForm<PrefillForm>();
  const [items, setItems] = useState<PrefillGroupItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<PrefillGroupItem | null>(null);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const { items: rows } = await api.prefillGroup.list();
      setItems(rows);
    } catch (e) {
      message.error(e instanceof Error ? e.message : '加载预填分组失败');
    } finally {
      setLoading(false);
    }
  }, [message]);

  useEffect(() => {
    void load();
  }, [load]);

  const openCreate = () => {
    setEditing(null);
    form.resetFields();
    form.setFieldsValue({ type: 'model' });
    setModalOpen(true);
  };

  const openEdit = (record: PrefillGroupItem) => {
    setEditing(record);
    form.setFieldsValue({
      name: record.name,
      type: record.type,
      items: itemsToText(record.items),
      description: record.description || undefined,
    });
    setModalOpen(true);
  };

  const onSubmit = async (values: PrefillForm) => {
    setSaving(true);
    try {
      const payload: PrefillGroupPayload = {
        id: editing?.id,
        name: values.name.trim(),
        type: values.type,
        items: textToItems(values.items),
        description: values.description ?? '',
      };
      if (editing) {
        await api.prefillGroup.update(payload);
        message.success('预填分组已更新');
      } else {
        await api.prefillGroup.create(payload);
        message.success('预填分组已创建');
      }
      setModalOpen(false);
      form.resetFields();
      await load();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '保存失败');
    } finally {
      setSaving(false);
    }
  };

  const onDelete = async (record: PrefillGroupItem) => {
    try {
      await api.prefillGroup.remove(record.id);
      message.success('预填分组已删除');
      await load();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '删除失败');
    }
  };

  const columns: TableColumnsType<PrefillGroupItem> = [
    { title: 'ID', dataIndex: 'id', width: 70 },
    {
      title: '名称',
      dataIndex: 'name',
      render: (name: string) => <Typography.Text strong>{name}</Typography.Text>,
    },
    {
      title: '类型',
      dataIndex: 'type',
      width: 110,
      render: (t: string) => <Tag color="geekblue">{t}</Tag>,
    },
    {
      title: '条目',
      dataIndex: 'items',
      render: (value: unknown) => {
        const list = Array.isArray(value) ? value.map((x) => String(x)) : [];
        return (
          <Typography.Text ellipsis style={{ maxWidth: 320 }}>
            {list.length === 0 ? '—' : `${list.length} 项:${list.slice(0, 5).join(', ')}${list.length > 5 ? ' …' : ''}`}
          </Typography.Text>
        );
      },
    },
    { title: '描述', dataIndex: 'description', render: (v: string) => v || '—' },
    {
      title: '更新时间',
      dataIndex: 'updated_time',
      width: 150,
      render: (t: number) => (t > 0 ? dayjs.unix(t).format('YYYY-MM-DD HH:mm') : '—'),
    },
    {
      title: '操作',
      key: 'actions',
      width: 160,
      render: (_v, record) => (
        <Space>
          <Button size="small" onClick={() => openEdit(record)}>
            编辑
          </Button>
          <Popconfirm title={`确认删除预填分组 ${record.name}?`} onConfirm={() => void onDelete(record)}>
            <Button size="small" danger>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <>
      <Space style={{ marginBottom: 12 }}>
        <Button type="primary" onClick={openCreate}>
          新增预填分组
        </Button>
      </Space>
      <Table<PrefillGroupItem>
        columns={columns}
        dataSource={items}
        rowKey="id"
        size="small"
        loading={loading}
        pagination={false}
      />

      <Modal
        title={editing ? `编辑预填分组:${editing.name}` : '新增预填分组'}
        open={modalOpen}
        onCancel={() => setModalOpen(false)}
        onOk={() => form.submit()}
        confirmLoading={saving}
        destroyOnClose
        width={520}
      >
        <Form form={form} layout="vertical" onFinish={onSubmit} initialValues={{ type: 'model' }}>
          <Form.Item
            name="name"
            label="名称"
            rules={[{ required: true, message: '请输入名称' }]}
          >
            <Input placeholder="常用模型" />
          </Form.Item>
          <Form.Item
            name="type"
            label="类型"
            tooltip="决定条目语义:model=模型名;tag=渠道标签;endpoint=端点"
            rules={[{ required: true, message: '请选择类型' }]}
          >
            <Select options={PREFILL_TYPE_OPTIONS} />
          </Form.Item>
          <Form.Item name="items" label="条目(每行一个)">
            <Input.TextArea rows={6} placeholder={'gpt-4o\ndeepseek-chat'} />
          </Form.Item>
          <Form.Item name="description" label="描述">
            <Input placeholder="模型广场常用筛选" />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
}
