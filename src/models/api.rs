//! API request and response schemas.
//!
//! Maps to the OpenAPI specification for the exchange endpoint and health check.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRequest {
    pub token: Option<String>,
    #[serde(default)]
    pub groups: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub issued_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub checks: HealthChecks,
    pub uptime_seconds: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthChecks {
    pub jwks: String,
    pub signing_key: String,
    pub database: String,
}
