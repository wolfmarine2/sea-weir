//! 测试公共设施:环境探测、fixture 加载、容器编排、断言辅助。

use std::sync::OnceLock;

/// 依赖连接参数。缺失时 L2/L4 套件按环境策略跳过(dev)或失败(test)。
pub struct TestEnv {
    pub database_url: Option<String>,
    pub redis_url: Option<String>,
    /// 已部署的 sea-weir 实例(L4 黑盒)。
    pub http_base_url: Option<String>,
    /// 严格模式:test 环境置 1,缺参数直接 panic 而非跳过。
    pub strict: bool,
}

impl TestEnv {
    pub fn get() -> &'static TestEnv {
        static ENV: OnceLock<TestEnv> = OnceLock::new();
        ENV.get_or_init(|| TestEnv {
            database_url: std::env::var("DATABASE_URL").ok(),
            redis_url: std::env::var("REDIS_URL").ok(),
            http_base_url: std::env::var("SEAWEIR_HTTP_BASE_URL").ok(),
            strict: std::env::var("STRICT").as_deref() == Ok("1"),
        })
    }
}

/// L2/L4 套件的准入检查。
///
/// - 非严格模式(dev):参数缺失 → 打印告警并返回 false,调用方 `return` 跳过
/// - 严格模式(test):参数缺失 → **panic**,杜绝"跳过依赖套件"的假绿
///
/// 这条策略沿用 service-auth 的教训:允许跳过就会出现依赖没配好但 CI 全绿的情况。
#[macro_export]
macro_rules! require_dep {
    ($opt:expr, $name:literal) => {
        match $opt.as_ref() {
            Some(v) => v.clone(),
            None => {
                if $crate::common::TestEnv::get().strict {
                    panic!("STRICT=1 但缺少 {} —— 拒绝跳过依赖套件", $name);
                }
                eprintln!("[skip] 缺少 {},跳过本用例(dev 策略)", $name);
                return;
            }
        }
    };
}

/// 加载录制的契约基线 fixture。
///
/// 文件位于 `cases/fixtures/baseline/<name>.json`,带 `recorded_from` /
/// `recorded_at` / `newapi_version` 元信息 —— 契约测试失败时先看这几个字段,
/// 判断是我方回归还是上游演进。
pub fn load_baseline(name: &str) -> serde_json::Value {
    let path = format!(
        "{}/cases/fixtures/baseline/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("读取基线 fixture 失败 {path}: {e}\n请先运行 test/env/record-baseline.sh"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("基线 fixture 解析失败 {path}: {e}"))
}

/// 加载 YAML 格式的黄金用例表。
pub fn load_vectors(name: &str) -> serde_yaml::Value {
    let path = format!("{}/cases/fixtures/{name}.yaml", env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("读取用例数据失败 {path}: {e}"));
    serde_yaml::from_str(&raw).unwrap_or_else(|e| panic!("用例数据解析失败 {path}: {e}"))
}

/// 比较两个 JSON 的**字段名集合**(递归),忽略取值。
///
/// 契约测试的核心工具:sea-weir 的响应允许新增字段,但**不允许改名或缺失**
/// 任何 new-api 已有的字段。返回 (缺失字段, 新增字段)。
pub fn diff_field_names(
    baseline: &serde_json::Value,
    actual: &serde_json::Value,
) -> (Vec<String>, Vec<String>) {
    fn walk(v: &serde_json::Value, prefix: &str, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, val) in m {
                    let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                    out.push(path.clone());
                    walk(val, &path, out);
                }
            }
            // 数组只看第一个元素的形状,避免长列表放大差异
            serde_json::Value::Array(a) => {
                if let Some(first) = a.first() {
                    walk(first, &format!("{prefix}[]"), out);
                }
            }
            _ => {}
        }
    }
    let (mut b, mut a) = (Vec::new(), Vec::new());
    walk(baseline, "", &mut b);
    walk(actual, "", &mut a);
    let missing: Vec<_> = b.iter().filter(|k| !a.contains(k)).cloned().collect();
    let extra: Vec<_> = a.iter().filter(|k| !b.contains(k)).cloned().collect();
    (missing, extra)
}

/// 断言 actual 不缺失 baseline 的任何字段(允许新增)。
pub fn assert_no_missing_fields(baseline: &serde_json::Value, actual: &serde_json::Value) {
    let (missing, _extra) = diff_field_names(baseline, actual);
    assert!(
        missing.is_empty(),
        "响应缺失 new-api 契约字段(会破坏客户端灰度替换):{missing:?}"
    );
}

/// new-api 中**确实存在**的 PascalCase 字段例外(2026-09-10 录制实测)。
///
/// 这些不是我们想要的风格,但它们是既成契约 —— 客户端可能已经在读这些键,
/// 改名会破坏兼容。sea-weir 必须原样保留。
///
/// - `DeletedAt`:GORM 的 `gorm.DeletedAt` 内嵌字段没写 json tag,序列化成了 Go 字段名
/// - `HeaderNavModules` / `SidebarModulesAdmin`:`/api/status` 里的配置键
const PASCAL_CASE_EXCEPTIONS: &[&str] = &["DeletedAt", "HeaderNavModules", "SidebarModulesAdmin"];

/// 判断某个键是否是**数据**而非字段名。
///
/// JSON 对象既可能是结构体(键=字段名,受命名约定约束),也可能是 map
/// (键=用户数据,不受约束)。例如 `/api/status` 的 `chats` 是 map,
/// 键是用户自定义的聊天客户端名(如 `"Cherry Studio"`)—— 拿它去校验命名是误报。
///
/// 启发式:含空格或非 ASCII 的键一定是数据。
fn looks_like_data_key(seg: &str) -> bool {
    seg.contains(' ') || !seg.is_ascii()
}

/// 断言字段名全部为 snake_case。
///
/// 守住审查发现的 P0 偏差:v1.0 文档曾规定 camelCase,而 new-api 以 snake_case 为主。
/// 已知例外见 [`PASCAL_CASE_EXCEPTIONS`];map 的数据键不参与校验。
pub fn assert_all_snake_case(v: &serde_json::Value) {
    let (_, all) = diff_field_names(&serde_json::json!({}), v);
    let bad: Vec<_> = all
        .iter()
        .filter(|p| {
            let seg = p.rsplit('.').next().unwrap_or("").trim_end_matches("[]");
            seg.chars().any(|c| c.is_ascii_uppercase())
                && !PASCAL_CASE_EXCEPTIONS.contains(&seg)
                && !looks_like_data_key(seg)
        })
        .collect();
    assert!(
        bad.is_empty(),
        "发现非 snake_case 字段(破坏契约兼容):{bad:?}\n         若确认是 new-api 的既成例外,加入 PASCAL_CASE_EXCEPTIONS 并在 TEST-VECTORS 记录"
    );
}
