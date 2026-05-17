use crate::db::audit::AuditRecord;
use reqwest::Client;
use serde::Serialize;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

/// A single Splunk HEC event, derived from an audit record.
/// Deliberately excludes raw tokens and JTI values to avoid leaking
/// credentials into the SIEM (F13).
#[derive(Debug, Clone, Serialize)]
pub struct SplunkEvent {
    pub time: String,
    pub source_ip: String,
    pub inbound_sub: Option<String>,
    pub validation: String,
    pub error_detail: Option<String>,
    pub outbound_sub: Option<String>,
    /// Truncated JTI (first 8 hex chars) — enough for correlation,
    /// not enough to replay a token (F13).
    pub token_jti_hint: Option<String>,
    pub response_code: i64,
    pub elapsed_ms: i64,
}

impl SplunkEvent {
    /// Build from an existing AuditRecord — avoids 9-parameter sprawl.
    pub fn from_record(record: &AuditRecord) -> Self {
        let token_jti_hint = record.token_jti.as_ref().map(|jti| {
            if jti.len() > 8 { jti[..8].to_string() } else { jti.clone() }
        });
        Self {
            time: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            source_ip: record.source_ip.clone(),
            inbound_sub: record.inbound_sub.clone(),
            validation: record.validation.clone(),
            error_detail: record.error_detail.clone(),
            outbound_sub: record.outbound_sub.clone(),
            token_jti_hint,
            response_code: record.response_code,
            elapsed_ms: record.elapsed_ms,
        }
    }
}

/// Start the background Splunk HEC export task.
/// Reads from the provided receiver, batches events, and posts to Splunk.
/// The `skip_tls_verify` flag controls certificate validation (F1).
/// Default is `false` — TLS must be valid in production.
pub fn start_splunk_exporter(
    hec_url: String,
    hec_token: String,
    skip_tls_verify: bool,
    rx: mpsc::Receiver<SplunkEvent>,
) {
    tokio::spawn(async move {
        run_exporter(hec_url, hec_token, skip_tls_verify, rx).await;
    });
}

async fn run_exporter(
    hec_url: String,
    hec_token: String,
    skip_tls_verify: bool,
    mut rx: mpsc::Receiver<SplunkEvent>,
) {
    let mut client_builder = Client::builder();

    // F1: TLS verification is strict by default.
    // Only bypass when explicitly configured (e.g. internal CA not in system trust store).
    if skip_tls_verify {
        warn!("Splunk HEC TLS verification is DISABLED — only use with trusted internal CAs");
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }

    let client = client_builder.build().unwrap_or_default();

    let batch_size = 50;
    let flush_interval = Duration::from_secs(10);
    let max_retries = 3;
    let retry_delay = Duration::from_secs(2);
    let mut batch: Vec<SplunkEvent> = Vec::with_capacity(batch_size);
    let mut interval = tokio::time::interval(flush_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    info!(
        url = hec_url,
        tls_verify = !skip_tls_verify,
        "Splunk HEC exporter started"
    );

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(e) => {
                        batch.push(e);
                        if batch.len() >= batch_size {
                            flush_batch(&client, &hec_url, &hec_token, &mut batch, max_retries, retry_delay).await;
                        }
                    }
                    None => {
                        if !batch.is_empty() {
                            flush_batch(&client, &hec_url, &hec_token, &mut batch, max_retries, retry_delay).await;
                        }
                        info!("Splunk HEC channel closed, exporter shutting down");
                        break;
                    }
                }
            }
            _ = interval.tick() => {
                if !batch.is_empty() {
                    flush_batch(&client, &hec_url, &hec_token, &mut batch, max_retries, retry_delay).await;
                }
            }
        }
    }
}

async fn flush_batch(
    client: &Client,
    hec_url: &str,
    hec_token: &str,
    batch: &mut Vec<SplunkEvent>,
    max_retries: u32,
    retry_delay: Duration,
) {
    let events: Vec<SplunkEvent> = std::mem::take(batch);
    let hec_payload: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            serde_json::json!({
                "time": e.time,
                "host": "jwt-exchange",
                "source": "jwt-exchange",
                "sourcetype": "jwt-exchange:audit",
                "event": e,
            })
        })
        .collect();

    // F16: Retry with exponential backoff on transient failures.
    for attempt in 0..=max_retries {
        match client
            .post(hec_url)
            .header("Authorization", format!("Splunk {hec_token}"))
            .json(&hec_payload)
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    debug!(count = events.len(), "Splunk HEC batch sent");
                    return;
                }
                warn!(
                    status = %resp.status(),
                    attempt = attempt + 1,
                    "Splunk HEC batch rejected"
                );
            }
            Err(e) => {
                warn!(
                    error = %e,
                    attempt = attempt + 1,
                    max_retries = max_retries,
                    "Splunk HEC send failed"
                );
            }
        }

        if attempt < max_retries {
            tokio::time::sleep(retry_delay * (attempt + 1) as u32).await;
        }
    }

    // All retries exhausted — F16: log at error level so operators know data was lost.
    error!(
        count = events.len(),
        hec_url = %hec_url,
        "Splunk HEC batch lost after {} retries — audit data gap",
        max_retries
    );
}
