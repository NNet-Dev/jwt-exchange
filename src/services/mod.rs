//! Business logic services.
//!
//! - **token_service**: Core token exchange — validates inbound JWTs,
//!   checks replay protection, filters groups, and mints downstream tokens.
//! - **audit_service**: Fire-and-forget audit logging to SQLite and Splunk.

pub mod audit_service;
pub mod token_service;
