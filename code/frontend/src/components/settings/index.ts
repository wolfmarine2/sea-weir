/**
 * 系统设置的 12 个配置组(与 new-api Setting 页的 12 个 TabPane 一一对应)。
 *
 * TODO(TDD): Operation / Model / Ratio / RateLimit / Payment / Drawing /
 *            Chats / Dashboard / Performance / ModelDeployment / CustomOAuth / Other
 *
 * 每组的保存都走 `PUT /api/option`(RootAuth),键名与 new-api 完全一致 ——
 * 键名改了会导致存量配置失效。
 */
export {};
