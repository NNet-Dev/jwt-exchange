//! Error types and HTTP response formatting.
//!
//! Defines `ServiceError` enum covering all failure modes (auth, DB, signing,
//! replay detection) with mapped HTTP status codes and structured JSON error
//! envelopes for API consumers.

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("no token provided in request body or Authorization header")]
    MissingToken,

    #[error("token is not a valid JWT structure")]
    MalformedToken,

    #[error("JWT signature verification failed")]
    InvalidSignature,

    #[error("JWT has expired")]
    ExpiredToken,

    #[error("JWT issuer does not match configured IdP issuer")]
    InvalidIssuer,

    #[error("JWT audience does not match expected audience")]
    InvalidAudience,

    #[error("key ID not found in JWKS")]
    UnknownKid,

    #[error("token has already been used (replay detected)")]
    ReplayDetected {
        inbound_sub: String,
        inbound_iss: Option<String>,
        inbound_aud: Option<String>,
        replay_id: String,
    },

    #[error("failed to fetch IdP configuration or JWKS")]
    IdPUnavailable,

    #[error("public certificate not loaded")]
    CertNotLoaded,

    #[error("internal server error")]
    InternalError,

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JWKS error: {0}")]
    Jwks(String),

    #[error("signing error: {0}")]
    Signing(String),

    #[error("other error: {0}")]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDetail {
    pub field: String,
    pub message: String,
    pub code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEnvelope {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ErrorDetail>>,
    pub request_id: String,
    pub timestamp: String,
}

impl ServiceError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::MissingToken => "missing_token",
            Self::MalformedToken => "malformed_token",
            Self::InvalidSignature => "invalid_signature",
            Self::ExpiredToken => "expired_token",
            Self::InvalidIssuer => "invalid_issuer",
            Self::InvalidAudience => "invalid_audience",
            Self::UnknownKid => "unknown_kid",
            Self::ReplayDetected { .. } => "replay_detected",
            Self::IdPUnavailable => "idp_unavailable",
            Self::CertNotLoaded => "cert_not_loaded",
            Self::InternalError => "internal_error",
            Self::Database(_) => "internal_error",
            Self::Jwks(_) => "internal_error",
            Self::Signing(_) => "internal_error",
            Self::Other(_) => "internal_error",
        }
    }

    pub fn to_envelope(&self, request_id: &str) -> ErrorEnvelope {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        ErrorEnvelope {
            error: self.error_code().to_string(),
            message: self.to_string(),
            detail: match self {
                Self::UnknownKid => Some("key ID not found in JWKS after refresh".to_string()),
                Self::InvalidSignature => Some("signature did not match any JWKS key".to_string()),
                Self::ExpiredToken => Some("token exp claim is in the past".to_string()),
                Self::InvalidIssuer => Some("iss claim does not match configured IdP issuer".to_string()),
                Self::InvalidAudience => {
                    Some("aud claim does not match expected audience".to_string())
                }
                _ => None,
            },
            details: None,
            request_id: request_id.to_string(),
            timestamp: now,
        }
    }

    /// Build an error response with a specific request ID.
    pub fn to_error_response(&self, request_id: &str) -> HttpResponse {
        let envelope = self.to_envelope(request_id);
        HttpResponse::build(self.status_code())
            .insert_header(("X-Request-Id", request_id.to_string()))
            .json(envelope)
    }
}

impl actix_web::ResponseError for ServiceError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::MissingToken | Self::MalformedToken => StatusCode::UNPROCESSABLE_ENTITY,
            Self::InvalidSignature
            | Self::ExpiredToken
            | Self::InvalidIssuer
            | Self::InvalidAudience
            | Self::UnknownKid
            | Self::ReplayDetected { .. } => StatusCode::UNAUTHORIZED,
            Self::IdPUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::CertNotLoaded => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InternalError
            | Self::Database(_)
            | Self::Jwks(_)
            | Self::Signing(_)
            | Self::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        // F18: The middleware intercepts this and replaces the request_id
        // with the real one. We use a placeholder here.
        let envelope = self.to_envelope("-");
        HttpResponse::build(self.status_code()).json(envelope)
    }
}
