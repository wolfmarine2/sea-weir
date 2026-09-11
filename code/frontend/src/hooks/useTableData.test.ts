/**
 * L1 — 表格地基。用例:`test/cases/14-frontend.md`(TC-UNI-FE-010~013)。
 */
import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { useTableData } from './useTableData';

interface Item {
  id: number;
}

interface Row {
  items: Item[];
  total: number;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

interface Call {
  params: { p: number; page_size: number } & Record<string, unknown>;
  d: ReturnType<typeof deferred<Row>>;
}

function setupFetcher() {
  const calls: Call[] = [];
  const fetcher = (params: { p: number; page_size: number } & Record<string, unknown>) => {
    const d = deferred<Row>();
    calls.push({ params, d });
    return d.promise;
  };
  return { calls, fetcher };
}

describe('useTableData', () => {
  it('TC-UNI-FE-010-POS:初始请求 p=1 ★', async () => {
    const { calls, fetcher } = setupFetcher();
    renderHook(() => useTableData<Item, Record<string, never>>({ fetcher, initialFilters: {} }));

    await waitFor(() => expect(calls).toHaveLength(1));
    expect(calls[0]!.params.p).toBe(1);
    expect(calls[0]!.params.page_size).toBe(10);
  });

  it('TC-UNI-FE-011-POS:修改筛选后重置回第 1 页 ★', async () => {
    const { calls, fetcher } = setupFetcher();
    const { result } = renderHook(() =>
      useTableData<Item, { keyword?: string }>({ fetcher, initialFilters: {} }),
    );
    await waitFor(() => expect(calls).toHaveLength(1));

    // 翻到第 5 页
    act(() => result.current.setPage(5));
    await waitFor(() => expect(calls).toHaveLength(2));
    expect(calls[1]!.params.p).toBe(5);

    // 修改筛选条件 → 必须回到第 1 页
    act(() => result.current.setFilters({ keyword: 'gpt' }));
    await waitFor(() => expect(calls).toHaveLength(3));
    expect(calls[2]!.params.p).toBe(1);
  });

  it('TC-UNI-FE-012-POS:并发请求竞态 —— 后发起的请求结果生效', async () => {
    const { calls, fetcher } = setupFetcher();
    const { result } = renderHook(() =>
      useTableData<Item, Record<string, never>>({ fetcher, initialFilters: {} }),
    );
    await waitFor(() => expect(calls).toHaveLength(1));

    // 第二次请求(第 2 页)
    act(() => result.current.setPage(2));
    await waitFor(() => expect(calls).toHaveLength(2));

    // 后发起的先返回
    await act(async () => {
      calls[1]!.d.resolve({ items: [{ id: 2 }], total: 2 });
    });
    await waitFor(() => expect(result.current.data).toEqual([{ id: 2 }]));

    // 先发起的后返回 —— 必须被丢弃,不能覆盖
    await act(async () => {
      calls[0]!.d.resolve({ items: [{ id: 1 }], total: 1 });
    });
    expect(result.current.data).toEqual([{ id: 2 }]);
    expect(result.current.total).toBe(2);
  });

  it('TC-UNI-FE-013-POS:失败时保留上次数据', async () => {
    const { calls, fetcher } = setupFetcher();
    const { result } = renderHook(() =>
      useTableData<Item, Record<string, never>>({ fetcher, initialFilters: {} }),
    );
    await waitFor(() => expect(calls).toHaveLength(1));

    await act(async () => {
      calls[0]!.d.resolve({ items: [{ id: 1 }], total: 1 });
    });
    await waitFor(() => expect(result.current.data).toEqual([{ id: 1 }]));

    // 刷新时失败
    act(() => result.current.refresh());
    await waitFor(() => expect(calls).toHaveLength(2));
    await act(async () => {
      calls[1]!.d.reject(new Error('boom'));
    });

    await waitFor(() => expect(result.current.error).toBeInstanceOf(Error));
    expect(result.current.data, '失败不得清空表格').toEqual([{ id: 1 }]);
  });

  it('页码下限为 1(setPage(0) 归一)', async () => {
    const { calls, fetcher } = setupFetcher();
    const { result } = renderHook(() =>
      useTableData<Item, Record<string, never>>({ fetcher, initialFilters: {} }),
    );
    await waitFor(() => expect(calls).toHaveLength(1));

    act(() => result.current.setPage(0));
    await waitFor(() => expect(result.current.page).toBe(1));
  });
});
