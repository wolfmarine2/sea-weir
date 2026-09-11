/**
 * 域表格数据 hook。「五件套」之一,11 个表格域复用。
 *
 * 职责:分页状态 + 筛选状态 + 请求编排 + loading/error。
 * 分页契约:页码 **1 起**,请求参数 `p` + `page_size`。
 */
import { useCallback, useEffect, useRef, useState } from 'react';

export interface UseTableDataOptions<T, F> {
  fetcher: (params: { p: number; page_size: number } & F) => Promise<{
    items: T[];
    total: number;
  }>;
  initialFilters: F;
  initialPageSize?: number;
}

export interface TableDataResult<T, F> {
  data: T[];
  total: number;
  page: number;
  pageSize: number;
  loading: boolean;
  error: unknown;
  filters: F;
  setFilters: (filters: F) => void;
  setPage: (page: number) => void;
  setPageSize: (pageSize: number) => void;
  refresh: () => void;
}

/**
 * 表格数据编排。
 *
 * 关键行为:
 * - 初始页码 **1**(不是 0)
 * - `setFilters` 会把页码**重置回 1**(否则筛选后可能停在空页)
 * - 竞态:只有「最新一次请求」的结果被采纳,先发后至的响应被丢弃
 * - 失败时保留上一次数据,只更新 error
 */
export function useTableData<T, F extends object>(
  opts: UseTableDataOptions<T, F>,
): TableDataResult<T, F> {
  const { initialFilters, initialPageSize = 10 } = opts;

  const [data, setData] = useState<T[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPageState] = useState(1);
  const [pageSize, setPageSizeState] = useState(initialPageSize);
  const [filters, setFiltersState] = useState<F>(initialFilters);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<unknown>(null);

  // 竞态控制:请求序号,只有最新的请求结果生效。
  const requestSeq = useRef(0);
  // 始终引用最新的 fetcher,避免调用方内联函数导致的重复请求。
  const fetcherRef = useRef(opts.fetcher);
  fetcherRef.current = opts.fetcher;

  const load = useCallback(async (p: number, size: number, f: F) => {
    const seq = ++requestSeq.current;
    setLoading(true);
    try {
      const result = await fetcherRef.current({ p, page_size: size, ...f });
      if (seq !== requestSeq.current) {
        return; // 已有更新的请求,丢弃本次结果。
      }
      setData(result.items);
      setTotal(result.total);
      setError(null);
    } catch (err) {
      if (seq !== requestSeq.current) {
        return;
      }
      // 保留上一次数据,仅记录错误。
      setError(err);
    } finally {
      if (seq === requestSeq.current) {
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    void load(page, pageSize, filters);
  }, [load, page, pageSize, filters]);

  const setFilters = useCallback((next: F) => {
    setFiltersState(next);
    // 筛选条件变化必须回到第 1 页(TC-UNI-FE-011)。
    setPageState(1);
  }, []);

  const setPage = useCallback((next: number) => {
    setPageState(Math.max(1, next)); // 页码下限为 1。
  }, []);

  const setPageSize = useCallback((next: number) => {
    setPageSizeState(Math.max(1, next));
    setPageState(1);
  }, []);

  const refresh = useCallback(() => {
    void load(page, pageSize, filters);
  }, [load, page, pageSize, filters]);

  return {
    data,
    total,
    page,
    pageSize,
    loading,
    error,
    filters,
    setFilters,
    setPage,
    setPageSize,
    refresh,
  };
}

// TDD 要点:
// - [x] 初始请求 p=1
// - [x] 修改筛选条件后**重置回第 1 页**
// - [x] 并发请求竞态:后发起的请求先返回时,不被先发起的覆盖
// - [x] 请求失败时保留上一次数据并展示错误,而非清空表格
