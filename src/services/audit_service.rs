//! Fire-and-forget audit logging service.
//!
//! Writes exchange attempt records to SQLite and streams to Splunk HEC
//! (if configured). Spawns async tasks so logging never blocks the response.

use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::db::audit::{self, AuditRecord};
use crate::logging::splunk::SplunkEvent;

/// Log an exchange attempt to both SQLite audit log and Splunk (if configured).
/// Fire-and-forget — spawned on tokio runtime.
pub fn log_exchange_attempt(
    pool: SqlitePool,
    splunk_tx: Option<mpsc::Sender<SplunkEvent>>,
    source_ip: String,
    inbound_sub: Option<String>,
    inbound_iss: Option<String>,
    inbound_aud: Option<String>,
    outbound_sub: Option<String>,
    token_jti: Option<String>,
    response_code: i64,
    elapsed_ms: i64,
    validation: String,
    error_detail: Option<String>,
) {
    let record = AuditRecord {
        source_ip,
        inbound_sub,
        inbound_iss,
        inbound_aud,
        validation,
        error_detail: error_detail.clone(),
        outbound_sub,
        token_jti,
        response_code,
        elapsed_ms,
    };

    // Splunk forward — build from record before it's moved into the spawn
    if let Some(tx) = splunk_tx {
        let event = SplunkEvent::from_record(&record);
        let _ = tx.try_send(event);
    }

    // Fire-and-forget audit insert
    tokio::spawn(async move {
        audit::insert_audit_log(&pool, &record).await;
    });
}
