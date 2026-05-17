//! Database access layer for the JWT Exchange service.
//!
//! Handles SQLite interactions for audit logging, JTI replay protection,
//! and background purge tasks.
//!
//! ## Tables
//! - `request_log`: Audit records for all token exchange attempts.
//! - `used_jti`: JWT ID tracking for replay detection (`jti TEXT PRIMARY KEY, exp INTEGER`).
//!
//! ## Background Tasks
//! - `start_purge_task`: Daily cleanup of expired audit logs.
//! - `start_jti_purge_task`: Hourly cleanup of expired JTI entries.

use sqlx::SqlitePool;
use tracing::{debug, error};

/// A single audit record representing a token exchange attempt.
/// Captures both successful mints and failed validations for SIEM ingestion.
pub struct AuditRecord {
    pub source_ip: String,
    pub inbound_sub: Option<String>,
    pub inbound_iss: Option<String>,
    pub inbound_aud: Option<String>,
    pub validation: String,
    pub error_detail: Option<String>,
    pub outbound_sub: Option<String>,
    pub token_jti: Option<String>,
    pub response_code: i64,
    pub elapsed_ms: i64,
}

/// Insert an audit record into the `request_log` table.
/// Fire-and-forget — errors are logged but not propagated.
pub async fn insert_audit_log(pool: &SqlitePool, record: &AuditRecord) {
    let result = sqlx::query!(
        r#"INSERT INTO request_log (source_ip, inbound_sub, inbound_iss, inbound_aud, validation, error_detail, outbound_sub, token_jti, response_code, elapsed_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
        record.source_ip,
        record.inbound_sub,
        record.inbound_iss,
        record.inbound_aud,
        record.validation,
        record.error_detail,
        record.outbound_sub,
        record.token_jti,
        record.response_code,
        record.elapsed_ms
    )
    .execute(pool)
    .await;

    if let Err(e) = result {
        error!(error = %e, "failed to insert audit log");
    }
}

/// Delete audit log entries older than the configured retention period.
/// Returns the number of rows deleted.
pub async fn purge_old_logs(pool: &SqlitePool, retention_days: i64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query!(
        r#"DELETE FROM request_log WHERE timestamp < datetime('now', '-' || ?1 || ' days')"#,
        retention_days
    )
    .execute(pool)
    .await?;

    let deleted = result.rows_affected();
    if deleted > 0 {
        debug!(deleted, "purged old log entries");
    }
    Ok(deleted)
}

/// Atomically check-and-insert a JTI to prevent replay attacks.
///
/// Uses `INSERT OR IGNORE` on the `used_jti` table. If the JTI already
/// Atomically check-and-insert a JTI to prevent replay attacks.
/// Returns `true` if the JTI was already used, `false` if successfully recorded.
///
/// When `allow_replay` is true, the replay key is `(jti, has_groups)`,
/// allowing the same token to be exchanged once with groups and once without.
/// When `allow_replay` is false (default), any prior use of the JTI blocks
/// all subsequent exchanges regardless of group presence.
pub async fn check_and_record_jti(
    pool: &SqlitePool,
    jti: &str,
    exp: i64,
    allow_replay: bool,
    has_groups: bool,
) -> Result<bool, sqlx::Error> {
    if allow_replay {
        // Composite key: same JTI can be used once per has_groups variant.
        let has_groups_int = if has_groups { 1 } else { 0 };
        let result = sqlx::query!(
            r#"INSERT OR IGNORE INTO used_jti (jti, has_groups, exp) VALUES (?1, ?2, ?3)"#,
            jti,
            has_groups_int,
            exp,
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 0)
    } else {
        // Strict mode: any prior use of this JTI blocks all future exchanges.
        let existing = sqlx::query!("SELECT 1 AS one FROM used_jti WHERE jti = ?1", jti)
            .fetch_optional(pool)
            .await?;
        if existing.is_some() {
            return Ok(true); // replay detected
        }
        let has_groups_int: i64 = 0; // irrelevant in strict mode, but required by schema
        sqlx::query!(
            r#"INSERT INTO used_jti (jti, has_groups, exp) VALUES (?1, ?2, ?3)"#,
            jti,
            has_groups_int,
            exp,
        )
        .execute(pool)
        .await?;
        Ok(false)
    }
}

/// Purge expired JTIs from the `used_jti` table.
/// Removes entries whose `exp` timestamp is in the past.
pub async fn purge_expired_jtis(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let now = chrono::Utc::now().timestamp();
    let result = sqlx::query!(
        r#"DELETE FROM used_jti WHERE exp < ?1"#,
        now
    )
    .execute(pool)
    .await?;

    let deleted = result.rows_affected();
    if deleted > 0 {
        debug!(deleted, "purged expired JTIs");
    }
    Ok(deleted)
}

/// Spawn the daily audit log purge task in the background.
/// Runs every 24 hours, deleting records older than `retention_days`.
pub fn start_purge_task(pool: SqlitePool, retention_days: i64) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400)); // daily
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        tracing::info!(retention_days, "Auto-purge task started (daily schedule)");

        loop {
            interval.tick().await;
            match purge_old_logs(&pool, retention_days).await {
                Ok(deleted) => {
                    if deleted > 0 {
                        tracing::info!(deleted, "Purged old audit log entries");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Auto-purge failed");
                }
            }
        }
    });
}

/// Spawn the hourly JTI replay-table cleanup task in the background.
/// Runs every 60 minutes, removing expired JTI entries to bound table growth.
pub fn start_jti_purge_task(pool: SqlitePool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // hourly
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        tracing::info!("JTI expiry purge task started (hourly schedule)");

        loop {
            interval.tick().await;
            match purge_expired_jtis(&pool).await {
                Ok(deleted) => {
                    if deleted > 0 {
                        tracing::info!(deleted, "Purged expired JTIs from replay table");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "JTI purge failed");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let replayed = check_and_record_jti(&pool, "jti-1", 9999, false, false).await.unwrap();
        assert!(!replayed, "first use should not be a replay");
    }

    #[tokio::test]
    async fn strict_second_use_any_variant_returns_true() {
        let pool = test_pool().await;
        let first = check_and_record_jti(&pool, "jti-1", 9999, false, false).await.unwrap();
        assert!(!first);

        let second = check_and_record_jti(&pool, "jti-1", 9999, false, true).await.unwrap();
        assert!(second, "strict mode should block any second use of same JTI");
    }

    #[tokio::test]
    async fn strict_different_jti_not_blocked() {
        let pool = test_pool().await;
        check_and_record_jti(&pool, "jti-a", 9999, false, false).await.unwrap();
        let replayed = check_and_record_jti(&pool, "jti-b", 9999, false, false).await.unwrap();
        assert!(!replayed, "different JTI should not be blocked");
    }

    // ── Replay mode (allow_replay=true) ────────────────────────

    #[tokio::test]
    async fn replay_mode_first_use_no_groups() {
        let pool = test_pool().await;
        let replayed = check_and_record_jti(&pool, "jti-1", 9999, true, false).await.unwrap();
        assert!(!replayed);
    }

    #[tokio::test]
    async fn replay_mode_same_jti_with_groups_allowed() {
        let pool = test_pool().await;
        let first = check_and_record_jti(&pool, "jti-1", 9999, true, false).await.unwrap();
        assert!(!first);

        let second = check_and_record_jti(&pool, "jti-1", 9999, true, true).await.unwrap();
        assert!(!second, "replay mode should allow same JTI with different has_groups");
    }

    #[tokio::test]
    async fn replay_mode_third_use_blocked() {
        let pool = test_pool().await;
        check_and_record_jti(&pool, "jti-1", 9999, true, false).await.unwrap();
        check_and_record_jti(&pool, "jti-1", 9999, true, true).await.unwrap();

        let third = check_and_record_jti(&pool, "jti-1", 9999, true, false).await.unwrap();
        assert!(third, "third use of same (jti, has_groups) pair should be blocked");
    }

    #[tokio::test]
    async fn replay_mode_duplicate_with_groups_blocked() {
        let pool = test_pool().await;
        check_and_record_jti(&pool, "jti-1", 9999, true, true).await.unwrap();
        let second = check_and_record_jti(&pool, "jti-1", 9999, true, true).await.unwrap();
        assert!(second, "duplicate (jti, has_groups) pair should be blocked");
    }

    #[tokio::test]
    async fn replay_mode_different_jti_independent() {
        let pool = test_pool().await;
        check_and_record_jti(&pool, "jti-a", 9999, true, false).await.unwrap();
        check_and_record_jti(&pool, "jti-a", 9999, true, true).await.unwrap();

        let replayed = check_and_record_jti(&pool, "jti-b", 9999, true, false).await.unwrap();
        assert!(!replayed);
    }
}
