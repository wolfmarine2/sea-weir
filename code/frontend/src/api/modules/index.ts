/**
 * 按域拆分的 API 函数。每个模块只做「拼参数 + 调 client + 返回 data」,
 * 不含业务逻辑,便于用 msw 做契约测试。
 */
export * as user from './user';
export * as token from './token';
export * as channel from './channel';
export * as log from './log';
export * as pricing from './pricing';
export * as topup from './topup';
export * as subscription from './subscription';
export * as model from './model';
export * as option from './option';
export * as security from './security';
export * as task from './task';
export * as dashboard from './dashboard';
export * as group from './group';
export * as system from './system';
export * as performance from './performance';
export * as oauthProvider from './oauthProvider';
