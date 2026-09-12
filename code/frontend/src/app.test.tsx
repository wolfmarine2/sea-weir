/**
 * 应用外壳冒烟测试:守卫「页面白屏」回归。
 *
 * 背景:路由表为空时 createBrowserRouter([]) 会让整个应用渲染为空。
 * 这里断言 App 能挂载出布局与首页内容 —— 只要路由/布局被误删就会红。
 */
import { render, screen } from '@testing-library/react';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import App from './App';

beforeAll(() => {
  // antd 依赖 matchMedia,jsdom 未实现。
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
});

describe('app shell', () => {
  it('renders layout + home instead of a blank page', async () => {
    render(<App />);

    // 顶栏系统名(布局已挂载)
    expect(await screen.findByRole('heading', { name: 'sea-weir' })).toBeInTheDocument();
    // 首页内容(路由命中 index 路由)
    expect(await screen.findByText('骨架阶段')).toBeInTheDocument();
  });
});
