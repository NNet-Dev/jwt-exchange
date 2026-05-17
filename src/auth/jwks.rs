use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use jsonwebtoken::jwk::{Jwk, JwkSet};
use reqwest::Client;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::config::AppConfig;
use crate::error::ServiceError;

/// Default interval for JWKS cache refresh (5 minutes).
const JWKS_REFRESH_INTERVAL: Duration = Duration::from_secs(300);

/// F4/F29: Minimum cooldown between force-refresh attempts.
/// Prevents DoS amplification via fabricated kid values.
const JWKS_FORCE_REFRESH_COOLDOWN: Duration = Duration::from_secs(60);

/// F4/F29: Maximum force-refresh failures before circuit-breaker.
const JWKS_MAX_FORCE_REFRESH_FAILURES: usize = 5;

#[derive(Debug)]
pub struct CachedJwks {
    pub keys_by_kid: HashMap<String, Jwk>,
    pub etag: Option<String>,
    /// The resolved JWKS URI from well-known discovery.
    pub jwks_uri: String,
    /// F4/F29: Timestamp of the last force-refresh attempt.
    pub last_force_refresh: Option<std::time::Instant>,
    /// F4/F29: Count of consecutive force-refresh failures since last success.
    pub consecutive_force_refresh_failures: usize,
}

pub type JwksCache = Arc<RwLock<CachedJwks>>;

impl CachedJwks {
    /// Returns true if the JWKS cache contains at least one key.
    pub fn is_healthy(&self) -> bool {
        !self.keys_by_kid.is_empty()
    }
}

pub async fn fetch_and_cache_jwks(
    config: &AppConfig,
    http_client: &Client,
) -> Result<JwksCache, ServiceError> {
    info!("Fetching JWKS from IdP");

    let jwks_uri = discover_jwks_uri(config, http_client).await?;
    let (jwks, etag) = fetch_jwks_with_etag(&jwks_uri, http_client, None).await?;

    let keys_by_kid = index_keys(&jwks);
    info!(key_count = keys_by_kid.len(), "JWKS loaded successfully");

    Ok(Arc::new(RwLock::new(CachedJwks {
        keys_by_kid,
        etag,
        jwks_uri,
        last_force_refresh: None,
        consecutive_force_refresh_failures: 0,
    })))
}

async fn discover_jwks_uri(
    config: &AppConfig,
    http_client: &Client,
) -> Result<String, ServiceError> {
    let well_known_url = format!(
        "{}/.well-known/openid-configuration",
        config.inbound_issuer_uri
    );

    let mut attempt = 0;
    let max_wait = Duration::from_secs(60);
    let mut backoff = Duration::from_secs(1);

    loop {
        attempt += 1;
        match fetch_jwks_uri_from_well_known(&well_known_url, http_client).await {
            Ok(uri) => return Ok(uri),
            Err(e) => {
                warn!(attempt, error = %e, "Failed to fetch well-known config");
                if attempt * backoff.as_secs() >= max_wait.as_secs() {
                    return Err(ServiceError::IdPUnavailable);
                }
                sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
            }
        }
    }
}

async fn fetch_jwks_uri_from_well_known(
    url: &str,
    http_client: &Client,
) -> Result<String, ServiceError> {
    let resp = http_client
        .get(url)
        .send()
        .await
        .map_err(|e| ServiceError::Other(anyhow::anyhow!("well-known fetch failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(ServiceError::Other(anyhow::anyhow!(
            "well-known returned status {}",
            resp.status()
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ServiceError::Other(anyhow::anyhow!("well-known parse failed: {e}")))?;

    body.get("jwks_uri")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| {
            ServiceError::Other(anyhow::anyhow!("jwks_uri not found in well-known config"))
        })
}

async fn fetch_jwks_with_etag(
    jwks_uri: &str,
    http_client: &Client,
    etag: Option<&str>,
) -> Result<(JwkSet, Option<String>), ServiceError> {
    let mut req = http_client.get(jwks_uri);
    if let Some(etag_value) = etag {
        req = req.header("If-None-Match", etag_value);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| ServiceError::Jwks(format!("JWKS fetch failed: {e}")))?;

    if resp.status() == 304 {
        return Err(ServiceError::Jwks(
            "JWKS not modified (should use cached)".to_string(),
        ));
    }

    if !resp.status().is_success() {
        return Err(ServiceError::Jwks(format!(
            "JWKS fetch returned status {}",
            resp.status()
        )));
    }

    let new_etag = resp
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let body: JwkSet = resp
        .json()
        .await
        .map_err(|e| ServiceError::Jwks(format!("JWKS parse failed: {e}")))?;

    Ok((body, new_etag))
}

fn index_keys(jwks: &JwkSet) -> HashMap<String, Jwk> {
    jwks.keys
        .iter()
        .filter_map(|key| {
            key.common
                .key_id
                .as_ref()
                .map(|kid| (kid.clone(), key.clone()))
        })
        .collect()
}

pub async fn start_jwks_refresh_task(jwks_cache: JwksCache, http_client: Client) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(JWKS_REFRESH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            match refresh_jwks(&jwks_cache, &http_client).await {
                Ok(_) => debug!("JWKS refreshed successfully"),
                Err(e) => {
                    warn!(error = %e, "JWKS refresh failed, keeping existing cache")
                }
            }
        }
    });
}

async fn refresh_jwks(jwks_cache: &JwksCache, http_client: &Client) -> Result<(), ServiceError> {
    let (jwks_uri, etag) = {
        let cache = jwks_cache.read().await;
        (cache.jwks_uri.clone(), cache.etag.clone())
    };

    let (jwks, new_etag) = match fetch_jwks_with_etag(&jwks_uri, http_client, etag.as_deref()).await
    {
        Ok(result) => result,
        Err(ServiceError::Jwks(msg)) if msg.contains("not modified") => {
            debug!("JWKS not modified, skipping refresh");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    let keys_by_kid = index_keys(&jwks);
    let mut cache = jwks_cache.write().await;
    cache.keys_by_kid = keys_by_kid;
    cache.etag = new_etag;
    info!(key_count = cache.keys_by_kid.len(), "JWKS cache updated");
    Ok(())
}

/// Force-refresh JWKS, subject to cooldown and circuit-breaker limits (F4/F29).
/// Returns Ok(()) if the refresh succeeded, or an error if:
/// - The cooldown is still active, or
/// - The circuit-breaker has tripped (too many consecutive failures), or
/// - The actual JWKS fetch failed.
pub async fn force_refresh_jwks(
    jwks_cache: &JwksCache,
    http_client: &Client,
    jwks_uri: &str,
) -> Result<(), ServiceError> {
    // F4/F29: Check cooldown
    {
        let cache = jwks_cache.read().await;
        if let Some(last) = cache.last_force_refresh {
            let elapsed = last.elapsed();
            if elapsed < JWKS_FORCE_REFRESH_COOLDOWN {
                warn!(
                    elapsed_secs = elapsed.as_secs(),
                    cooldown_secs = JWKS_FORCE_REFRESH_COOLDOWN.as_secs(),
                    "JWKS force-refresh cooldown active"
                );
                return Err(ServiceError::Jwks(
                    "force-refresh cooldown active".to_string(),
                ));
            }
        }

        // F4/F29: Check circuit-breaker
        if cache.consecutive_force_refresh_failures >= JWKS_MAX_FORCE_REFRESH_FAILURES {
            warn!(
                failures = cache.consecutive_force_refresh_failures,
                max = JWKS_MAX_FORCE_REFRESH_FAILURES,
                "JWKS force-refresh circuit-breaker tripped"
            );
            return Err(ServiceError::Jwks(
                "force-refresh circuit-breaker tripped".to_string(),
            ));
        }
    }

    // Record the attempt timestamp
    {
        let mut cache = jwks_cache.write().await;
        cache.last_force_refresh = Some(std::time::Instant::now());
    }

    let result = fetch_jwks_with_etag(jwks_uri, http_client, None).await;
    match result {
        Ok((jwks, new_etag)) => {
            let keys_by_kid = index_keys(&jwks);
            let mut cache = jwks_cache.write().await;
            cache.keys_by_kid = keys_by_kid;
            cache.etag = new_etag;
            cache.consecutive_force_refresh_failures = 0; // Reset on success
            info!(
                key_count = cache.keys_by_kid.len(),
                "JWKS force-refreshed successfully"
            );
            Ok(())
        }
        Err(e) => {
            let mut cache = jwks_cache.write().await;
            cache.consecutive_force_refresh_failures += 1;
            warn!(
                error = %e,
                failures = cache.consecutive_force_refresh_failures,
                max = JWKS_MAX_FORCE_REFRESH_FAILURES,
                "JWKS force-refresh failed"
            );
            Err(e)
        }
    }
}
