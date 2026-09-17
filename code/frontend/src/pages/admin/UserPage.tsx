/**
 * 管理端-用户。用户访问控制:列表 / 创建 / 改角色·状态·分组 / 重置密码 / 删除。
 *
 * 说明:内部使用不涉及充值,本页不含额度调整。
 * 权限:AdminAuth;root 账号仅 root 可操作(服务端 guard_root 兜底)。
 */
import { useCallback, useEffect, useState } from 'react';
import {
  App as AntApp,
  AutoComplete,
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
import type { CreateUserPayload } from '@/api/modules/user';
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
  const [batchOpen, setBatchOpen] = useState(false);
  const [batchText, setBatchText] = useState('');
  const [editOpen, setEditOpen] = useState(false);
  const [editing, setEditing] = useState<User | null>(null);
  const [saving, setSaving] = useState(false);
  /** 「分组」页维护的分组名,供「分组」字段下拉选择(仍允许直接输入未登记的名称)。 */
  const [groupOptions, setGroupOptions] = useState<{ value: string }[]>([]);

  useEffect(() => {
    api.group
      .list()
      .then(({ items }) => setGroupOptions(items.map((g) => ({ value: g.name }))))
      .catch(() => setGroupOptions([]));
  }, []);

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

  /** 解析批量导入文本:每行 `用户名,口令[,角色[,显示名[,分组]]]`。 */
  const parseBatch = (text: string): CreateUserPayload[] => {
    const users: CreateUserPayload[] = [];
    for (const raw of text.split('\n')) {
      const line = raw.trim();
      if (line === '' || line.startsWith('#')) continue;
      const [username, password, role, display_name, group] = line
        .split(',')
        .map((s) => s.trim());
      if (!username || !password) continue;
      users.push({
        username,
        password,
        role: role ? Number(role) : undefined,
        display_name: display_name || undefined,
        group: group || undefined,
      });
    }
    return users;
  };

  const onBatchImport = async () => {
    const users = parseBatch(batchText);
    if (users.length === 0) {
      message.warning('没有解析到有效行(格式:用户名,口令[,角色[,显示名[,分组]]])');
      return;
    }
    setSaving(true);
    try {
      const r = await api.user.batchCreateUsers(users);
      if (r.failed === 0) {
        message.success(`已创建 ${r.created} 个用户`);
      } else {
        const detail = r.results
          .filter((x) => !x.ok)
          .map((x) => `${x.username}:${x.error ?? '失败'}`)
          .join(';');
        message.warning(`成功 ${r.created},失败 ${r.failed} —— ${detail}`);
      }
      setBatchOpen(false);
      setBatchText('');
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '批量导入失败');
    } finally {
      setSaving(false);
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
        <Space>
          <Button onClick={() => setBatchOpen(true)}>批量导入</Button>
          <Button type="primary" onClick={() => setCreateOpen(true)}>
            创建用户
          </Button>
        </Space>
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
          <Form.Item
            name="group"
            label="分组"
            tooltip="下拉选择「分组」页维护的分组;也可直接输入尚未登记的名称"
          >
            <AutoComplete
              options={groupOptions}
              placeholder="default"
              allowClear
              filterOption={(input, option) =>
                String(option?.value ?? '').toLowerCase().includes(input.toLowerCase())
              }
            />
          </Form.Item>
        </Form>
      </Modal>

      <Modal
        title="批量导入用户"
        open={batchOpen}
        onCancel={() => setBatchOpen(false)}
        onOk={() => void onBatchImport()}
        confirmLoading={saving}
        width={640}
        destroyOnClose
      >
        <Typography.Paragraph type="secondary">
          每行一个用户,格式:<Typography.Text code>用户名,口令[,角色[,显示名[,分组]]]</Typography.Text>
          ;以 # 开头的行忽略。角色取 0/1/10/100,缺省 1(common)。单次最多 200 条。
        </Typography.Paragraph>
        <Input.TextArea
          rows={10}
          value={batchText}
          onChange={(e) => setBatchText(e.target.value)}
          placeholder={'alice,alice123\nbob,bob123,10,运维,vip'}
        />
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
          <Form.Item
            name="group"
            label="分组"
            tooltip="下拉选择「分组」页维护的分组;也可直接输入尚未登记的名称"
          >
            <AutoComplete
              options={groupOptions}
              allowClear
              filterOption={(input, option) =>
                String(option?.value ?? '').toLowerCase().includes(input.toLowerCase())
              }
            />
          </Form.Item>
          <Form.Item name="password" label="重置口令(留空则不改)">
            <Input.Password autoComplete="new-password" />
          </Form.Item>
        </Form>
      </Modal>
    </Card>
  );
}
