//! JWT Exchange — IdP-to-downstream token exchange service.
//!
//! Validates incoming IdP-issued JWTs (e.g. Okta, Auth0), extracts identity
//! claims, and mints new RSA-signed JWTs tailored for a downstream service.
//!
//! ## Architecture
//! - **HTTP server**: actix-web with request ID middleware
//! - **Auth**: JWKS caching with force-refresh + circuit breaker
//! - **Signing**: RSA-256 with auto-generated or loaded key pairs
//! - **Audit**: SQLite with 60-day auto-purge and JTI replay protection
//! - **Export**: Streaming Splunk HEC exporter (optional)

use jwt_exchange::app::bootstrap;
use jwt_exchange::config::AppConfig;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jwt_exchange=info".parse()?),
        )
        .init();

    let config = AppConfig::from_env().map_err(|e| anyhow::anyhow!("configuration error: {e}"))?;

    let booted = bootstrap(config).await?;
    booted.server.await?;
    Ok(())
}
