/**
 * 控制台-日志。本人消费日志分页,对接 `GET /api/log/self`。
 *
 * 待补:统计卡片(/api/log/stat)、按模型/时间筛选、令牌维度日志。
 */
import { useCallback } from 'react';
import { Card, Tag, Typography } from 'antd';
import type { TableColumnsType } from 'antd';
import dayjs from 'dayjs';

import { api } from '@/api';
import { DataTable } from '@/components/table';
import { useTableData } from '@/hooks';
import type { LogItem } from '@/types';

type LogFilters = Record<string, never>;

const LOG_TYPE: Record<number, { color: string; text: string }> = {
  0: { color: 'default', text: '未知' },
  1: { color: 'blue', text: '充值' },
  2: { color: 'green', text: '消费' },
  3: { color: 'purple', text: '管理' },
  4: { color: 'default', text: '系统' },
  5: { color: 'red', text: '错误' },
  6: { color: 'orange', text: '退款' },
};

export default function LogPage() {
  const fetcher = useCallback(
    ({ p, page_size }: { p: number; page_size: number }) => api.log.selfLogs({ p, page_size }),
    [],
  );
  const table = useTableData<LogItem, LogFilters>({ fetcher, initialFilters: {} });

  const columns: TableColumnsType<LogItem> = [
    {
      title: '时间',
      dataIndex: 'created_at',
      width: 150,
      render: (value: number) => dayjs.unix(value).format('YYYY-MM-DD HH:mm:ss'),
    },
    {
      title: '类型',
      dataIndex: 'type',
      width: 80,
      render: (value: number) => {
        const t = LOG_TYPE[value] ?? { color: 'default', text: String(value) };
        return <Tag color={t.color}>{t.text}</Tag>;
      },
    },
    { title: '模型', dataIndex: 'model_name' },
    { title: '令牌', dataIndex: 'token_name' },
    {
      title: 'Tokens',
      key: 'tokens',
      render: (_v, record) => `${record.prompt_tokens} / ${record.completion_tokens}`,
    },
    { title: '额度', dataIndex: 'quota', width: 90 },
    {
      title: '耗时',
      dataIndex: 'use_time',
      width: 80,
      render: (value: number) => `${value}s`,
    },
    {
      title: '流式',
      dataIndex: 'is_stream',
      width: 70,
      render: (value: boolean) => (value ? '是' : '否'),
    },
    {
      title: '渠道',
      dataIndex: 'channel_id',
      width: 70,
      render: (value: number | null) => value ?? '-',
    },
  ];

  return (
    <Card title={<Typography.Text strong>消费日志</Typography.Text>}>
      <DataTable<LogItem>
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
    </Card>
  );
}
