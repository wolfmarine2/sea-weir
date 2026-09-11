/**
 * i18n 初始化。
 *
 * **7 种语言全量沿用** new-api 的资源(不裁剪):
 * zh-CN / zh-TW / en / ja / fr / ru / vi。
 * antd 官方 locale 缺 vi/ru 时以 en 兜底,但业务文案仍用对应语言。
 */
import i18n from 'i18next';

export const SUPPORTED_LOCALES = ['zh-CN', 'zh-TW', 'en', 'ja', 'fr', 'ru', 'vi'] as const;
export type Locale = (typeof SUPPORTED_LOCALES)[number];

// TODO(TDD): i18next 初始化 + LanguageDetector + 资源加载
export default i18n;

// TDD 要点:
// - [ ] 7 个语言包的 key 集合完全一致(缺 key 会导致线上显示原始 key)
// - [ ] 未知语言回落 zh-CN
