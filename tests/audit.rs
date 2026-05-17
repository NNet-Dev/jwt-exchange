use jwt_exchange::db::audit::check_and_record_jti;
use sqlx::SqlitePool;

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        r#"CREATE TABLE used_jti (
            jti TEXT NOT NULL,
            has_groups INTEGER NOT NULL DEFAULT 0,
            exp INTEGER NOT NULL,
            PRIMARY KEY (jti, has_groups)
        )"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

// ── Strict mode (allow_replay=false) ────────────────────────

#[tokio::test]
async fn strict_first_use_returns_false() {
    let pool = test_pool().await;
    let replayed = check_and_record_jti(&pool, "jti-1", 9999, false, false)
        .await
        .unwrap();
    assert!(!replayed, "first use should not be a replay");
}

#[tokio::test]
async fn strict_second_use_any_variant_returns_true() {
    let pool = test_pool().await;
    let first = check_and_record_jti(&pool, "jti-1", 9999, false, false)
        .await
        .unwrap();
    assert!(!first);

    let second = check_and_record_jti(&pool, "jti-1", 9999, false, true)
        .await
        .unwrap();
    assert!(
        second,
        "strict mode should block any second use of same JTI"
    );
}

#[tokio::test]
async fn strict_different_jti_not_blocked() {
    let pool = test_pool().await;
    check_and_record_jti(&pool, "jti-a", 9999, false, false)
        .await
        .unwrap();
    let replayed = check_and_record_jti(&pool, "jti-b", 9999, false, false)
        .await
        .unwrap();
    assert!(!replayed, "different JTI should not be blocked");
}

// ── Replay mode (allow_replay=true) ────────────────────────

#[tokio::test]
async fn replay_mode_first_use_no_groups() {
    let pool = test_pool().await;
    let replayed = check_and_record_jti(&pool, "jti-1", 9999, true, false)
        .await
        .unwrap();
    assert!(!replayed);
}

#[tokio::test]
async fn replay_mode_same_jti_with_groups_allowed() {
    let pool = test_pool().await;
    let first = check_and_record_jti(&pool, "jti-1", 9999, true, false)
        .await
        .unwrap();
    assert!(!first);

    let second = check_and_record_jti(&pool, "jti-1", 9999, true, true)
        .await
        .unwrap();
    assert!(
        !second,
        "replay mode should allow same JTI with different has_groups"
    );
}

#[tokio::test]
async fn replay_mode_third_use_blocked() {
    let pool = test_pool().await;
    check_and_record_jti(&pool, "jti-1", 9999, true, false)
        .await
        .unwrap();
    check_and_record_jti(&pool, "jti-1", 9999, true, true)
        .await
        .unwrap();

    let third = check_and_record_jti(&pool, "jti-1", 9999, true, false)
        .await
        .unwrap();
    assert!(
        third,
        "third use of same (jti, has_groups) pair should be blocked"
    );
}

#[tokio::test]
async fn replay_mode_duplicate_with_groups_blocked() {
    let pool = test_pool().await;
    check_and_record_jti(&pool, "jti-1", 9999, true, true)
        .await
        .unwrap();
    let second = check_and_record_jti(&pool, "jti-1", 9999, true, true)
        .await
        .unwrap();
    assert!(
        second,
        "duplicate (jti, has_groups) pair should be blocked"
    );
}

#[tokio::test]
async fn replay_mode_different_jti_independent() {
    let pool = test_pool().await;
    check_and_record_jti(&pool, "jti-a", 9999, true, false)
        .await
        .unwrap();
    check_and_record_jti(&pool, "jti-a", 9999, true, true)
        .await
        .unwrap();

    let replayed = check_and_record_jti(&pool, "jti-b", 9999, true, false)
        .await
        .unwrap();
    assert!(!replayed);
}
