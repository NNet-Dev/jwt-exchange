//! POST /api/v1/exchange — token exchange handler.
//!
//! Accepts an inbound IdP JWT (via Authorization header or body), validates
//! it against JWKS, checks replay protection, and returns a minted downstream
//! JWT. Logs all attempts to SQLite and Splunk.

use actix_web::{web, HttpRequest, HttpResponse, ResponseError};
use tracing::info;

use crate::app::AppState;
use crate::error::ServiceError;
use crate::middleware::RequestId;
use crate::models::api::{ExchangeRequest, ExchangeResponse};
use crate::services::{audit_service, token_service};

pub async fn exchange(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: Option<web::Json<ExchangeRequest>>,
) -> Result<HttpResponse, ServiceError> {
    let start = std::time::Instant::now();
    let request_id = RequestId::from_request(&req);

    // Token from header takes precedence; fall back to body if present
    let raw_token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(String::from)
        .or_else(|| body.as_ref().and_then(|b| b.token.clone()))
        .ok_or(ServiceError::MissingToken)?;

    let result = token_service::exchange_token(
        &state.config,
        &state.jwks_cache,
        &state.encoding_key,
        &state.http_client,
        &state.pool,
        &raw_token,
        body.as_ref().and_then(|b| b.groups.as_deref()),
    )
    .await;

    let source_ip = extract_source_ip(&req);
    let elapsed_ms = start.elapsed().as_millis() as i64;

    // Extract audit fields from result
    let (inbound_sub, inbound_iss, inbound_aud, outbound_sub, token_jti, code, err, validation) = match &result {
        Ok(r) => (
            Some(r.inbound_sub.clone()),
            r.inbound_iss.clone(),
            r.inbound_aud.clone(),
            Some(r.outbound_sub.clone()),
            Some(r.token_hash.clone()),
            200i64,
            None,
            "success",
        ),
        Err(ServiceError::ReplayDetected { inbound_sub, inbound_iss, inbound_aud, replay_id }) => (
            Some(inbound_sub.clone()),
            inbound_iss.clone(),
            inbound_aud.clone(),
            None,
            Some(replay_id.clone()),
            401i64,
            Some("token has already been used (replay detected)".to_string()),
            "success", // Token itself was valid — replay is a post-validation check
        ),
        Err(e) => (
            None,
            None,
            None,
            None,
            None,
            e.status_code().as_u16() as i64,
            Some(format!("{e}")),
            "",
        ),
    };
    let validation = if validation.is_empty() {
        err.as_deref().unwrap_or("")
    } else {
        validation
    };

    audit_service::log_exchange_attempt(
        state.pool.clone(),
        state.splunk_tx.clone(),
        source_ip,
        inbound_sub,
        inbound_iss,
        inbound_aud,
        outbound_sub,
        token_jti,
        code,
        elapsed_ms,
        validation.to_string(),
        err,
    );

    let r = match result {
        Ok(r) => r,
        Err(e) => return Ok(e.to_error_response(&request_id)),
    };

    info!(
        request_id,
        elapsed_ms,
        outbound_sub = r.outbound_sub,
        "Token exchange successful"
    );
    Ok(HttpResponse::Ok()
        .insert_header(("X-Request-Id", request_id))
        .json(ExchangeResponse {
            access_token: r.access_token,
            token_type: "Bearer".to_string(),
            expires_in: r.expires_in,
            issued_at: r.issued_at,
        }))
}

fn extract_source_ip(req: &HttpRequest) -> String {
    // Check X-Forwarded-For first (standard proxy header)
    if let Some(xff) = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.split(',').map(str::trim).find(|s| !s.is_empty()))
    {
        return xff.to_string();
    }

    // Fall back to X-Real-IP (nginx-style)
    if let Some(xri) = req
        .headers()
        .get("X-Real-IP")
        .and_then(|h| h.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return xri.to_string();
    }

    // Direct connection — peer address (container IP when no proxy)
    req.peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".into())
}
