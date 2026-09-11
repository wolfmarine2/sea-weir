/**
 * 配置驱动的域表格。对应 C4 组件 `table_kit`。
 *
 * 「五件套」之一(ADR-009):Table + ColumnDefs + Filters + Actions + useXxxData。
 * 11 个域复用同一套骨架,只提供各自的列定义、筛选项与行操作。
 *
 * 分页契约:页码 **1 起**,请求参数 `p` + `page_size`,响应 `{items,total,page,page_size}`。
 */

export interface DataTableProps<T> {
  columns: unknown[];
  dataSource: T[];
  total: number;
  page: number;
  pageSize: number;
  loading?: boolean;
  onPageChange: (page: number, pageSize: number) => void;
}

export function DataTable<T>(_props: DataTableProps<T>) {
  // TODO(TDD): antd Table 封装 + 分页联动 + 空态/加载态
  return null;
}

// TDD 要点(这是 11 个域共用的基础设施,测试收益最高):
// - [ ] 首次渲染请求 page=1(不是 0 —— 契约是 1 起)
// - [ ] 翻页触发 onPageChange 且参数正确
// - [ ] loading 时展示骨架屏而非空白
// - [ ] total=0 时展示空态
