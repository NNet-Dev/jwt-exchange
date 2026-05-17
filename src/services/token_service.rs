//! Core token exchange service.
//!
//! Validates inbound IdP JWTs against JWKS, checks replay protection via
//! JTI or SHA-256 hash, filters groups against a whitelist, and mints
//! new RSA-256 signed JWTs for the downstream service.

use std::collections::HashSet;

use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::auth::jwks::{force_refresh_jwks, JwksCache};
use crate::config::AppConfig;
use crate::error::ServiceError;

/// Claims extracted from the inbound identity provider JWT.
/// Some fields are consumed by jsonwebtoken's validation layer (aud, iss)
/// and are not read directly in our code, but must remain for deserialization.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct InboundClaims {
    pub sub: Option<String>,
    pub iss: Option<String>,
    pub aud: Option<serde_json::Value>,
    pub exp: Option<i64>,
    pub iat: Option<i64>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub jti: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QlikClaims {
    pub userid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userdirectory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<String>>,
    pub iss: String,
    pub aud: String,
    pub exp: i64,
    pub nbf: i64,
    pub iat: i64,
    pub jti: String,
}

pub struct TokenExchangeResult {
    pub access_token: String,
    pub expires_in: i64,
    pub issued_at: String,
    pub outbound_sub: String,
    pub token_jti: String,
    /// SHA-256 hash of the raw incoming token, used for replay protection when JTI is absent.
    pub token_hash: String,
    /// Inbound claim fields for audit logging.
    pub inbound_sub: String,
    pub inbound_iss: Option<String>,
    pub inbound_aud: Option<String>,
}

pub async fn exchange_token(
    config: &AppConfig,
    jwks_cache: &JwksCache,
    encoding_key: &EncodingKey,
    http_client: &Client,
    pool: &sqlx::SqlitePool,
    raw_token: &str,
    requested_groups: Option<&[String]>,
) -> Result<TokenExchangeResult, ServiceError> {
    // Input validation: token size limit (F7)
    if raw_token.len() > config.max_token_size {
        return Err(ServiceError::MalformedToken);
    }

    // Step 1: Decode header to extract kid
    let kid = extract_kid(raw_token)?;

    // Step 2: Try to find key in cache
    let decoding_key = {
        let cache = jwks_cache.read().await;
        match cache.keys_by_kid.get(&kid) {
            Some(jwk) => DecodingKey::from_jwk(jwk)
                .map_err(|e| ServiceError::Jwks(format!("invalid JWK for kid {kid}: {e}")))?,
            None => {
                drop(cache);
                // Step 3: Force-refresh JWKS and retry
                warn!(kid, "Unknown kid in JWT, force-refreshing JWKS");
                let jwks_uri = {
                    let c = jwks_cache.read().await;
                    c.jwks_uri.clone()
                };
                force_refresh_jwks(jwks_cache, http_client, &jwks_uri).await?;

                let cache = jwks_cache.read().await;
                match cache.keys_by_kid.get(&kid) {
                    Some(jwk) => DecodingKey::from_jwk(jwk).map_err(|e| {
                        ServiceError::Jwks(format!("invalid JWK for kid {kid}: {e}"))
                    })?,
                    None => {
                        let available: Vec<&str> =
                            cache.keys_by_kid.keys().map(|k| k.as_str()).collect();
                        warn!(
                            kid,
                            available_kids = ?available,
                            jwks_uri = %cache.jwks_uri,
                            "KID not found in JWKS after force-refresh"
                        );
                        return Err(ServiceError::UnknownKid);
                    }
                }
            }
        }
    };

    // Step 4: Validate and decode the token
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[config.inbound_issuer_uri.as_str()]);
    validation.leeway = 30;

    // Audience validation: enabled by default (F6)
    if config.inbound_audience_validation_enabled {
        if let Some(ref expected_aud) = config.inbound_expected_audience {
            validation.set_audience(&[expected_aud.as_str()]);
        }
    } else {
        validation.validate_aud = false;
    }

    let token_data = match decode::<InboundClaims>(raw_token, &decoding_key, &validation) {
        Ok(data) => data,
        Err(e) => {
            warn!(error = %e, "JWT validation failed");
            return match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    Err(ServiceError::ExpiredToken)
                }
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => Err(ServiceError::InvalidIssuer),
                jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                    Err(ServiceError::InvalidAudience)
                }
                jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                    Err(ServiceError::InvalidSignature)
                }
                jsonwebtoken::errors::ErrorKind::InvalidToken => Err(ServiceError::MalformedToken),
                _ => Err(ServiceError::InvalidSignature),
            };
        }
    };

    let claims = &token_data.claims;

    let inbound_sub = claims
        .sub
        .clone()
        .ok_or_else(|| ServiceError::MalformedToken)?;

    info!(inbound_sub, "Validated inbound JWT");

    // Step 5: Replay protection — use JTI if present, else SHA-256 hash of raw token (F2)
    let replay_id = if let Some(ref jti) = claims.jti {
        jti.clone()
    } else {
        hash_token(raw_token)
    };

    let token_exp = claims.exp.unwrap_or(i64::MAX);
    let has_groups = requested_groups.map_or(false, |g| !g.is_empty());
    let already_used = crate::db::audit::check_and_record_jti(
        pool,
        &replay_id,
        token_exp,
        config.allow_replay,
        has_groups,
    )
    .await
    .map_err(|e| ServiceError::Other(e.into()))?;
    if already_used {
        warn!(replay_id, inbound_sub, "Replay attack detected");
        return Err(ServiceError::ReplayDetected);
    }

    // Step 6: Build Qlik claims
    let iat = Utc::now().timestamp();
    let inbound_exp = claims.exp.unwrap_or(i64::MAX);
    let configured_exp = iat + config.token_ttl_seconds;
    let exp = std::cmp::min(configured_exp, inbound_exp);

    // Filter groups against whitelist, validate size limits (F7)
    let filtered_groups = filter_groups(
        requested_groups,
        &config.groups_whitelist,
        config.max_groups_count,
        config.max_group_name_length,
    )?;

    let token_jti = uuid::Uuid::new_v4().to_string();

    let qlik_claims = QlikClaims {
        userid: inbound_sub.clone(),
        userdirectory: config.qlik_user_directory.clone(),
        name: claims.name.clone(),
        email: claims.email.clone(),
        groups: filtered_groups,
        iss: "jwt-exchange".to_string(),
        aud: config.qlik_audience.clone(),
        exp,
        nbf: iat,
        iat,
        jti: token_jti.clone(),
    };

    info!(
        outbound_sub = %inbound_sub,
        aud = %config.qlik_audience,
        iss = "jwt-exchange",
        exp = %exp,
        "Minting Qlik JWT"
    );

    // Step 7: Sign the token
    let header = Header::new(Algorithm::RS256);
    let access_token = encode(&header, &qlik_claims, encoding_key)
        .map_err(|e| ServiceError::Signing(format!("JWT signing failed: {e}")))?;

    let expires_in = exp - iat;
    let issued_at = chrono::DateTime::<Utc>::from_timestamp(iat, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    Ok(TokenExchangeResult {
        access_token,
        expires_in,
        issued_at,
        outbound_sub: inbound_sub.clone(),
        token_jti,
        token_hash: replay_id,
        inbound_sub,
        inbound_iss: claims.iss.clone(),
        inbound_aud: claims
            .aud
            .as_ref()
            .and_then(|v| v.as_str().map(String::from)),
    })
}

/// Compute SHA-256 hex digest of the raw token string for replay identification.
fn hash_token(raw_token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_token.as_bytes());
    hex::encode(hasher.finalize())
}

fn extract_kid(token: &str) -> Result<String, ServiceError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(ServiceError::MalformedToken);
    }

    let header_bytes = base64url_decode(parts[0]).ok_or_else(|| ServiceError::MalformedToken)?;

    let header: serde_json::Value =
        serde_json::from_slice(&header_bytes).map_err(|_| ServiceError::MalformedToken)?;

    header
        .get("kid")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| ServiceError::MalformedToken)
}

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let mut buf = Vec::with_capacity(input.len() * 3 / 4);
    let mut bits: u32 = 0;
    let mut bit_count: u32 = 0;

    for &byte in input.as_bytes() {
        let val = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            b'=' => break,
            _ => continue,
        };

        bits = (bits << 6) | val as u32;
        bit_count += 6;

        if bit_count >= 8 {
            bit_count -= 8;
            buf.push((bits >> bit_count) as u8);
            bits &= (1 << bit_count) - 1;
        }
    }

    Some(buf)
}

fn filter_groups(
    requested: Option<&[String]>,
    whitelist: &HashSet<String>,
    max_count: usize,
    max_name_length: usize,
) -> Result<Option<Vec<String>>, ServiceError> {
    let Some(groups) = requested else {
        return Ok(None);
    };

    if whitelist.is_empty() {
        return Ok(None);
    }

    // Validate group count (F7)
    if groups.len() > max_count {
        return Err(ServiceError::MalformedToken);
    }

    // Validate each group name length (F7)
    if groups.iter().any(|g| g.len() > max_name_length) {
        return Err(ServiceError::MalformedToken);
    }

    let filtered: Vec<String> = groups
        .iter()
        .filter(|g| whitelist.contains(g.as_str()))
        .cloned()
        .collect();

    if filtered.is_empty() {
        Ok(None)
    } else {
        debug!(groups = ?filtered, "Filtered groups against whitelist");
        Ok(Some(filtered))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{base64url_decode, filter_groups, hash_token};

    // ── filter_groups ───────────────────────────────────────────

    #[test]
    fn filter_groups_all_match() {
        let whitelist: HashSet<String> = ["admins", "analysts", "developers"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let groups = vec!["admins".to_string(), "analysts".to_string()];
        let result = filter_groups(Some(&groups), &whitelist, 50, 256);
        assert_eq!(
            result.unwrap(),
            Some(vec!["admins".to_string(), "analysts".to_string()])
        );
    }

    #[test]
    fn filter_groups_some_filtered() {
        let whitelist: HashSet<String> = ["admins", "analysts"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let groups = vec![
            "admins".to_string(),
            "unknown-group".to_string(),
            "analysts".to_string(),
        ];
        let result = filter_groups(Some(&groups), &whitelist, 50, 256);
        assert_eq!(
            result.unwrap(),
            Some(vec!["admins".to_string(), "analysts".to_string()])
        );
    }

    #[test]
    fn filter_groups_none_match_returns_none() {
        let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
        let groups = vec!["random".to_string(), "other".to_string()];
        let result = filter_groups(Some(&groups), &whitelist, 50, 256);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn filter_groups_none_input_returns_none() {
        let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
        let result = filter_groups(None, &whitelist, 50, 256);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn filter_groups_empty_whitelist_returns_none() {
        let whitelist: HashSet<String> = HashSet::new();
        let groups = vec!["admins".to_string()];
        let result = filter_groups(Some(&groups), &whitelist, 50, 256);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn filter_groups_empty_input_returns_none() {
        let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
        let groups: Vec<String> = vec![];
        let result = filter_groups(Some(&groups), &whitelist, 50, 256);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn filter_groups_preserves_order() {
        let whitelist: HashSet<String> = ["z-group", "a-group", "m-group"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let groups = vec![
            "m-group".to_string(),
            "z-group".to_string(),
            "x-group".to_string(),
            "a-group".to_string(),
        ];
        let result = filter_groups(Some(&groups), &whitelist, 50, 256);
        assert_eq!(
            result.unwrap(),
            Some(vec![
                "m-group".to_string(),
                "z-group".to_string(),
                "a-group".to_string()
            ])
        );
    }

    #[test]
    fn filter_groups_preserves_duplicates() {
        let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
        let groups = vec!["admins".to_string(), "admins".to_string()];
        let result = filter_groups(Some(&groups), &whitelist, 50, 256);
        assert_eq!(
            result.unwrap(),
            Some(vec!["admins".to_string(), "admins".to_string()])
        );
    }

    #[test]
    fn filter_groups_exceeds_max_count_returns_error() {
        let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
        let groups: Vec<String> = (0..51).map(|i| format!("group-{i}")).collect();
        let result = filter_groups(Some(&groups), &whitelist, 50, 256);
        assert!(result.is_err());
    }

    #[test]
    fn filter_groups_exceeds_max_name_length_returns_error() {
        let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
        let groups = vec!["a".repeat(257)];
        let result = filter_groups(Some(&groups), &whitelist, 50, 256);
        assert!(result.is_err());
    }

    // ── base64url_decode ────────────────────────────────────────

    #[test]
    fn decode_jwt_header() {
        // {"alg":"RS256","typ":"JWT"} encoded
        let encoded = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9";
        let decoded = base64url_decode(encoded).unwrap();
        let s = String::from_utf8(decoded).unwrap();
        assert!(s.contains("RS256"));
        assert!(s.contains("JWT"));
    }

    #[test]
    fn decode_empty_string() {
        let result = base64url_decode("");
        assert_eq!(result, Some(vec![]));
    }

    #[test]
    fn decode_simple() {
        // "hello" -> "aGVsbG8"
        let result = base64url_decode("aGVsbG8").unwrap();
        assert_eq!(result, b"hello");
    }

    // ── hash_token ──────────────────────────────────────────────

    #[test]
    fn hash_token_produces_consistent_sha256() {
        let token = "header.payload.signature";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn hash_token_differs_for_different_inputs() {
        let hash1 = hash_token("token-a");
        let hash2 = hash_token("token-b");
        assert_ne!(hash1, hash2);
    }
}
