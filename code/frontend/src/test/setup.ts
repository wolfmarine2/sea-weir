/**
 * Vitest 全局设置。
 *
 * 契约测试策略:用 msw 拦截 `/api/*`,以**从 new-api 录制的真实响应**作为 fixture。
 * 这样前端类型定义与后端契约的偏差会在测试阶段暴露,而不是联调时。
 */
import '@testing-library/jest-dom/vitest';

// TODO(TDD): msw server setup/reset/close
