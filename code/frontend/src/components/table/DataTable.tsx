/**
 * 配置驱动的域表格。对应 C4 组件 `table_kit`。
 *
 * 「五件套」之一(ADR-009):Table + ColumnDefs + Filters + Actions + useXxxData。
 * 11 个域复用同一套骨架,只提供各自的列定义、筛选项与行操作。
 *
 * 分页契约:页码 **1 起**,请求参数 `p` + `page_size`,响应 `{items,total,page,page_size}`。
 */
import { Table } from 'antd';
import type { TableColumnsType } from 'antd';

export interface DataTableProps<T> {
  columns: TableColumnsType<T>;
  dataSource: T[];
  total: number;
  page: number;
  pageSize: number;
  loading?: boolean;
  /** 默认按 `id` 作为行键。 */
  rowKey?: string;
  onPageChange: (page: number, pageSize: number) => void;
}

export function DataTable<T extends object>({
  columns,
  dataSource,
  total,
  page,
  pageSize,
  loading,
  rowKey = 'id',
  onPageChange,
}: DataTableProps<T>) {
  return (
    <Table<T>
      columns={columns}
      dataSource={dataSource}
      rowKey={rowKey}
      loading={loading ?? false}
      size="small"
      scroll={{ x: 'max-content' }}
      pagination={{
        current: page,
        pageSize,
        total,
        showSizeChanger: true,
        showTotal: (count) => `共 ${count} 条`,
        onChange: onPageChange,
      }}
    />
  );
}

// TDD 要点(这是 11 个域共用的基础设施,测试收益最高):
// - [x] 首次渲染请求 page=1(不是 0 —— 契约是 1 起)由 useTableData 保证
// - [x] 翻页触发 onPageChange 且参数正确(antd pagination.onChange 直传)
// - [ ] loading 时展示骨架屏而非空白(antd Table loading)
// - [x] total=0 时展示空态(antd Table 默认空态)
