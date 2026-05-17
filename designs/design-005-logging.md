---
app: jwt-exchange
owner: Marc
status: Active
supersedes: []
depends_on: [design-001-init.md]
owns: [database]
---

# Design 005 — Logging

---

## Purpose

Define the request audit logging system: SQLite storage with configurable auto-purge, JTI replay protection, and a streaming Splunk HEC export for external log aggregation.

---

## Why logging matters

Every token exchange is a security-relevant event. We need to know:
- Who exchanged a token (inbound identity provider subject)
- When it happened
- Whether validation succeeded or failed
- What the outcome was (token minted, or rejected and why)

SQLite is chosen for simplicity — no external database dependency, embedded in the service, file-based persistence.

---

## SQLite schema

### request_log

```sql
CREATE TABLE IF NOT EXISTS request_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp       TEXT NOT NULL DEFAULT (datetime('now')),
    source_ip       TEXT NOT NULL,
    inbound_sub     TEXT,                     -- NULL if validation failed before claim extraction
    inbound_iss     TEXT,                     -- extracted issuer from incoming JWT
    inbound_aud     TEXT,                     -- extracted audience from incoming JWT
    validation      TEXT NOT NULL,            -- 'success' | 'replay_detected' | 'expired' | 'invalid_sig' | 'unknown_kid' | 'bad_iss' | 'bad_aud' | 'malformed'
    error_detail    TEXT,                     -- human-readable error context (NULL on success)
    outbound_sub    TEXT,                     -- subject in the minted JWT (NULL if not minted)
    token_jti       TEXT,                     -- JTI of the minted JWT (NULL if not minted)
    response_code   INTEGER NOT NULL,         -- HTTP response code returned
    elapsed_ms      INTEGER NOT NULL,         -- request processing time in milliseconds
    exported_to_splunk INTEGER NOT NULL DEFAULT 0  -- 0 = pending, 1 = exported
);

CREATE INDEX IF NOT EXISTS idx_request_log_timestamp ON request_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_request_log_inbound_sub ON request_log(inbound_sub);
CREATE INDEX IF NOT EXISTS idx_request_log_exported ON request_log(exported_to_splunk, timestamp);
```

### used_jti (replay protection)

```sql
CREATE TABLE IF NOT EXISTS used_jti (
    jti         TEXT NOT NULL,              -- JWT ID or SHA-256 hash of raw token
    has_groups  INTEGER NOT NULL DEFAULT 0, -- 0 = no groups, 1 = with groups
    exp         INTEGER NOT NULL,           -- inbound token expiry (Unix epoch)
    PRIMARY KEY (jti, has_groups)
);

CREATE INDEX IF NOT EXISTS idx_used_jti_exp ON used_jti(exp);
```

### schema_version

```sql
CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO schema_version (version) VALUES (1);
```

### Field rationale

- **`timestamp`** — ISO 8601 datetime string. Indexed for range queries and purge.
- **`source_ip`** — extracted from the request (X-Forwarded-For if behind a proxy, otherwise the direct connection IP).
- **`inbound_sub`** — the subject from the incoming identity provider JWT. NULL if we couldn't parse or validate the token at all (e.g., malformed JWT).
- **`validation`** — categorical outcome. `'success'` means the token passed all cryptographic checks (signature, issuer, expiry). `'replay_detected'` means the token was valid but had already been consumed. Enables quick aggregation: `SELECT validation, COUNT(*) FROM request_log GROUP BY validation`.
- **`error_detail`** — for failed validations, captures the specific reason (e.g., "token has already been used (replay detected)", "unknown kid abc123").
- **`outbound_sub` / `token_jti`** — only populated on successful exchange. Links the outbound token to the inbound request for audit trails.
- **`elapsed_ms`** — end-to-end request processing time. Useful for performance monitoring.
- **`exported_to_splunk`** — tracks whether the row has been streamed to Splunk. Used for monitoring export lag, not for purge gating.
- **`used_jti.jti`** — the JWT ID from the inbound token, or SHA-256 hash if no `jti` claim exists.
- **`used_jti.has_groups`** — distinguishes between group and non-group exchanges for the optional replay mode (`ALLOW_REPLAY=true`).

---

## Auto-purge

### Mechanism

Two background tasks run on configured intervals:

**Audit log purge** (default: every 6 hours):
```sql
DELETE FROM request_log
WHERE timestamp < datetime('now', '-{LOG_RETENTION_DAYS} days');
```

**JTI expiry cleanup** (default: every 1 hour):
```sql
DELETE FROM used_jti
WHERE exp < strftime('%s', 'now');
```

### Configuration

| Key | Default | Description |
|---|---|---|
| `LOG_RETENTION_DAYS` | `60` | Days to retain request logs |

### Design notes

- Purge runs **after** the log row is written — it never interferes with the current request.
- Purge is based on timestamp only, not on Splunk export status. If Splunk is down for longer than the retention period, unexported rows will be lost. This is an acceptable trade-off — Splunk availability is monitored separately.
- JTI cleanup is based on the inbound token's `exp` claim, not the current time. This ensures JTIs are retained long enough to catch replays within the token's validity window.

---

## Splunk HEC export stream

### Mechanism

The Splunk exporter uses a streaming channel-based architecture, not a polling model:

```
Audit service ──mpsc channel──▶ Export task ──HTTP POST──▶ Splunk HEC
```

### Flow

1. On startup, start the export task as an async Tokio task with an `mpsc` channel receiver.
2. Each audit log write also sends a copy of the event through the channel.
3. The export task buffers incoming events and flushes in batches to the Splunk HEC endpoint.
4. On success, the event is considered delivered (no ack back to the writer).
5. On failure, the task retries up to 3 times with exponential backoff, then drops the batch and logs the error.
6. The `exported_to_splunk` column in `request_log` is updated asynchronously by the export task (used for monitoring, not for reliability).

### Splunk HEC event shape

```json
{
  "time": 1715947200.000,
  "host": "jwt-exchange-host",
  "source": "jwt-exchange",
  "sourcetype": "jwt-exchange:audit",
  "event": {
    "timestamp": "2026-05-17T12:00:00",
    "source_ip": "10.0.1.5",
    "inbound_sub": "00u1abc2def3ghi4jkl5",
    "inbound_iss": "https://<tenant>.okta.com",
    "inbound_aud": "api://default",
    "validation": "success",
    "error_detail": null,
    "outbound_sub": "00u1abc2def3ghi4jkl5",
    "token_jti": "a1b2c3d4",
    "response_code": 200,
    "elapsed_ms": 42
  }
}
```

**Security note:** The `token_jti` field is truncated to the first 8 hex characters in Splunk events to prevent full token ID exposure.

### Configuration

| Key | Required | Default | Description |
|---|---|---|---|
| `SPLUNK_HEC_URL` | No | _(disabled)_ | Splunk HEC endpoint (e.g. `https://splunk:8088/services/collector`) |
| `SPLUNK_HEC_TOKEN` | No | _(disabled)_ | Splunk HEC authentication token |
| `SPLUNK_HEC_SKIP_TLS_VERIFY` | No | `false` | Disable TLS verification for Splunk |

### Resilience

- **Splunk unavailable:** Export task retries 3 times with exponential backoff, then drops the batch and logs the error. Rows remain in SQLite.
- **SQLite unavailable:** Service degrades gracefully. Exchange requests still work (JWT validation and minting), but the audit log write fails silently (logged to stderr/stdout). The request is not blocked by a logging failure.
- **Channel full:** If the export task falls behind (e.g., thousands of events queued), the channel buffer fills and new sends are dropped with a warning. This prevents memory blowout.

---

## Schema migration

The initial schema is created at startup via `CREATE TABLE IF NOT EXISTS`. The `schema_version` table tracks the current version. For future schema changes:

1. Increment the version in `schema_version`.
2. Apply incremental migrations (ALTER TABLE, CREATE INDEX) as needed.

Migration v1 is the initial schema above (request_log, used_jti, schema_version).

---

## Cross-references

- Init design: `design-001-init.md` — overall architecture and configuration
- HTTP API: `design-002-http-api.md` — endpoint shapes (this logging design is invoked by the exchange handler)
- Token mapping: `design-003-token-mapping.md` — replay protection details
