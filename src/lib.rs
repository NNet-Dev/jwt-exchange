//! JWT Exchange — IdP-to-downstream token exchange service.
//!
//! Validates incoming IdP-issued JWTs (e.g. Okta, Auth0), extracts identity
//! claims, and mints new RSA-signed JWTs tailored for a downstream service.

pub mod app;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod handlers;
pub mod logging;
pub mod middleware;
pub mod models;
pub mod services;
