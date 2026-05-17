//! Authentication and key management.
//!
//! - **jwks**: JWKS fetching, caching with ETag support, force-refresh with
//!   cooldown and circuit-breaker, background refresh task.
//! - **signing**: RSA key pair resolution — loads existing PEM files or
//!   auto-generates a self-signed RSA-2048 key pair on first run.

pub mod jwks;
pub mod signing;
