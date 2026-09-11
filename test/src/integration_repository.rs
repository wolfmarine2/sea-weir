//! L2 集成 — Repository 额度不变量。用例文档:`cases/05-integration-repository.md`
//!
//! **必须用真实数据库**:条件原子扣减依赖 `UPDATE ... WHERE quota >= $1` 的
//! 影响行数语义,行锁依赖 `SELECT ... FOR UPDATE`,唯一约束依赖真实索引 ——
//! 这三条 mock 一条都验证不了。
//!
//! 溯源:ADR-005、TEST-VECTORS.md §8

use crate::require_dep;
use pretty_assertions::assert_eq;

/// 起 openGauss 容器并跑迁移,返回连接池。
async fn setup_db() -> sqlx::PgPool {
    // TODO(实现): testcontainers 起 postgres(openGauss 兼容)+ 执行 migrations
    todo!("testcontainers 编排")
}

// ───────────── 条件原子扣减(TEST-VECTORS §8)─────────────

/// TC-INT-REP-001-BND:条件原子扣减的余额边界。★
///
/// 关键在"余额不足时余额**不变**"——这正是 new-api 缺失的守卫
/// (`model/user.go:928` 是无条件的 `quota - ?`,并发下会扣成负值)。
#[rstest::rstest]
#[case(1000, 300, true,  700)]
#[case(300,  300, true,  0)]     // 恰好等于
#[case(299,  300, false, 299)]   // 不足 → 余额不变
#[case(0,    1,   false, 0)]
#[case(100,  0,   true,  100)]   // 扣 0
#[tokio::test]
async fn tc_int_rep_001_bnd_conditional_atomic_decrease(
    #[case] initial: i64,
    #[case] amount: i64,
    #[case] expect_ok: bool,
    #[case] expect_remain: i64,
) {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    let _pool = setup_db().await;

    // TODO(实现):
    //   1. 建用户,quota = initial
    //   2. repo.try_decrease_quota(id, amount)
    //   3. 断言返回值 == expect_ok
    //   4. 重新查库,断言 quota == expect_remain
    let _ = (initial, amount, expect_ok, expect_remain);
}

/// TC-INT-REP-002-CNC:并发扣减,余额不得为负。★★
///
/// 本项目账务正确性的**核心断言**。初始 1000,10 个并发各扣 300:
/// 成功恰好 3 次,终值 100,任何时刻不为负。
#[tokio::test]
async fn tc_int_rep_002_cnc_concurrent_decrease_never_negative() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    let _pool = setup_db().await;

    // TODO(实现):
    //   1. 建用户,quota = 1000
    //   2. futures::future::join_all 发起 10 个并发 try_decrease_quota(id, 300)
    //   3. 断言 success_count == 3
    //   4. 断言最终 quota == 100
    //   5. 断言 quota >= 0(即使实现有 bug 也要显式断言,便于定位)
    let success_count = 0usize;
    let final_quota = 0i64;
    assert_eq!(success_count, 3, "10 个并发扣 300,1000 余额应恰好成功 3 次");
    assert_eq!(final_quota, 100);
    assert!(final_quota >= 0, "余额绝不能为负");
}

/// TC-INT-REP-003-CNC:增减交叉并发后总额守恒。
///
/// 模拟"边消费边退款"的真实场景。
#[tokio::test]
async fn tc_int_rep_003_cnc_mixed_increase_decrease_conserves() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 初始 10000;50 个并发扣 100、50 个并发加 100;
    //             终值应为 10000(全部扣减都能成功)
}

/// TC-INT-REP-004-POS:令牌 unlimited_quota 时不扣库。
///
/// 无限额度令牌若仍走扣减,会产生无意义的数据库写入并把 remain_quota 扣成负数。
#[tokio::test]
async fn tc_int_rep_004_pos_unlimited_token_skips_write() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): unlimited_quota=true 的令牌 try_decrease_quota → true,
    //             且 remain_quota 保持不变
}

// ───────────── abilities 事务一致性 ─────────────

/// TC-INT-REP-010-POS:sync_abilities 中途失败必须整体回滚。★
///
/// 实现是"事务内先删后插"。若事务边界写错,失败会留下"删了没插"的空洞 ——
/// 表现为该渠道的所有模型突然不可用。
#[tokio::test]
async fn tc_int_rep_010_pos_sync_abilities_rollback_on_failure() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现):
    //   1. 渠道 A 已有 3 条 abilities
    //   2. 构造使插入阶段失败的输入(如超长 model 名)
    //   3. 断言 sync_abilities 返回 Err
    //   4. 断言 abilities 仍是原来的 3 条(无半删状态)
}

/// TC-INT-REP-011-POS:渠道禁用后不出现在 list_enabled。
#[tokio::test]
async fn tc_int_rep_011_pos_disabled_channel_excluded() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): update_status(id, AUTO_DISABLED) 后 list_enabled 不含该渠道
}

/// TC-INT-REP-012-POS:多 key 渠道按 key 粒度禁用。
///
/// 一个 key 失效不应让整个渠道下线。
#[tokio::test]
async fn tc_int_rep_012_pos_multi_key_partial_disable() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): disable_key(id, 1) 后:
    //   - channel_info 中下标 1 标记禁用
    //   - 渠道 status 仍为 ENABLED
    //   - 下标 0、2 仍可被选中
}

// ───────────── 软删除与部分索引 ─────────────

/// TC-INT-REP-020-POS:软删除后同名可重建。
///
/// 唯一约束用部分索引 `WHERE deleted_at IS NULL`。若用普通唯一索引,
/// 删除过的用户名将永远无法重新注册。
#[tokio::test]
async fn tc_int_rep_020_pos_soft_delete_allows_reuse() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 建用户 alice → 软删除 → 再建 alice 应成功
}

/// TC-INT-REP-021-NEG:未删除时同名冲突。
#[tokio::test]
async fn tc_int_rep_021_neg_duplicate_username_rejected() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 重复建 alice → Err
}

// ───────────── 日志分库 ─────────────

/// TC-INT-REP-030-POS:log_dsn 为空时日志写主库。
#[tokio::test]
async fn tc_int_rep_030_pos_log_falls_back_to_main_db() {
    let _url = require_dep!(crate::common::TestEnv::get().database_url, "DATABASE_URL");
    // TODO(实现): 配置 log_dsn=None,断言两个 pool 指向同一实例且日志可写可查
}
