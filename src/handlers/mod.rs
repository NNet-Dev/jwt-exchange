//! HTTP request handlers.
//!
//! - **exchange**: Token exchange endpoint (`POST /api/v1/exchange`).
//! - **cert**: Public certificate endpoint (`GET /api/v1/cert`).
//! - **health**: Health and readiness probe (`GET /api/v1/health`).

pub mod cert;
pub mod exchange;
pub mod health;
