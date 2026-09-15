/**
 * 管理端-渠道。列表 / 创建 / 编辑 / 启停 / 刷新余额 / 测试 / 拉取模型 / 删除(AdminAuth)。
 *
 * 契约:列表不回传渠道密钥;编辑不传 key 时后端沿用原密钥。
 * 支持渠道级 param_override(上游请求参数改写)与 header_override(自定义 header,支持 {api_key})。
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
  Tag,
  Typography,
} from 'antd';
import type { TableColumnsType } from 'antd';
import dayjs from 'dayjs';

import { api } from '@/api';
import { DataTable } from '@/components/table';
import { useTableData } from '@/hooks';
import type { ChannelItem, ChannelPayload, ChannelTypeOption } from '@/types';

type ChannelFilters = Record<string, never>;

interface ChannelForm {
  name: string;
  type: number;
  key?: string | undefined;
  models: string;
  group: string;
  base_url?: string | undefined;
  priority?: number | undefined;
  weight?: number | undefined;
  enabled: boolean;
  /** 上游协议:'' 自动(按渠道类型推断)/ openai_chat / openai_responses / anthropic。 */
  api_protocol?: string | undefined;
  param_override?: string | undefined;
  header_override?: string | undefined;
}

/** 上游协议选项(存入渠道 setting.api_protocol)。 */
const PROTOCOLS = [
  { value: '', label: '自动(按渠道类型:Anthropic 类型→Anthropic,其余→OpenAI Chat)' },
  { value: 'openai_chat', label: 'OpenAI Chat —— /v1/chat/completions' },
  { value: 'openai_responses', label: 'OpenAI Responses —— /v1/responses' },
  { value: 'anthropic', label: 'Anthropic Messages —— /v1/messages(x-api-key)' },
];

const STATUS: Record<number, { color: string; text: string }> = {
  1: { color: 'green', text: '启用' },
  2: { color: 'default', text: '手动禁用' },
  3: { color: 'orange', text: '自动禁用' },
};

const PARAM_HINT = '{"max_tokens":4096,"stream_options":null} 或 {"operations":[{"mode":"set","path":"a.b","value":1}]}';
const HEADER_HINT = '{"X-Custom":"value","Authorization":"Bearer {api_key}"}';

function isJson(text: string | undefined): boolean {
  const t = (text ?? '').trim();
  if (t === '') return true;
  try {
    JSON.parse(t);
    return true;
  } catch {
    return false;
  }
}

function parseJson(text: string | undefined): unknown {
  const t = (text ?? '').trim();
  return t === '' ? undefined : JSON.parse(t);
}

/** 读取渠道 setting.api_protocol('' 表示自动)。 */
function protocolOf(setting: unknown): string {
  if (setting && typeof setting === 'object' && !Array.isArray(setting)) {
    const v = (setting as Record<string, unknown>).api_protocol;
    if (typeof v === 'string') return v;
  }
  return '';
}

/**
 * 把上游协议并入渠道 setting,保留 setting 里的其它配置(如 balance)。
 * 显式置空时返回 `{}` 以清掉旧值(后端 update 用 setting 整体覆盖)。
 */
function withProtocol(existing: unknown, protocol: string | undefined): Record<string, unknown> {
  const base =
    existing && typeof existing === 'object' && !Array.isArray(existing)
      ? { ...(existing as Record<string, unknown>) }
      : {};
  if (protocol) {
    base.api_protocol = protocol;
  } else {
    delete base.api_protocol;
  }
  return base;
}

export default function ChannelPage() {
  const { message, modal } = AntApp.useApp();
  const [form] = Form.useForm<ChannelForm>();
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<ChannelItem | null>(null);
  const [saving, setSaving] = useState(false);
  /** 渠道类型目录(后端下发);加载失败时退回数字输入,表单仍可用。 */
  const [typeOptions, setTypeOptions] = useState<ChannelTypeOption[]>([]);

  useEffect(() => {
    api.channel
      .types()
      .then(({ items }) => setTypeOptions(items))
      .catch(() => setTypeOptions([]));
  }, []);

  const fetcher = useCallback(
    ({ p, page_size }: { p: number; page_size: number }) => api.channel.list({ p, page_size }),
    [],
  );
  const table = useTableData<ChannelItem, ChannelFilters>({ fetcher, initialFilters: {} });

  const openCreate = () => {
    setEditing(null);
    form.resetFields();
    setModalOpen(true);
  };

  const openEdit = (record: ChannelItem) => {
    setEditing(record);
    form.setFieldsValue({
      name: record.name,
      type: record.type,
      key: undefined,
      models: record.models,
      group: record.group,
      base_url: record.base_url ?? undefined,
      priority: record.priority,
      weight: record.weight,
      enabled: record.status === 1,
      api_protocol: protocolOf(record.setting),
      param_override: record.param_override
        ? JSON.stringify(record.param_override, null, 2)
        : undefined,
      header_override: record.header_override
        ? JSON.stringify(record.header_override, null, 2)
        : undefined,
    });
    setModalOpen(true);
  };

  const onSubmit = async (values: ChannelForm) => {
    setSaving(true);
    try {
      const payload: ChannelPayload = {
        type: values.type,
        name: values.name,
        models: values.models,
        group: values.group,
        status: values.enabled ? 1 : 2,
        base_url: values.base_url,
        priority: values.priority,
        weight: values.weight,
        setting: withProtocol(editing?.setting, values.api_protocol),
        param_override: parseJson(values.param_override),
        header_override: parseJson(values.header_override),
      };
      if (editing) {
        await api.channel.update({ ...payload, id: editing.id });
        message.success('渠道已更新');
      } else {
        await api.channel.create({ ...payload, key: values.key });
        message.success('渠道已创建');
      }
      setModalOpen(false);
      form.resetFields();
      table.refresh();
    } catch (e) {
      message.error(e instanceof Error ? e.message : '保存失败');
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
        content: (
          <Typography.Paragraph copyable={{ text: models.join(',') }}>
            {models.join(', ')}
          </Typography.Paragraph>
        ),
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
      render: (value: string) => (
        <Typography.Text ellipsis style={{ maxWidth: 240 }}>
          {value}
        </Typography.Text>
      ),
    },
    { title: 'Key 数', dataIndex: 'key_count', width: 80 },
    { title: '优先级', dataIndex: 'priority', width: 80 },
    { title: '权重', dataIndex: 'weight', width: 80 },
    {
      title: '余额(USD)',
      dataIndex: 'balance',
      width: 130,
      render: (value: number, record) => (
        <Typography.Text>
          ${value.toFixed(2)}
          {record.balance_updated_time > 0 && (
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              {' '}
              ({dayjs.unix(record.balance_updated_time).format('MM-DD HH:mm')})
            </Typography.Text>
          )}
        </Typography.Text>
      ),
    },
    {
      title: '操作',
      key: 'actions',
      render: (_value, record) => (
        <Space wrap>
          <Button size="small" onClick={() => openEdit(record)}>
            编辑
          </Button>
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
          <Button type="primary" onClick={openCreate}>
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
        title={editing ? `编辑渠道:${editing.name}` : '创建渠道'}
        open={modalOpen}
        onCancel={() => setModalOpen(false)}
        onOk={() => form.submit()}
        confirmLoading={saving}
        destroyOnClose
        width={640}
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={onSubmit}
          initialValues={{ type: 1, enabled: true, group: 'default', priority: 0, weight: 0 }}
        >
          <Form.Item name="name" label="名称" rules={[{ required: true, message: '请输入名称' }]}>
            <Input />
          </Form.Item>
          <Form.Item
            name="type"
            label="类型(渠道平台)"
            tooltip="渠道编号(ChannelType):标识上游是哪家平台,决定内置余额探测与「上游协议」留空时的推断。选中后会带出该平台的默认 Base URL。"
            rules={[{ required: true, message: '请选择渠道类型' }]}
          >
            {typeOptions.length > 0 ? (
              <Select
                showSearch
                optionFilterProp="label"
                placeholder="搜索平台名或编号,如 DeepSeek / 43"
                options={typeOptions.map((o) => ({
                  value: o.type,
                  label: `${o.type} — ${o.name}`,
                }))}
                onChange={(value: number) => {
                  const opt = typeOptions.find((o) => o.type === value);
                  if (opt?.default_base_url && !form.getFieldValue('base_url')) {
                    form.setFieldValue('base_url', opt.default_base_url);
                  }
                }}
              />
            ) : (
              // 目录拉取失败时退回数字输入,避免表单不可用。
              <InputNumber min={0} style={{ width: '100%' }} placeholder="如 43" />
            )}
          </Form.Item>
          <Form.Item
            name="key"
            label={editing ? '密钥(留空则沿用原密钥)' : '密钥(多 key 换行分隔)'}
            rules={editing ? [] : [{ required: true, message: '请输入密钥' }]}
          >
            <Input.TextArea rows={2} placeholder="sk-xxx" />
          </Form.Item>
          <Form.Item
            name="models"
            label="模型(逗号分隔)"
            rules={[{ required: true, message: '请输入模型' }]}
          >
            <Input placeholder="gpt-4o,deepseek-chat" />
          </Form.Item>
          <Form.Item name="group" label="分组(逗号分隔)" rules={[{ required: true }]}>
            <Input placeholder="default,vip" />
          </Form.Item>
          <Form.Item name="base_url" label="Base URL">
            <Input placeholder="https://api.deepseek.com" />
          </Form.Item>
          <Form.Item
            name="api_protocol"
            label="上游协议(对外部平台使用的 API 协议)"
            tooltip="决定 sea-weir 用哪种协议调用该渠道:OpenAI Chat、OpenAI Responses 或 Anthropic Messages。留空则按渠道类型推断。"
          >
            <Select options={PROTOCOLS} />
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
          <Form.Item
            name="param_override"
            label="请求参数改写 param_override(JSON)"
            rules={[{ validator: (_r, v: unknown) => (isJson(v as string) ? Promise.resolve() : Promise.reject(new Error('必须是合法 JSON'))) }]}
          >
            <Input.TextArea rows={3} placeholder={PARAM_HINT} />
          </Form.Item>
          <Form.Item
            name="header_override"
            label="请求头改写 header_override(JSON)"
            rules={[{ validator: (_r, v: unknown) => (isJson(v as string) ? Promise.resolve() : Promise.reject(new Error('必须是合法 JSON'))) }]}
          >
            <Input.TextArea rows={3} placeholder={HEADER_HINT} />
          </Form.Item>
        </Form>
      </Modal>
    </Card>
  );
}
