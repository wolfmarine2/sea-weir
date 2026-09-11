# ADR-009: 前端架构

## Metadata

| 项 | 内容 |
|----|------|
| 编号 | ADR-009 |
| 标题 | 前端架构(React 18 + TS strict + antd 5 + zustand) |
| 状态 | Proposed |
| 日期 | 2026-09-10 |
| 决策者 | Architecture Team |
| 影响范围 | `code/frontend` |
| 依赖 | ADR-002 |

## Status History

| 日期 | 状态 | 说明 |
|------|------|------|
| 2026-09-10 | Proposed | 初始提议 |

## Context(上下文)

### 问题陈述

new-api 前端为 React 18 纯 JSX + Semi Design + Context/useReducer + 单例 axios,存在:无类型检查、20+ 服务端配置平铺进 localStorage、权限守卫仅查 localStorage、组件库与公司栈(antd)不一致等问题。重写时需确定语言模式、组件库、状态管理与数据获取方案。

### 约束条件

- 页面与交互行为与 new-api 对齐(用户无感迁移)
- 公司 web 栈先例(web-auth):React 18 + TS + antd 5 + zustand + axios + vite
- 页面量大(24 个页面域、11 个表格域、12 个设置组)

### 驱动力

- 类型安全:TS strict 零错误门禁
- 公司栈统一:组件/主题/权限模式复用
- 状态卫生:收敛 localStorage;权限以服务端为准
- 迁移效率:表格三件套等模式沿用并类型化

### 目标状态

TS strict;localStorage 仅存用户偏好与登录态;路由守卫为体验层;按域表格模式复用。

### 备选方案

#### 方案 A:React 18 + TS strict + antd 5 + zustand + axios + vite

**描述**: 全面 TS 化,组件库换 antd 5,状态用 zustand(status/user/theme 三 store),API 层按域模块化。
**优点**:
- 公司栈统一;类型门禁;状态收敛
- axios 层保留 GET 去重等优化,按域拆分函数
**缺点**:
- Semi → antd 组件映射需逐页适配(表格/表单差异)

#### 方案 B:保留 Semi Design,仅 TS 化

**描述**: 不换组件库。
**优点**:
- 页面迁移成本最低(组件 API 不变)
**缺点**:
- 与公司栈分裂;Semi 定制机制(cssLayer/变量映射)长期维护成本高

#### 方案 C:引入 React Query 全面重构数据层

**描述**: 以 React Query 替代手写 hooks。
**优点**:
- 缓存/失效/重试标准化
**缺点**:
- 与现有「hooks + axios」模式差异大,迁移工作量显著增加;收益可在后续迭代获得

### 决策依据

| 维度 | 权重 | A | B | C |
|---|---|---|---|---|
| 公司栈统一 | 高 | ✔ | ✘ | ✔ |
| 迁移成本 | 高 | △ | ✔ | ✘ |
| 状态卫生 | 中 | ✔ | △ | ✔ |
| 长期演进 | 中 | ✔ | △ | ✔ |

## Decision(决策)

### 选择的方案

在 sea-weir 前端重构中,面对「类型安全 + 公司栈统一 + 状态收敛」的关注点,我们选择 **React 18 + TS strict + antd 5 + zustand + axios + vite(数据层保留 hooks 模式)**,而非保留 Semi 或全面引入 React Query,以获得栈统一与可控迁移成本,接受 Semi→antd 的逐页适配工作。

### 决策理由

1. antd 5 与公司 web 栈统一,主题/权限/表格模式可复用
2. zustand 三 store(user/status/theme)替代 Context 嵌套;仅用户偏好持久化,服务端配置不再平铺 localStorage
3. 权限以服务端逐端点鉴权为准,前端守卫(角色/模块可见性)仅作体验层
4. 数据层保留 hooks 模式控制迁移范围,React Query 列入演进

### 技术架构

- 状态:`stores/user.ts`(登录态、角色;persist)、`stores/status.ts`(`/api/status` 全站配置;不持久化)、`stores/theme.ts`(亮/暗;persist)
- API 层:`api/client.ts`(New-Api-User 头、success=false 统一报错、GET 去重、401 清态跳登录)+ `api/modules/*.ts`(按域函数)
- 路由:静态注册 + 守卫组件(Private/Admin/Root);模块可见性由 status store 驱动
- 页面:覆盖 new-api 全部 24 个页面目录,重组为 28 个页面 —— console ×8(含 Playground、PersonalSetting)/ admin ×7 / auth ×3 / 运营文案 ×4(Home、About、UserAgreement、PrivacyPolicy)/ 外部聊天入口 ×2(Chat、Chat2Link)/ 兜底页 ×2(NotFound、Forbidden)/ pricing ×1 / setup ×1;表格域沿用「Table + ColumnDefs + Filters + Actions + useXxxData」五件套并类型化
- i18n:i18next 资源全量沿用 **7 种语言**(zh-CN / zh-TW / en / ja / fr / ru / vi),antd ConfigProvider 同步 locale;antd 官方无 vi/ru 缺项时以 en 兜底
- 特色:Playground SSE 直连 `/pg/chat/completions`;安全验证 Modal(对接 `/api/verify`);主题暗色 antd algorithm

### 实施计划

1. 脚手架 + api client + 三 store + 布局(顶栏/侧边/主题)
2. 表格五件套基础设施 + 令牌/日志两个样板页
3. 按域迁移其余页面;Playground 与 Pricing 最后(交互最重)
4. 运营文案页与兜底页(改动小、可并行);视觉走查与 7 种语言 i18n 资源补齐;localStorage 收敛审计

## Consequences(后果)

### 正面影响

1. 类型门禁消除整类前端运行时错误
2. localStorage 从 20+ 键收敛到 ≤5(用户偏好/登录态)
3. 权限语义与后端对齐(前端不再承担安全职责)

### 负面影响

1. Semi→antd 视觉与交互细节差异需走查
2. 图表库 VChart → @ant-design/charts 需重配主题

### 缓解措施

1. 页面级截图比对走查清单;图表封装一层适配组件隔离差异

### 长期影响

数据层可平滑演进 React Query;组件资产(表格五件套)可反哺公司其他管理台项目。

## Notes

- `New-Api-User` 头由 api client 从 user store 注入(契约兼容)
- 聊天链接模板等少量跨会话配置仍 persist(白名单制)

## References

- `doc/system-design.md` §3.3、§4.2、§5.3
- 相关 ADR:ADR-002、ADR-004

---

**文档版本**: v1.1
**最后更新**: 2026-09-10
**下次评审**: 2026-12-10(3个月后)
**评审责任人**: Architecture Team Lead
