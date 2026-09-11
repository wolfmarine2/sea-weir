import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'node:path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { '@': path.resolve(__dirname, './src') },
  },
  server: {
    port: 5173,
    // 本地开发时把两个面都代理到后端,与 nginx 的生产反代路径保持一致
    proxy: Object.fromEntries(
      ['/api', '/v1', '/v1beta', '/mj', '/suno', '/kling', '/jimeng', '/pg', '/dashboard'].map(
        (p) => [p, { target: 'http://localhost:8080', changeOrigin: true }],
      ),
    ),
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
  },
});
