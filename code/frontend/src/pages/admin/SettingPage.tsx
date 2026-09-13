/**
 * 管理端-系统设置:倍率/定价配置(RootAuth,对接 `GET/PUT /api/option`)。
 *
 * 每个配置项是 JSON 文本,支持两种写法:
 *   - 对象:`{"gpt-4o": 2}`(按模型/分组分别设倍率)
 *   - 标量:`1`(对全部生效)
 * 保存后后端会失效定价缓存,中继计费立即按新倍率执行。
 *
 * 待补:12 个配置组 Tabs、非倍率类设置(系统名/公告/OAuth/支付开关等)。
 */
import { useEffect, useState } from 'react';
import { App as AntApp, Button, Card, Form, Input, Space, Typography } from 'antd';

import { api } from '@/api';

/** 定价相关选项(值均为 JSON 文本)。 */
const FIELDS: Array<{ key: string; label: string; hint: string }> = [
  { key: 'ModelRatio', label: '模型倍率', hint: '{"gpt-4o": 2}' },
  { key: 'GroupRatio', label: '分组倍率', hint: '{"default": 1, "vip": 0.8}' },
  { key: 'CompletionRatio', label: '补全倍率', hint: '{"gpt-4o": 3}' },
  { key: 'CacheRatio', label: '缓存命中倍率', hint: '{"gpt-4o": 0.1}' },
  { key: 'CacheCreationRatio', label: '缓存写入倍率', hint: '1.25' },
  { key: 'CacheCreation5mRatio', label: '缓存写入倍率(5m)', hint: '1.25' },
  { key: 'CacheCreation1hRatio', label: '缓存写入倍率(1h)', hint: '2' },
  { key: 'ImageRatio', label: '图片倍率', hint: '1' },
  { key: 'AudioRatio', label: '音频倍率', hint: '1' },
  { key: 'AudioInputPrice', label: '音频输入单价($/百万 token)', hint: '0' },
  { key: 'ModelPrice', label: '按次计费价格', hint: '{"dall-e-3": 0.04}' },
];

function isJson(text: string): boolean {
  const t = text.trim();
  if (t === '') return true;
  try {
    JSON.parse(t);
    return true;
  } catch {
    return false;
  }
}

export default function SettingPage() {
  const { message } = AntApp.useApp();
  const [form] = Form.useForm<Record<string, string>>();
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setLoading(true);
    api.option
      .getOptions()
      .then((opts) => {
        const values: Record<string, string> = {};
        for (const f of FIELDS) {
          values[f.key] = opts[f.key] ?? '';
        }
        form.setFieldsValue(values);
      })
      .catch((e: unknown) => message.error(e instanceof Error ? e.message : '加载配置失败'))
      .finally(() => setLoading(false));
  }, [form, message]);

  const onFinish = async (values: Record<string, string>) => {
    setSaving(true);
    try {
      const changed: Record<string, string> = {};
      for (const f of FIELDS) {
        const v = (values[f.key] ?? '').trim();
        if (v !== '') changed[f.key] = v;
      }
      const { updated } = await api.option.updateOptions(changed);
      message.success(`已保存 ${updated} 项,定价缓存已失效`);
    } catch (e) {
      message.error(e instanceof Error ? e.message : '保存失败');
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card title="倍率配置" loading={loading}>
      <Typography.Paragraph type="secondary">
        值为 JSON:对象形式按模型/分组分别设置,标量形式对全部生效。留空表示不修改该项。
      </Typography.Paragraph>
      <Form form={form} layout="vertical" onFinish={onFinish}>
        {FIELDS.map((f) => (
          <Form.Item
            key={f.key}
            name={f.key}
            label={`${f.label}(${f.key})`}
            rules={[
              {
                validator: (_rule, value: unknown) =>
                  isJson(typeof value === 'string' ? value : '')
                    ? Promise.resolve()
                    : Promise.reject(new Error('必须是合法 JSON')),
              },
            ]}
          >
            <Input.TextArea rows={2} placeholder={f.hint} />
          </Form.Item>
        ))}
        <Space>
          <Button type="primary" htmlType="submit" loading={saving}>
            保存
          </Button>
          <Button
            onClick={() => {
              form.resetFields();
            }}
          >
            重置
          </Button>
        </Space>
      </Form>
    </Card>
  );
}
