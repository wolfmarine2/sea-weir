# 11. 路由完整性测试用例

**版本**:v1.0 | **层**:L1 | **测试代码**:`test/src/unit_route_completeness.rs`
**基准来源**:CONTRACTS.md 附录 A(305 条端点清单)

## 定位

把**文档与实现绑在一起**的一组断言。任何一方改动而另一方没跟上,CI 就红。

漏注册一条路由的后果是"某个接口 404",而人工 review 很难发现 ——
尤其在 236 条管理面路由里漏一条。

## 用例清单

| 编号 | 名称 | 优先级 | 断言 |
|---|---|---|---|
| TC-UNI-RTE-001-POS | 总路由数 == 305 ★ | P0 | 防漏防重 |
| TC-UNI-RTE-002-POS | 分面计数逐一对表 | P0 | 236/54/11/4 |
| TC-UNI-RTE-003-POS | MJ 双前缀注册 ★ | P0 | `/mj` 与 `/:mode/mj` 集合相同 |
| TC-UNI-RTE-004-POS | 受保护路由都挂闸门 ★★ | P0 | 遍历断言,防越权 |
| TC-UNI-RTE-005-POS | 取渠道密钥双重防护 | P0 | RootAuth + 二次验证 |
| TC-UNI-RTE-006-POS | 公开端点不挂 auth(6 组) | P0 | 反向断言 |
| TC-UNI-RTE-007-POS | 11 条占位端点存在 | P1 | — |
| TC-UNI-RTE-008-POS | Playground 走 UserAuth | P0 | 不接受 sk-token |

## 关键用例详述

### TC-UNI-RTE-004-POS:受保护路由都挂了闸门 ★★

```yaml
背景: >
  最典型的越权漏洞成因:新增端点时忘了挂角色闸门。
  代码 review 时"看起来在 admin 分组里"不代表中间件真的生效 ——
  axum 的 layer 挂在 Router 上,写错嵌套层级就会失效。

方法: >
  维护一份 RouteManifest(注册时同步登记),遍历断言
  「requires_auth 为真但 auth_layer 为空」的集合必须为空。

失败输出: "列出所有未挂闸门的 method + path,便于直接定位"
```

### TC-UNI-RTE-003-POS:MJ 双前缀 ★

```yaml
背景: >
  new-api 的 registerMjRouterGroup 被调用两次,同一组 16 条路由同时注册在
  `/mj/**` 与 `/:mode/mj/**`。`:mode` 变体是给指定模式的存量客户端用的。
  漏掉会让这批客户端全部 404,而 `/mj/**` 的测试全绿 —— 很容易漏。

assert:
  - "/mj/** 有 16 条"
  - "/:mode/mj/** 有 16 条"
  - "两者剥离前缀后的路径集合完全相同"
```

### TC-UNI-RTE-006-POS:公开端点反向断言

```yaml
背景: >
  正向漏挂闸门是越权;反向多挂闸门是**功能不可用**,同样严重:
  - /api/setup 挂了 auth → 首装时无法创建 root(死锁)
  - /api/status 挂了 auth → 前端登录页拿不到配置,白屏
  - 支付 webhook 挂了 auth → 支付回调全部失败,用户付了钱不到账

覆盖:
  - GET  /api/status
  - GET  /api/setup
  - POST /api/setup
  - GET  /api/notice
  - POST /api/stripe/webhook
  - POST /api/user/epay/notify
```

## 实现前置

骨架的 `sea_weir_server::router` 尚未提供路由自省能力。本组用例要求补:

```rust
pub struct RouteManifest { /* ... */ }
pub struct RouteEntry {
    pub method: String,
    pub path: String,
    pub surface: String,          // api / relay / video / dashboard
    pub requires_auth: bool,
    pub auth_layer: Option<String>,
    pub extra_layers: Vec<String>,
    pub is_placeholder: bool,
}
pub fn route_manifest() -> &'static RouteManifest;
pub fn registered_route_count() -> usize;
```

**这是测试驱动出的设计产物** —— 注册时同步登记,顺带获得了运行期路由自省能力
(可用于 `/api/performance` 的诊断输出)。
