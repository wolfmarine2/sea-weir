//! L1 单元 — 适配器共性逻辑。用例文档:`cases/04-unit-adaptors.md`
//!
//! 这里的测试**一次覆盖 35 个适配器的共性**,是投入产出比最高的一层。
//! 各适配器的专有转换在 `unit_convert.rs` 与录制回放中覆盖。

use pretty_assertions::assert_eq;
use sea_weir_adaptors::{sync::common, ApiType, TaskPlatform};

/// TC-UNI-ADP-001-POS:ApiType 数量锁死为 35。
///
/// 防止漏实现:注册表构建时会遍历 `ALL`,少一个就有渠道类型永远选不中。
#[test]
fn tc_uni_adp_001_pos_api_type_count() {
    assert_eq!(ApiType::ALL.len(), 35, "同步渠道适配器应为 35 个(不含仅作计数位的 Dummy)");
}

/// TC-UNI-ADP-002-POS:TaskPlatform 数量锁死为 10。
#[test]
fn tc_uni_adp_002_pos_task_platform_count() {
    assert_eq!(TaskPlatform::ALL.len(), 10);
}

/// TC-UNI-ADP-003-POS:ApiType 判别值与 Go 侧 iota 顺序一致。★
///
/// 渠道表 `type` 列存的是数字,枚举顺序错位会让所有存量渠道指向错误的适配器。
#[test]
fn tc_uni_adp_003_pos_api_type_discriminants_match_go_iota() {
    assert_eq!(ApiType::OpenAi as i32, 0);
    assert_eq!(ApiType::Anthropic as i32, 1);
    assert_eq!(ApiType::PaLM as i32, 2);
    assert_eq!(ApiType::Gemini as i32, 9);
    assert_eq!(ApiType::Ollama as i32, 11);
    assert_eq!(ApiType::VertexAi as i32, 19);
    assert_eq!(ApiType::Codex as i32, 34, "最后一个真实类型,Dummy 之前");
}

/// TC-UNI-ADP-004-POS:注册表对每个 ApiType 都有实现。★
#[test]
fn tc_uni_adp_004_pos_registry_covers_all_api_types() {
    let reg = sea_weir_adaptors::AdaptorRegistry::build();
    let missing: Vec<_> = ApiType::ALL.iter().filter(|t| reg.get(**t).is_none()).collect();
    assert!(missing.is_empty(), "以下 ApiType 缺少适配器实现:{missing:?}");
}

/// TC-UNI-ADP-005-POS:注册表对每个任务平台都有实现。
#[test]
fn tc_uni_adp_005_pos_registry_covers_all_task_platforms() {
    let reg = sea_weir_adaptors::AdaptorRegistry::build();
    let missing: Vec<_> = TaskPlatform::ALL.iter().filter(|t| reg.get_task(**t).is_none()).collect();
    assert!(missing.is_empty(), "以下任务平台缺少适配器实现:{missing:?}");
}

/// TC-UNI-ADP-006-BND:URL 拼接的三种边界。
///
/// base_url 由运维手填,尾斜杠与是否已含 `/v1` 都不可控,拼错会导致 404。
#[rstest::rstest]
#[case(None,                              "https://api.openai.com", "/v1/chat/completions", "https://api.openai.com/v1/chat/completions")]
#[case(Some("https://proxy.example.com"), "https://api.openai.com", "/v1/chat/completions", "https://proxy.example.com/v1/chat/completions")]
#[case(Some("https://proxy.example.com/"),"https://api.openai.com", "/v1/chat/completions", "https://proxy.example.com/v1/chat/completions")]
#[case(Some("https://proxy.example.com/v1"), "https://api.openai.com", "/v1/chat/completions", "https://proxy.example.com/v1/chat/completions")]
fn tc_uni_adp_006_bnd_join_url(
    #[case] base: Option<&str>,
    #[case] default_base: &str,
    #[case] path: &str,
    #[case] expected: &str,
) {
    assert_eq!(common::join_url(base, default_base, path), expected);
}

/// TC-UNI-ADP-007-POS:header_override 能覆盖既有头,也能新增。
#[test]
fn tc_uni_adp_007_pos_header_override() {
    // TODO(实现): 构造带 header_override 的 RelayInfo 后断言:
    //   - 覆盖已有的 Authorization
    //   - 新增自定义头
    //   - override 为 null 时不改动
}

/// TC-UNI-ADP-008-POS:param_override 是深合并而非整体替换。★
///
/// 若实现成整体替换,配置了 `{"temperature":0}` 会把用户的 messages 也覆盖掉。
#[test]
fn tc_uni_adp_008_pos_param_override_is_deep_merge() {
    let mut body = serde_json::json!({
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "hi"}],
        "temperature": 1.0
    });
    // TODO(实现): apply_param_override(&mut body, info_with(json!({"temperature": 0})))
    let _ = &mut body;
    // 期望:temperature 变 0,messages 与 model 保持不变
}

/// TC-UNI-ADP-009-POS:状态码重映射。无配置时原样返回。
#[rstest::rstest]
#[case(500, None,               500)]
#[case(500, Some("{\"500\":200}"), 200)]
#[case(404, Some("{\"500\":200}"), 404)]
fn tc_uni_adp_009_pos_status_code_mapping(
    #[case] upstream: u16,
    #[case] mapping: Option<&str>,
    #[case] expected: u16,
) {
    assert_eq!(common::map_status_code(upstream, mapping), expected);
}

/// TC-UNI-ADP-010-POS:渠道名在全表内唯一。
///
/// 渠道名进日志与前端展示,重名会让排障时无法区分。
#[test]
fn tc_uni_adp_010_pos_channel_names_unique() {
    let reg = sea_weir_adaptors::AdaptorRegistry::build();
    let mut names = Vec::new();
    for t in ApiType::ALL {
        if let Some(a) = reg.get(*t) {
            names.push(a.channel_name());
        }
    }
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "存在重名渠道:{names:?}");
}
