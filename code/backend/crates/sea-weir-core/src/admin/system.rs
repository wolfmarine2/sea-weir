//! 系统域:首装向导(setup)、系统状态(status)、公告/关于/协议文案
//!
//! 契约见 doc/architecture/CONTRACTS.md。

// TODO(TDD): 定义本域的 Service 结构体与方法。
//   依赖以 `Arc<dyn XxxRepository>` 注入,便于用 mock 单测。
//
// 本域测试要点:
//   未初始化时 PostSetup 创建 root;已初始化后再次调用被拒;status 返回的开关集合与前端 store 期望一致
