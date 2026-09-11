# 04. 适配器单元测试用例

**版本**:v1.0 | **层**:L1 | **测试代码**:`test/src/unit_adaptor_common.rs`
**基准来源**:TEST-VECTORS.md;CONTRACTS.md §11

## 定位

35 个同步适配器 + 10 个任务适配器,逐个写测试成本极高。策略是:
**共性逻辑集中测(本文件),专有转换用录制回放测**(见 `06-unit-convert-stream.md`)。

`sync/common.rs` 的四个函数(URL 拼接、header/param override、状态码映射)
被 35 个适配器复用,一次覆盖等于覆盖全部。

## 用例清单

| 编号 | 名称 | 类型 | 优先级 |
|---|---|---|---|
| TC-UNI-ADP-001-POS | ApiType 数量 == 35 | 正向 | P0 |
| TC-UNI-ADP-002-POS | TaskPlatform 数量 == 10 | 正向 | P0 |
| TC-UNI-ADP-003-POS | 判别值与 Go iota 一致 ★ | 正向 | P0 |
| TC-UNI-ADP-004-POS | 注册表覆盖全部 ApiType ★ | 正向 | P0 |
| TC-UNI-ADP-005-POS | 注册表覆盖全部任务平台 | 正向 | P0 |
| TC-UNI-ADP-006-BND | URL 拼接四种边界 | 边界 | P0 |
| TC-UNI-ADP-007-POS | header_override 覆盖与新增 | 正向 | P1 |
| TC-UNI-ADP-008-POS | param_override 深合并 ★ | 正向 | P0 |
| TC-UNI-ADP-009-POS | 状态码重映射(3 组) | 正向 | P1 |
| TC-UNI-ADP-010-POS | 渠道名唯一 | 正向 | P2 |

## 关键用例详述

### TC-UNI-ADP-003-POS:判别值必须与 Go 侧 iota 顺序一致 ★

```yaml
背景: >
  渠道表的 type 列存的是**数字**。Rust 枚举的判别值若与 Go 的 iota 顺序错位,
  所有存量渠道都会指向错误的适配器 —— 比如配置的 OpenAI 渠道会用 Anthropic 的
  请求格式去调用,表现为大面积 400。
  这是数据迁移场景特有的风险,新项目不会遇到。

锚点:
  OpenAI:    0
  Anthropic: 1
  PaLM:      2
  Gemini:    9
  Ollama:    11
  VertexAi:  19
  Codex:     34    # 最后一个真实类型,Dummy 之前
```

### TC-UNI-ADP-004-POS:注册表全覆盖 ★

```yaml
背景: >
  注册表缺一个 ApiType,对应渠道类型就永远选不中,且**不会在启动时报错** ——
  要等到某个用户恰好用到那个渠道才会失败。
  因此这条断言应在启动时也执行一次(fail fast),测试只是双保险。

assert: "ApiType::ALL 中每一项 registry.get() 都返回 Some"
```

### TC-UNI-ADP-006-BND:URL 拼接 ★

```yaml
背景: >
  base_url 由运维手填,格式不可控:可能带尾斜杠、可能已含 /v1、可能为空(用默认)。
  拼错的表现是 404 或 405,排查时容易误判为"渠道挂了"。

matrix:
  - { base: null,                          path: "/v1/chat/completions", expect: "https://api.openai.com/v1/chat/completions" }
  - { base: "https://proxy.example.com",   path: "/v1/chat/completions", expect: "https://proxy.example.com/v1/chat/completions" }
  - { base: "https://proxy.example.com/",  path: "/v1/chat/completions", expect: "https://proxy.example.com/v1/chat/completions" }
  - { base: "https://proxy.example.com/v1",path: "/v1/chat/completions", expect: "https://proxy.example.com/v1/chat/completions" }
```

### TC-UNI-ADP-008-POS:param_override 必须深合并 ★

```yaml
背景: >
  运维用 param_override 强制某些参数(如把 temperature 锁成 0)。
  若实现成整体替换,配置 {"temperature":0} 会把用户请求里的 messages、model
  一起覆盖掉 —— 请求变成空壳,上游 400。

input:
  body:     { model: "gpt-4", messages: [...], temperature: 1.0 }
  override: { temperature: 0 }
expected:
  { model: "gpt-4", messages: [...原样...], temperature: 0 }
```

## 各适配器的标准测试套(实现阶段逐个铺开)

每个适配器落地时应有以下测试,建议做成宏或 rstest 模板复用:

1. `get_request_url` 在 base_url 三种形态下的拼接
2. `setup_request_header` 注入正确的鉴权头,且 header_override 能覆盖
3. `convert_request` 黄金用例:固定输入 → 固定上游请求体(快照测试)
4. `do_response` 非流式解析 usage;上游缺 usage 时的兜底
5. 上游 4xx/5xx → `NewApiError` 的 status/error_code 映射
6. 适配器自身解析失败 → `local_error = true`

**落地顺序**按渠道使用频度:openai → anthropic → gemini → deepseek →
volcengine → ali → 其余。
