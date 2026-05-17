# Code Conventions — Rust

**Status:** Living document.
**Audience:** Anyone writing Rust code in this project — humans and AI assistants generating code.
**Scope:** Rust style, project layout, type system usage, error handling, persistence, async, testing.
**Companion docs:** Other languages have their own `CODE_CONVENTIONS-<language>.md`. For naming of files and folders, see `NAMING_CONVENTIONS.md`. For HTTP API and JSON wire format conventions, see `API_CONVENTIONS.md`.

When this document and the official Rust style guide (`rustfmt`'s defaults plus the API guidelines) conflict, this document wins (this is rare). When this document is silent, follow `rustfmt`, the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/), and Clippy with `--all-targets --all-features -D warnings`.

---

## 1. Style basics

### 1.1 Naming

- **Modules and files:** `snake_case` (`service_engine.rs`, `runtime_adapter.rs`).
- **Functions and methods:** `snake_case` (`acquire_lock`, `provision`).
- **Variables:** `snake_case` (`item_key`, `request_id`).
- **Types (structs, enums, traits, type aliases):** `PascalCase` (`ServiceEngine`, `Request`, `RuntimeAdapter`).
- **Enum variants:** `PascalCase` (`Status::PendingReview`, `Status::Approved`).
- **Constants and statics:** `SCREAMING_SNAKE_CASE` (`DEFAULT_TTL_SECONDS`, `MAX_BUDGET_USD`).
- **Lifetimes:** short, lowercase (`'a`, `'src`, `'ctx`). Avoid one-letter lifetimes longer than `'a`/`'b`/`'c`; prefer descriptive names for any lifetime that escapes a single function.
- **Generic type parameters:** single uppercase letter for simple cases (`T`, `E`), `PascalCase` descriptive name for complex cases (`Provider`, `Outcome`).

### 1.2 Edition and toolchain

- **Edition:** `edition = "2021"` minimum. Bump as new editions stabilize.
- **MSRV (Minimum Supported Rust Version):** declared in `Cargo.toml` via `rust-version = "1.X"`. Pick the lowest version that supports features you use. Don't track stable blindly.
- **`rustfmt`:** required. Configuration in `rustfmt.toml` only when defaults need overriding (rare). The defaults are the conventions.
- **`clippy`:** required. Run with `-D warnings`. Don't `#[allow(clippy::...)]` without a comment explaining why.

### 1.3 Indentation and line length

- 4 spaces (rustfmt default; never tabs).
- 100-character line limit (rustfmt default).
- Don't fight rustfmt. If a formatted line reads badly, the code structure usually needs changing, not the formatter.

### 1.4 Imports

Order is enforced by rustfmt's `group_imports = "StdExternalCrate"` setting:

1. `std`, `core`, `alloc`.
2. External crates (anything from `Cargo.toml` `[dependencies]`).
3. Local items (`crate::`, `super::`, `self::`).

Each group separated by a blank line. Within a group, alphabetize.

```rust
use std::sync::Arc;
use std::time::Duration;

use actix_web::{web, HttpResponse, Result as ActixResult};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::ServiceError;
use crate::services::item_service;
```

Use `use crate::path::{A, B, C}` to group related imports rather than four separate lines. Don't use glob imports (`use foo::*`) except for prelude modules and the test module pattern (`use super::*` in `#[cfg(test)] mod tests`).

### 1.5 String formatting

`format!`, `println!`, `write!` with named captures (Rust 1.58+):

```rust
// Good
let msg = format!("Acquired lock for {item_key}");
log::info!("Acquired lock for {item_key}");

// Acceptable, older style
let msg = format!("Acquired lock for {}", item_key);

// Bad — manual concatenation
let msg = "Acquired lock for ".to_string() + &item_key;
```

For SQL with sqlx, **never** use `format!` to interpolate values into queries. Always use the `query!`/`query_as!` macros or the bind API:

```rust
// Good — compile-time-checked, parameterized
let row = sqlx::query!("SELECT * FROM items WHERE id = $1", item_id)
    .fetch_one(&pool)
    .await?;

// Bad — SQL injection vector
let q = format!("SELECT * FROM items WHERE id = '{item_id}'");
sqlx::query(&q).fetch_one(&pool).await?;
```

---

## 2. Type system

Rust's type system is a tool. Use it.

### 2.1 Newtype pattern for domain identifiers

Don't pass `String` everywhere. Wrap domain identifiers in newtypes so the compiler catches mix-ups:

```rust
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct ItemId(String);

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct UserId(String);

impl ItemId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str { &self.0 }
}
```

Now `fn assign(item: ItemId, owner: UserId)` cannot be called with the arguments swapped.

### 2.2 Enums for state

If a value can be in one of several distinct states, an enum with variants is almost always better than a struct with optional fields:

```rust
// Good — invalid states unrepresentable
pub enum ItemStatus {
    Draft,
    PendingReview { submitted_at: DateTime<Utc> },
    Approved { approved_by: UserId, approved_at: DateTime<Utc> },
    Archived { reason: String },
}

// Bad — every consumer has to check which fields are populated
pub struct ItemStatus {
    pub state: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub approved_by: Option<UserId>,
    pub approved_at: Option<DateTime<Utc>>,
    pub archive_reason: Option<String>,
}
```

### 2.3 Builders for construction

For types with many optional construction parameters, use a builder:

```rust
let request = RequestBuilder::new(ItemId::new("abc-123"))
    .with_owner(UserId::new("usr_xyz"))
    .with_priority(Priority::High)
    .build()?;
```

Don't reach for the builder reflexively — for 2–3 fields, struct literal syntax is fine. For 6+, a builder pays for itself. Consider `derive_builder` or `bon` rather than hand-rolling.

### 2.4 Lifetimes

Annotate lifetimes only when the borrow checker requires it. The most common case is returning a reference tied to an input:

```rust
fn first_word<'a>(s: &'a str) -> &'a str { ... }
```

Don't annotate lifetimes "for clarity" if elision would have done the same job. The elided form is the convention.

For data structures that hold references, prefer owning the data (`String`, `Vec<T>`) over borrowing (`&str`, `&[T]`) unless there's a measured performance reason. Borrowing in structs forces lifetime annotations to leak through every consumer.

### 2.5 `Result` everywhere; never panic in library code

Every fallible operation returns `Result<T, E>`. Use `?` for propagation:

```rust
pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let contents = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&contents)?;
    config.validate()?;
    Ok(config)
}
```

`unwrap()` and `expect()` are acceptable in:
- Tests.
- Build scripts.
- `main()` of a CLI binary, when followed by an error message and process exit.
- Cases where you can prove the value cannot be `None`/`Err` (and a comment explains why).

Library code never panics on caller-controllable input. A panic in library code is a bug.

### 2.6 Avoid `Box<dyn Trait>` unless you need it

Generics monomorphize and stay zero-cost; trait objects allocate and have vtable overhead. Reach for `impl Trait` first, then generic type parameters, and only use `Box<dyn Trait>` when you genuinely need heterogeneous storage at runtime (e.g., a `Vec<Box<dyn Plugin>>`).

---

## 3. Project shape

The project's shape determines its layout. Three shapes are common; the principle is the same in each — **separate the layers, talk only downward**.

### 3.1 Service shape (HTTP API + persistence)

Stack: **Tokio + actix-web + sqlx**.

```
src/
  main.rs              ← Binary entry. Constructs app, starts the server.
  app.rs               ← `HttpServer` configuration; route mounting.
  handlers/            ← HTTP handlers (extractors → services). Thin.
    items.rs
    users.rs
    mod.rs
  services/            ← Business logic. Stateless. Calls db.
    item_service.rs
    mod.rs
  db/                  ← sqlx pool, queries, migrations module.
    pool.rs
    items.rs
    mod.rs
  models/              ← API request/response DTOs (serde).
    items.rs
    mod.rs
  domain/              ← Domain types (newtypes, enums, value objects).
    mod.rs
  error.rs             ← Application error type + ResponseError impl.
  config.rs            ← Configuration loaded from env/file.
  lib.rs               ← Re-exports for testing; can be empty for binary-only.
migrations/            ← sqlx migration files.
```

`main.rs` stays minimal — load config, build the app, run it. All mounting and assembly lives in `app.rs`.

### 3.2 CLI shape (command-line tool)

Stack: **clap** (derive API) for argument parsing, **anyhow** for ergonomic errors at the binary boundary.

```
src/
  main.rs              ← Binary entry. Calls cli::run().
  cli.rs               ← clap-derived `Args`/`Command`; dispatch to commands.
  commands/            ← One module per subcommand.
    init.rs
    run.rs
    status.rs
    mod.rs
  services/            ← Business logic. Called by commands.
    mod.rs
  io.rs                ← stdout/stderr formatting helpers.
  error.rs             ← Application error type.
```

`main.rs` is typically:

```rust
fn main() -> anyhow::Result<()> {
    env_logger::init();
    cli::run()
}
```

Use clap's derive API (`#[derive(Parser)]`) — it generates `--help` for free and keeps argument definitions next to their handler types. Avoid hand-rolling `std::env::args()` parsing.

### 3.3 Library shape (crate consumed by other code)

```
src/
  lib.rs               ← Public API surface. Re-exports + crate-level docs.
  models.rs            ← Public types.
  error.rs             ← Public error type.
  internal/            ← Private modules. NOT re-exported from lib.rs.
    mod.rs
  ...
```

Public API discipline:

- Everything reachable from `lib.rs` via `pub` is part of the public API.
- Anything that should not leak is `pub(crate)` or unmarked (private).
- `lib.rs` has crate-level rustdoc (`//!`) describing the crate's purpose and a minimal usage example.
- Use `#[non_exhaustive]` on public enums and structs that may grow new variants/fields, so adding one isn't a breaking change for consumers.
- Avoid leaking external crate types in your public API unless you re-export them — this couples your consumers to your dependency versions.

The library shape's contract with consumers is the public surface. Removing or renaming a public item is a breaking change in the sense of `../designs/design-000-meta.md` §3 (and also Cargo's semver rules — see [SemVer Compatibility](https://doc.rust-lang.org/cargo/reference/semver.html)).

### 3.4 The shared layered principle

Whatever the shape, layers talk only downward:

- **Handlers / Commands / Public-API** — entry points. Thin. Validate input, dispatch, format output.
- **Services** — business logic. No knowledge of the entry layer.
- **Storage / IO** — persistence and external IO. Called by services.

Skipping layers (a handler running raw SQL, a CLI command opening a TCP socket directly) is the smell. Push the work into the right layer.

### 3.5 Handlers (service shape)

Handlers are thin. They:
- Accept extractors (`web::Path`, `web::Json`, `web::Data<PgPool>`).
- Delegate to a service function.
- Map service errors to HTTP responses (via the `ResponseError` impl on the error type).
- Return `Result<HttpResponse, ServiceError>`.

Handlers do **not**:
- Call `sqlx::query!` directly.
- Contain multi-step business logic.
- Maintain state outside the request scope.

If a handler is more than ~30 lines, it's probably doing too much; extract logic into a service function.

```rust
#[post("/items")]
pub async fn create_item(
    pool: web::Data<PgPool>,
    body: web::Json<ItemCreate>,
) -> Result<HttpResponse, ServiceError> {
    let item = item_service::create_item(&pool, body.into_inner()).await?;
    Ok(HttpResponse::Created().json(ItemResponse::from(item)))
}
```

### 3.6 Services

Service functions are the business logic. Each function:
- Takes typed parameters (including `&PgPool` or `&mut Transaction<'_, Postgres>` for db access).
- Returns `Result<T, ServiceError>` (or a service-local error type).
- Has no awareness of HTTP, clap, or any entry-layer concept.

Services do **not**:
- Import from `handlers/` or `commands/`.
- Maintain global mutable state.
- Form import cycles.

```rust
pub async fn create_item(
    pool: &PgPool,
    request: ItemCreate,
) -> Result<Item, ServiceError> {
    let id = ItemId::new(uuid::Uuid::new_v4().to_string());
    db::items::insert(pool, &id, &request).await?;
    Ok(db::items::fetch_by_id(pool, &id).await?)
}
```

### 3.7 Database layer (service shape)

`db/pool.rs` constructs the `PgPool` (or `SqlitePool`) once at startup. It's passed via `web::Data<PgPool>` for actix-web.

Query modules in `db/` own the SQL. Use the `query!` and `query_as!` macros for compile-time SQL validation against the live database (set `DATABASE_URL` for builds, or use `cargo sqlx prepare` to vendor query metadata for offline builds).

```rust
pub async fn fetch_by_id(pool: &PgPool, id: &ItemId) -> Result<Item, sqlx::Error> {
    sqlx::query_as!(
        Item,
        r#"SELECT id, key, status as "status: ItemStatus", created_at FROM items WHERE id = $1"#,
        id.as_str()
    )
    .fetch_one(pool)
    .await
}
```

### 3.8 No SQL in handlers

Handlers never call `sqlx::query!` directly. SQL belongs in `db/`. If a handler needs data, it calls a service function that returns the data.

There are no exceptions. Inline SQL in handlers is bootstrap-residue (see `DESIGN_PHILOSOPHY.md`); don't ship it.

---

## 4. Error handling

### 4.1 Use `thiserror` for library and service errors

Define a single application error type with `thiserror`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("item with key '{0}' already exists")]
    DuplicateItemKey(String),

    #[error("invalid status transition from {from:?} to {to:?}")]
    InvalidStatusTransition { from: ItemStatus, to: ItemStatus },

    #[error("item not found: {0}")]
    NotFound(String),

    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

`#[from]` generates the `From` impl that powers the `?` operator. `#[error(transparent)]` delegates the message to the wrapped error.

For HTTP service shape, implement `actix_web::ResponseError`:

```rust
impl actix_web::ResponseError for ServiceError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::DuplicateItemKey(_) => StatusCode::CONFLICT,
            Self::InvalidStatusTransition { .. } => StatusCode::CONFLICT,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Database(_) | Self::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        // Build the error envelope per API_CONVENTIONS.md §6
        ...
    }
}
```

### 4.2 Use `anyhow` at the binary boundary

CLI binaries and integration test helpers can use `anyhow::Result<T>` for ergonomic error propagation when the caller doesn't need to branch on the error variant:

```rust
fn main() -> anyhow::Result<()> {
    let config = load_config().context("failed to load config")?;
    run(config).context("service exited with error")?;
    Ok(())
}
```

`.context(...)` adds a chain of human-readable context that prints with `{:?}` formatting. Use it liberally in `main` and CLI command handlers.

**Don't use `anyhow` in library code** that other crates depend on. Library consumers want typed errors they can match on; `anyhow::Error` is opaque.

### 4.3 Don't catch and re-throw without value

```rust
// Bad — adds nothing
match do_thing() {
    Ok(x) => Ok(x),
    Err(e) => Err(e),
}

// Bad — silently swallows
let _ = do_thing();

// Good — converts to a typed error with context
do_thing().map_err(|e| ServiceError::Other(anyhow::anyhow!("do_thing failed: {e}")))?;
```

### 4.4 Don't use `Result` for routine validity checks

```rust
// Bad — wrong tool for the job
let parsed: Result<ItemId, _> = ItemId::parse(input);
if parsed.is_err() {
    return Ok(default_id());
}

// Good — ask the question directly
let id = match ItemId::try_parse(input) {
    Some(id) => id,
    None => default_id(),
};
```

`Result` is for fallible operations; `Option` is for "value may not exist."

---

## 5. Persistence (sqlx)

### 5.1 Use the macros for compile-time checking

`sqlx::query!`, `query_as!`, `query_scalar!` validate SQL at compile time against a live database (or vendored metadata via `cargo sqlx prepare`). Use them.

`sqlx::query()` (no `!`) is the runtime-checked fallback. Use it only for genuinely dynamic SQL (e.g., `ORDER BY` column chosen at runtime), and document why.

### 5.2 Pools, not raw connections

```rust
let pool = sqlx::postgres::PgPoolOptions::new()
    .max_connections(10)
    .connect(&database_url)
    .await?;
```

Pass `&PgPool` (or `&SqlitePool`) into service and db-layer functions. Don't construct ad-hoc connections.

### 5.3 Transactions are explicit

```rust
let mut tx = pool.begin().await?;

sqlx::query!("INSERT INTO items (...) VALUES (...)", ...)
    .execute(&mut *tx)
    .await?;

sqlx::query!("INSERT INTO item_versions (...) VALUES (...)", ...)
    .execute(&mut *tx)
    .await?;

tx.commit().await?;
```

If `tx` is dropped without `commit()`, sqlx rolls back automatically.

### 5.4 Migrations

Use `sqlx::migrate!()` at startup to apply migrations from `migrations/`:

```rust
sqlx::migrate!("./migrations").run(&pool).await?;
```

Migration files: `<timestamp>_<description>.sql`. The timestamp ordering matters; don't rely on alphabetical sort of the description.

---

## 6. Async

The default is **async** (because actix-web and sqlx are both async). Sync is the exception.

### 6.1 Tokio runtime

actix-web 4+ runs on Tokio. There's only one runtime in the process; don't mix Tokio with async-std.

For binaries, use `#[actix_web::main]` (which is `tokio::main` under the hood):

```rust
#[actix_web::main]
async fn main() -> std::io::Result<()> { ... }
```

For CLI tools that don't use actix-web, use `#[tokio::main]`:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> { ... }
```

### 6.2 Don't block the runtime

Long sync work (heavy CPU, blocking IO) inside an async function blocks the executor thread. Wrap it in `spawn_blocking`:

```rust
let result = tokio::task::spawn_blocking(move || expensive_sync_work(input))
    .await?;
```

This is the most common async pitfall in services. The compiler doesn't catch it; you have to know.

### 6.3 `Send` requirements

Most async code in services needs to be `Send` (work-stealing executors move futures across threads). If you hit a `not Send` error, the usual causes are:
- Holding a `Rc<T>` or `RefCell<T>` across an `.await`.
- Holding a `MutexGuard` across an `.await` (use `tokio::sync::Mutex` or restructure).

Restructure to drop the non-`Send` value before the `.await`.

### 6.4 Sync code is allowed

CLI tools that don't need concurrency, scripts, and library functions called only from sync contexts don't need to be async. Async has costs (compile time, debug overhead, ecosystem fragmentation); use it where it pays.

---

## 7. Testing

### 7.1 Layout

```
src/                   ← #[cfg(test)] mod tests at the bottom of source files (unit tests)
tests/                 ← Integration tests; one file per concern
  api_items.rs
  service_locking.rs
  schema.rs
```

Unit tests inside `src/` test private functions and have access to `super::*`. Integration tests in `tests/` test the public API as an external consumer would.

### 7.2 Async tests

```rust
#[tokio::test]
async fn acquire_lock_succeeds_for_unheld_item_key() { ... }

#[actix_web::test]
async fn create_item_returns_201() { ... }   // for actix handler tests
```

### 7.3 sqlx tests

`sqlx::test` provides a fresh per-test database (with migrations applied):

```rust
#[sqlx::test]
async fn insert_item_persists(pool: PgPool) {
    let id = ItemId::new("abc-123");
    db::items::insert(&pool, &id, &request).await.unwrap();

    let fetched = db::items::fetch_by_id(&pool, &id).await.unwrap();
    assert_eq!(fetched.key, "abc-123");
}
```

### 7.4 Test naming

Test names describe the behavior:

```rust
fn acquire_lock_succeeds_for_unheld_item_key() { ... }
fn acquire_lock_fails_when_already_held() { ... }
fn acquire_lock_succeeds_when_existing_lock_is_stale() { ... }
```

Not `test_lock_1`, `test_lock`, `TestAcquireLockHappyPath`.

### 7.5 Error path coverage

Every happy path needs at least two error path tests covering:
- Bad input (validation failure).
- Resource conflict (duplicate, lock held, invalid transition).

This is a hard rule. Error-path gaps are defects.

---

## 8. Documentation

### 8.1 Module and crate docs

Every public module starts with a `//!` doc comment:

```rust
//! Item service — business logic layer.
//!
//! Handles item creation, lookup, status transitions, and lock management.
//! Layer: services. Imports from db only. No HTTP knowledge.
```

`lib.rs` of any library crate gets crate-level docs (`//!`) describing what the crate is, with a minimal example.

### 8.2 Public item docs

Public functions, types, and traits get `///` doc comments:

```rust
/// Attempt to acquire a creation lock.
///
/// Returns `Ok(true)` if acquired, `Ok(false)` if already held by another holder.
/// Stale locks (past `expires_at`) are automatically force-released and acquired.
///
/// # Errors
///
/// Returns `ServiceError::Database` if the lock table cannot be reached.
pub async fn acquire_lock(
    pool: &PgPool,
    item_key: &ItemId,
    locked_by: &str,
    ttl: Duration,
) -> Result<bool, ServiceError> { ... }
```

For library crates, `cargo doc --no-deps` should build clean (no warnings about missing docs). Add `#![warn(missing_docs)]` at the top of `lib.rs` to enforce this.

Doc examples are tested by `cargo test`. Use them for non-trivial APIs:

```rust
/// # Examples
///
/// ```
/// use mycrate::ItemId;
/// let id = ItemId::new("abc-123");
/// assert_eq!(id.as_str(), "abc-123");
/// ```
```

### 8.3 Inline comments

Comments explain **why**, not **what**:

```rust
// Bad — restates the code
i += 1;  // increment i

// Good — explains the why
// Off-by-one: indices in this dataset are 1-based per the upstream spec.
i += 1;
```

If the code itself isn't self-explanatory, often the right fix is renaming a variable or extracting a function, not adding a comment.

---

## 9. Cargo, features, and workspace

### 9.1 `Cargo.toml`

```toml
[package]
name = "myproject"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "MIT"
description = "Short one-liner."
repository = "..."

[dependencies]
# group by purpose with comments
tokio = { version = "1", features = ["full"] }
actix-web = "4"
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "macros", "migrate"] }

# error handling
thiserror = "1"
anyhow = "1"

# serde
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Group dependencies by purpose with comments. Pin to major versions only (`"1"`, not `"1.2.3"`) unless you have a specific reason.

### 9.2 Features

Use features for optional functionality, not for "configuration":

```toml
[features]
default = []
metrics = ["dep:prometheus"]
```

Don't use features to gate "production" vs "development" behavior — use config or environment instead.

### 9.3 Workspaces

For projects with multiple related crates (a service + a shared library + a CLI), use a Cargo workspace:

```
Cargo.toml          ← workspace root
crates/
  service/
    Cargo.toml
    src/
  shared/
    Cargo.toml
    src/
  cli/
    Cargo.toml
    src/
```

Workspace `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.dependencies]
# shared dep versions go here; member crates reference via `dep.workspace = true`
serde = { version = "1", features = ["derive"] }
```

---

## 10. Why conventions matter even in a single project

It's tempting to think conventions only matter when multiple people or systems collaborate. They matter inside a single project too:

- **AI assistants generate code that matches the conventions of the code they see.** Drifty conventions produce drifty generated code. Tight conventions produce tight generated code.
- **`rustfmt` and `clippy` are part of the conventions.** Run them in CI; treat warnings as errors. Catching style drift early is cheaper than a rewrite later.
- **Future-you is a different collaborator.** Code you'll be reading in six months was written by someone with different context. Conventions are the shared assumptions that let future-you understand what you wrote.

Reference these conventions in design docs rather than restating them. When they need to change, update the doc and any in-flight designs that depend on them.

---

## 11. Cross-references

- For naming of files and folders (including `.rs` modules and migration files): `NAMING_CONVENTIONS.md`.
- For HTTP API and JSON wire format conventions (including `serde` rename attributes for camelCase wire format): `API_CONVENTIONS.md`.
- For other languages: `CODE_CONVENTIONS-<language>.md`.
- For the design values that motivate code-level discipline: `../DESIGN_PHILOSOPHY.md` (especially "Conventions are constitutional" and "Bootstrap-residue is a smell").
- For the structural foundation that governs design docs and contracts: `../designs/design-000-meta.md`.
- For the official Rust API guidelines: <https://rust-lang.github.io/api-guidelines/>.
- For Cargo SemVer compatibility rules: <https://doc.rust-lang.org/cargo/reference/semver.html>.
