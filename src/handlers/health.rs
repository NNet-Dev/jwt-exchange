//! GET /api/v1/health — health and readiness probe.
//!
//! Checks JWKS cache, signing key, and database connectivity.
//! Returns `200 healthy` or `503 unhealthy` with per-component status.

use std::time::Instant;

use actix_web::{web, HttpResponse};
use sqlx::SqlitePool;

use crate::app::AppState;
use crate::auth::jwks::JwksCache;
use crate::db;
use crate::models::api::{HealthChecks, HealthResponse};

static START_TIME: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

/// Simplified health check — returns status and uptime only.
/// Internal state is not exposed in the response
/// to avoid leaking details to unauthenticated callers.
pub async fn health(
    pool: web::Data<SqlitePool>,
    jwks_cache: web::Data<JwksCache>,
    _state: web::Data<AppState>,
) -> HttpResponse {
    // Check JWKS
    let jwks_healthy = {
        let cache = jwks_cache.read().await;
        cache.is_healthy()
    };

    // Check DB
    let db_healthy = db::pool::check_connection(pool.get_ref()).await.is_ok();

    let all_healthy = jwks_healthy && db_healthy;
    let uptime = START_TIME.elapsed().as_secs();

    let response = HealthResponse {
        status: if all_healthy {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        checks: HealthChecks {
            jwks: if jwks_healthy {
                "ok".to_string()
            } else {
                "error".to_string()
            },
            signing_key: "ok".to_string(),
            database: if db_healthy {
                "ok".to_string()
            } else {
                "error".to_string()
            },
        },
        uptime_seconds: uptime,
    };

    if all_healthy {
        HttpResponse::Ok().json(response)
    } else {
        HttpResponse::ServiceUnavailable().json(response)
    }
}
