---
app: jwt-exchange
owner: Marc
status: Draft
supersedes: []
depends_on: [design-001-init.md]
owns: [database]
---

# Design 005 — Logging

---

## Purpose

Define the request audit logging system: SQLite storage with 60-day auto-purge, and a Splunk HEC export stream for external log aggregation.

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

```sql
CREATE TABLE IF NOT EXISTS request_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    source_ip   TEXT NOT NULL,
    inbound_sub    TEXT,                     -- NULL if validation failed before claim extraction
    inbound_iss    TEXT,                     -- extracted issuer from incoming JWT
    inbound_aud    TEXT,                     -- extracted audience from incoming JWT
    validation  TEXT NOT NULL,            -- 'success' | 'expired' | 'invalid_sig' | 'unknown_kid' | 'bad_iss' | 'bad_aud' | 'malformed'
    error_detail TEXT,                    -- human-readable error context (NULL on success)
    outbound_sub    TEXT,                     -- subject in the minted JWT (NULL if not minted)
    token_jti   TEXT,                     -- JTI of the minted JWT (NULL if not minted)
    response_code INTEGER NOT NULL,       -- HTTP response code returned
    elapsed_ms  INTEGER NOT NULL          -- request processing time in milliseconds
);

CREATE INDEX IF NOT EXISTS idx_request_log_timestamp ON request_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_request_log_inbound_sub ON request_log(inbound_sub);
```

### Field rationale

- **`timestamp`** — ISO 8601 datetime string. Indexed for range queries and purge.
- **`source_ip`** — extracted from the request (X-Forwarded-For if behind a proxy, otherwise the direct connection IP).
- **`inbound_sub`** — the subject from the incoming identity provider JWT. NULL if we couldn't parse or validate the token at all (e.g., malformed JWT).
- **`validation`** — categorical outcome. Enables quick aggregation: `SELECT validation, COUNT(*) FROM request_log GROUP BY validation`.
- **`error_detail`** — for failed validations, captures the specific reason (e.g., "token expired at 2026-05-17T08:00:00Z", "unknown kid abc123", "issuer mismatch: expected X got Y").
- **`outbound_sub` / `token_jti`** — only populated on successful exchange. Links the outbound token to the inbound request for audit trails.
- **`elapsed_ms`** — end-to-end request processing time. Useful for performance monitoring.

---

## Auto-purge

### Mechanism

A background task runs periodically (default: every 6 hours) and deletes rows older than the configured retention period:

```sql
DELETE FROM request_log
WHERE timestamp < datetime('now', '-{LOG_RETENTION_DAYS} days');
```

### Configuration

| Key | Default | Description |
|---|---|---|
| `LOG_RETENTION_DAYS` | `60` | Days to retain request logs |
| `PURGE_INTERVAL_SECONDS` | `21600` | Seconds between purge runs (6 hours) |

### Design notes

- Purge runs **after** the log row is written — it never interferes with the current request.
- If the purge deletes rows that haven't yet been exported to Splunk, those rows are lost. The purge should therefore only delete rows that are older than the retention period AND have been successfully exported (if Splunk is configured).
- **Alternative:** Add an `exported_to_splunk` boolean column. Purge only deletes rows where `exported_to_spluck = true OR timestamp < retention_cutoff`. But this adds complexity. Simpler approach: Splunk export is async and best-effort. If Splunk is down for >60 days, that's an operational issue, not a design issue.

Let's keep it simple: purge by timestamp only. If Splunk export fails, that's monitored separately.

---

## Splunk HEC export stream

### Mechanism

An async background task reads unexported log rows and sends them to the Splunk HTTP Event Collector (HEC):

```
SQLite ──unexported rows──▶ Export task ──HTTP POST──▶ Splunk HEC
```

### Flow

1. On startup, start the export task as an async Tokio task.
2. The task polls SQLite every `SPLUNK_POLL_INTERVAL_SECONDS` (default: 10s) for rows where `exported_to_splunk = false`.
3. Batch up to `SPLUNK_BATCH_SIZE` (default: 100) rows.
4. Format each row as a Splunk HEC JSON event.
5. POST the batch to the Splunk HEC endpoint.
6. On success, mark rows as exported.
7. On failure, log the error and retry on the next poll cycle.

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
    "token_jti": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "response_code": 200,
    "elapsed_ms": 42
  }
}
```

### Batch request

Multiple events can be sent in a single HEC request as a JSON array or newline-delimited JSON:

```json
[
  { "time": ..., "event": {...} },
  { "time": ..., "event": {...} }
]
```

### Configuration

| Key | Required | Default | Description |
|---|---|---|---|
| `SPLUNK_HEC_URL` | No | _(disabled)_ | Splunk HEC endpoint (e.g. `https://splunk:8088/services/collector`) |
| `SPLUNK_HEC_TOKEN` | No | _(disabled)_ | Splunk HEC authentication token |
| `SPLUNK_POLL_INTERVAL_SECONDS` | No | `10` | Seconds between export polls |
| `SPLUNK_BATCH_SIZE` | No | `100` | Max rows per export batch |
| `SPLUNK_SOURCE` | No | `jwt-exchange` | Splunk `source` field |
| `SPLUNK_SOURCETYPE` | No | `jwt-exchange:audit` | Splunk `sourcetype` field |

### Resilience

- **Splunk unavailable:** Export task logs the error, rows remain unexported, retried on next poll. No rows are lost.
- **SQLite unavailable:** Service degrades gracefully. Exchange requests still work (JWT validation and minting), but the audit log write fails silently (logged to stderr/stdout). The request is not blocked by a logging failure.
- **Backpressure:** If the export task falls behind (e.g., thousands of unexported rows), it processes in batches of `SPLUNK_BATCH_SIZE` per poll cycle. This prevents memory blowout.

---

## Schema migration

The initial schema is created at startup via `CREATE TABLE IF NOT EXISTS`. For future schema changes:

1. Add a `schema_version` table to track the current schema version.
2. On startup, compare current version to latest.
3. Apply incremental migrations (ALTER TABLE, CREATE INDEX) as needed.

```sql
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO schema_version (version) VALUES (1);
```

Migration v1 is the initial schema above. Future migrations increment the version and apply their changes.

---

## Cross-references

- Init design: `design-001-init.md` — overall architecture and configuration
- HTTP API: `design-002-http-api.md` — endpoint shapes (this logging design is invoked by the exchange handler)
