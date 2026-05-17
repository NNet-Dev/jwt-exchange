//! External service logging and SIEM integration.
//!
//! - **splunk**: Streaming Splunk HEC exporter with batching, retry, and
//!   TLS verification controls. Converts audit records to truncated events
//!   that exclude raw tokens and full JTI values.

pub mod splunk;
