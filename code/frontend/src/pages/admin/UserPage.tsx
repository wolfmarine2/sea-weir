/**
 * 管理端-用户。用户访问控制:列表 / 创建 / 改角色·状态·分组 / 重置密码 / 删除。
 *
 * 说明:内部使用不涉及充值,本页不含额度调整。
 * 权限:AdminAuth;root 账号仅 root 可操作(服务端 guard_root 兜底)。
 */
import { useCallback, useState } from 'react';
import {
  App as AntApp,
  Button,
  Card,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Tag,
  Typography,
} from 'antd';
import type { TableColumnsType } from 'antd';
import dayjs from 'dayjs';

import { api } from '@/api';
import { DataTable } from '@/components/table';
import { useTableData } from '@/hooks';
import type { User } from '@/types';

type UserFilters = Record<string, never>;

const ROLE_OPTIONS = [
  { value: 0, label: 'guest(0)' },
  { value: 1, label: 'common(1)' },
  { value: 10, label: 'admin(10)' },
  { value: 100, label: 'root(100)' },
];

const ROLE_TAG: Record<number, { color: string; text: string }> = {
  0: { color: 'default', text: 'guest' },
  1: { color: 'blue', text: 'common' },
  10: { color: 'purple', text: 'admin' },
  100: { color: 'red', text: 'root' },
};

interface CreateForm {
  username: string;
  password: string;
  role: number;
  display_name?: string;
  group?: string;
}

interface EditForm {
  role: number;
  status: number;
  display_name?: string;
  group?: string;
  password?: string;
}

export default function UserPage() {
  const { message } = AntApp.useApp();
  const [createForm] = Form.useForm<CreateForm>();
  const [editForm] = Form.useForm<EditForm>();
  const [createOpen, setCreateOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [editing, setEditing] = useState<User | null>(null);
  const [saving, setSaving] = useState(false);

  const fetcher = useCallback(
    ({ p, page_size }: { p: number; page_size: number }) => api.user.listUsers({ p, page_size }),
    [],
  );
  const table = useTableData<User, UserFilters>({ fetcher, initialFilters: {} });

  const onCreate = async (values: CreateForm) => {
    setSaving(true);
    try {
      await api.user.createUser(values);
      setCreateOpen(false);
      createForm.resetFields();
      message.success('用户已创建');
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '创建失败');
    } finally {
      setSaving(false);
    }
  };

  const openEdit = (record: User) => {
    setEditing(record);
    editForm.setFieldsValue({
      role: record.role,
      status: record.status,
      display_name: record.display_name,
      group: record.group,
    });
    setEditOpen(true);
  };

  const onEdit = async (values: EditForm) => {
    if (editing === null) return;
    setSaving(true);
    try {
      await api.user.updateUser({ id: editing.id, ...values, password: values.password || undefined });
      setEditOpen(false);
      message.success('已保存');
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '保存失败');
    } finally {
      setSaving(false);
    }
  };

  const onDelete = async (id: number) => {
    try {
      await api.user.deleteUser(id);
      message.success('已删除');
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '删除失败');
    }
  };

  const columns: TableColumnsType<User> = [
    { title: 'ID', dataIndex: 'id', width: 70 },
    { title: '用户名', dataIndex: 'username' },
    { title: '显示名', dataIndex: 'display_name' },
    {
      title: '角色',
      dataIndex: 'role',
      width: 100,
      render: (value: number) => {
        const t = ROLE_TAG[value] ?? { color: 'default', text: String(value) };
        return <Tag color={t.color}>{t.text}</Tag>;
      },
    },
    {
      title: '状态',
      dataIndex: 'status',
      width: 90,
      render: (value: number) =>
        value === 1 ? <Tag color="green">启用</Tag> : <Tag color="orange">禁用</Tag>,
    },
    { title: '分组', dataIndex: 'group', width: 120 },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      width: 160,
      render: (value: number) => (value > 0 ? dayjs.unix(value).format('YYYY-MM-DD HH:mm') : '-'),
    },
    {
      title: '操作',
      key: 'actions',
      render: (_value, record) => (
        <Space>
          <Button size="small" onClick={() => openEdit(record)}>
            编辑
          </Button>
          <Popconfirm title="确认删除该用户?" onConfirm={() => void onDelete(record.id)}>
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
      title={<Typography.Text strong>用户</Typography.Text>}
      extra={
        <Button type="primary" onClick={() => setCreateOpen(true)}>
          创建用户
        </Button>
      }
    >
      <DataTable<User>
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
        title="创建用户"
        open={createOpen}
        onCancel={() => setCreateOpen(false)}
        onOk={() => createForm.submit()}
        confirmLoading={saving}
        destroyOnClose
      >
        <Form form={createForm} layout="vertical" onFinish={onCreate} initialValues={{ role: 1 }}>
          <Form.Item name="username" label="用户名" rules={[{ required: true, message: '请输入用户名' }]}>
            <Input autoComplete="off" />
          </Form.Item>
          <Form.Item
            name="password"
            label="口令"
            rules={[{ required: true, min: 6, message: '口令至少 6 位' }]}
          >
            <Input.Password autoComplete="new-password" />
          </Form.Item>
          <Form.Item name="role" label="角色">
            <Select options={ROLE_OPTIONS} />
          </Form.Item>
          <Form.Item name="display_name" label="显示名">
            <Input />
          </Form.Item>
          <Form.Item name="group" label="分组">
            <Input placeholder="default" />
          </Form.Item>
        </Form>
      </Modal>

      <Modal
        title={`编辑用户:${editing?.username ?? ''}`}
        open={editOpen}
        onCancel={() => setEditOpen(false)}
        onOk={() => editForm.submit()}
        confirmLoading={saving}
        destroyOnClose
      >
        <Form form={editForm} layout="vertical" onFinish={onEdit}>
          <Form.Item name="role" label="角色">
            <Select options={ROLE_OPTIONS} />
          </Form.Item>
          <Form.Item name="status" label="状态">
            <Select
              options={[
                { value: 1, label: '启用' },
                { value: 2, label: '禁用' },
              ]}
            />
          </Form.Item>
          <Form.Item name="display_name" label="显示名">
            <Input />
          </Form.Item>
          <Form.Item name="group" label="分组">
            <Input />
          </Form.Item>
          <Form.Item name="password" label="重置口令(留空则不改)">
            <Input.Password autoComplete="new-password" />
          </Form.Item>
        </Form>
      </Modal>
    </Card>
  );
}
