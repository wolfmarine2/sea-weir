/**
 * 全站类型定义。
 *
 * **命名铁律**:所有与后端交互的字段一律 `snake_case`,
 * 与 new-api 契约逐字段一致(doc/architecture/CONTRACTS.md §通用约定)。
 * 前端内部状态可用 camelCase,但**跨越 API 边界的类型不可以**。
 */
export * from './api';
export * from './domain';
