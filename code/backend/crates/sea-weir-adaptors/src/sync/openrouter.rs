//! `openrouter` 渠道适配器。

pub struct OpenrouterAdaptor;

// TODO(TDD): impl crate::Adaptor for OpenrouterAdaptor
//   1. 先写 convert_request 的黄金用例(固定输入 → 固定上游请求体)
//   2. 再用 wiremock 覆盖 do_request / do_response 的正常与错误分支
//   3. 复用 crate::sync::common 中的共性逻辑,不要复制粘贴
