/**
 * 应用入口。
 *
 * 装配顺序:i18n 初始化 → antd ConfigProvider(locale + 主题 algorithm)→ Router → App。
 */
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './i18n';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
