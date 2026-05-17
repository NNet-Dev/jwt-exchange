//! Database access layer.
//!
//! - **pool**: SQLite connection management with WAL mode and graceful shutdown.
//! - **audit**: Audit log insertion, JTI replay protection, and background purge tasks.

pub mod audit;
pub mod pool;
