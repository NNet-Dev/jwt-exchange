---
app: jwt-exchange
owner: Marc
status: Active
supersedes: []
depends_on: []
owns: [http-api, database]
---

# Design 001 — Init

---

## Purpose

JWT Exchange is a token exchange service. It receives an IdP-issued JWT, validates it against the IdP JWKS, extracts the user identity, and mints a new RSA-signed JWT tailored for a downstream JWT virtual proxy. All requests are logged to SQLite with a configurable auto-purge and a Splunk HEC export stream.

---

## What this project owns

- **Domain:** Token exchange — IdP JWT in, downstream JWT out
- **Data:**
  - IdP JWKS (fetched at startup, cached with ETag-aware refresh)
  - RSA signing key pair (auto-generated or loaded from PEM)
  - Request audit log (SQLite, configurable retention with auto-purge)
  - Used JTI tracking (SQLite, for replay protection)
- **Services:**
  - HTTP endpoint that accepts an inbound JWT and returns a downstream-compatible JWT
  - HTTP endpoint that returns the public X.509 certificate (PEM)
  - Background task for log auto-purge
  - Background task for JTI expiry cleanup
  - Streaming Splunk HEC exporter (optional)
- **External systems:**
  - **IdP issuer** (e.g. `https://<tenant>.okta.com`) — validates incoming tokens via `/.well-known/openid-configuration` → `jwks_uri`
  - **Downstream service** (e.g. Qlik Sense JWT virtual proxy) — trusts the public X.509 cert of our signing key pair
  - **Splunk HEC** — receives structured log events via HTTP

---

## Architecture at a glance

This project is an HTTP service built with Rust. It deploys independently and follows the layered service shape defined in `foundation/conventions/CODE_CONVENTIONS-rust.md`: handlers → services → storage/IO.

### Core flow

```
Client ──IdP JWT──▶ JWT Exchange ──Downstream JWT──▶ Client (→ downstream service)
                    │
                    └──▶ IdP well-known (discovery at startup)
                    └──▶ IdP JWKS (key material for validation)
                    └──▶ RSA private key (signing key for new JWT)
                    └──▶ SQLite (audit log write, JTI replay check)
                    └──▶ Splunk HEC (log export stream)
                    └──▶ Auto-purge (background, configurable retention)
```

### HTTP endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/v1/exchange` | Accept inbound JWT, validate, mint and return downstream JWT |
| `GET` | `/api/v1/cert` | Return the public X.509 certificate in PEM format |
| `GET` | `/api/v1/health` | Health check (startup probe, readiness probe) |

### Step by step

1. **Startup**
   - Load configuration from environment variables.
   - Fetch `/.well-known/openid-configuration` from the IdP issuer.
   - Extract `jwks_uri` and fetch the JWKS. Cache in memory.
   - Resolve RSA key pair: load existing PEM files if provided, otherwise auto-generate RSA-2048 + X.509 cert in `RSA_KEY_DIR`.
   - Initialize SQLite database for request logging and JTI tracking.
   - Start streaming Splunk HEC export (if configured).
   - Start background auto-purge task (audit log retention) and JTI cleanup task.

2. **Incoming request** (`POST /api/v1/exchange`)
   - Accept an inbound JWT (from `Authorization: Bearer` header or request body).
   - Validate signature against the cached JWKS.
   - Verify standard claims: `exp` (not expired), `iss` (matches IdP issuer), `aud` (if configured).
   - Check replay protection: atomically record the token's `jti` (or SHA-256 hash if no `jti`) in the `used_jti` table. If already recorded, reject with `401 replay_detected`.
   - Extract `sub` and any mapped claims (e.g. groups, email, name).
   - Filter requested groups against `GROUPS_WHITELIST`.

3. **Token exchange**
   - Build a new JWT payload with downstream-required claims:
     - `userid` — mapped from IdP `sub` (or a custom claim)
     - `userdirectory` — from env var, if set; excluded from payload if not
     - `name`, `email` — carried from IdP claims if present
     - `groups` — filtered against whitelist (omitted if none match)
     - Standard JWT claims: `iss`, `aud` (mandatory), `exp`, `nbf`, `iat`, `jti`
   - Sign with RSA (RS256) using the local private key.

4. **Response**
   - Return the new JWT to the caller.
   - Log the exchange attempt to SQLite and Splunk (async, non-blocking).

### Key design decisions

- **RSA signing** — Downstream JWT virtual proxy only supports RS256, RS384, RS512. RS256 is the standard.
- **Public cert endpoint** — `GET /api/v1/cert` returns the PEM-formatted public X.509 certificate for pasting into the downstream proxy's certificate field.
- **No relay/proxy** — the service returns the new token to the caller; the caller is responsible for sending it to the downstream service. This keeps the service stateless and avoids session management.
- **JWKS caching** — JWKS is cached in memory with ETag-aware periodic refresh. If signature validation fails with an unknown `kid`, re-fetch the JWKS and retry once (handles key rotation). Cooldown prevents abuse.
- **SQLite for logging** — lightweight, file-based, no external database dependency. Auto-purge via background task.
- **Replay protection** — atomic `INSERT OR IGNORE` on `used_jti` table. Each token can only be exchanged once. Optional `ALLOW_REPLAY` mode permits the same JTI twice (once with groups, once without) via composite key `(jti, has_groups)`.
- **Splunk HEC export** — streaming, non-blocking. Logs are written to SQLite first, then streamed to Splunk. If Splunk is unavailable, logs stay in SQLite.

### Configuration

| Key | Required | Default | Description |
|---|---|---|---|
| `INBOUND_ISSUER_URI` | Yes | — | IdP issuer URL (JWKS discovery base) |
| `INBOUND_AUDIENCE_VALIDATION` | No | `true` | Validate `aud` claim on inbound tokens |
| `INBOUND_EXPECTED_AUDIENCE` | If validation enabled | — | Expected audience value |
| `QLIK_AUDIENCE` | Yes | — | Audience for minted downstream JWTs |
| `QLIK_USER_DIRECTORY` | No | _(excluded)_ | User directory for downstream `sub` claim |
| `GROUPS_WHITELIST` | No | — | Comma-separated group whitelist |
| `RSA_PRIVATE_KEY_PATH` | No | — | Path to existing RSA private key (PEM) |
| `RSA_PUBLIC_CERT_PATH` | No | — | Path to existing X.509 cert (PEM) |
| `RSA_KEY_DIR` | No | `/app/data/keys` | Directory for auto-generated key pair |
| `DB_PATH` | No | `/data/jwt-exchange.db` | SQLite audit log path |
| `LOG_RETENTION_DAYS` | No | `60` | Audit log retention period |
| `SPLUNK_HEC_URL` | No | _(disabled)_ | Splunk HEC endpoint URL |
| `SPLUNK_HEC_TOKEN` | No | _(disabled)_ | Splunk HEC authentication token |
| `SPLUNK_HEC_SKIP_TLS_VERIFY` | No | `false` | Disable TLS verification for Splunk |
| `ALLOW_REPLAY` | No | `false` | Allow same JTI twice (with/without groups) |
| `MAX_TOKEN_SIZE` | No | `10240` | Maximum inbound token size (bytes) |
| `MAX_GROUPS_COUNT` | No | `50` | Maximum groups in a single request |
| `MAX_GROUP_NAME_LENGTH` | No | `256` | Maximum group name length (chars) |
| `TOKEN_TTL_SECONDS` | No | `3600` | TTL for minted downstream JWT |
| `LISTEN_HOST` | No | `0.0.0.0` | Service bind address |
| `LISTEN_PORT` | No | `8080` | Service listen port |

### Downstream JWT payload shape

Per downstream documentation, the JWT payload must include:

```json
{
  "userid": "user-sub-from-idp",
  "userdirectory": "QSEFW",          // optional: only if QLIK_USER_DIRECTORY is set
  "name": "Full Name",
  "email": "user@example.com",
  "groups": ["Group A", "Group B"],
  "iss": "jwt-exchange",
  "aud": "qlik-sense-jwt",           // mandatory: from QLIK_AUDIENCE
  "exp": 1635355629,
  "nbf": 1635352029,
  "iat": 1635352029,
  "jti": "unique-token-id"
}
```

The `userid` and `userdirectory` claim names are configurable in the downstream proxy settings.

### Certificate management

The RSA key pair is **auto-generated at first startup** if no existing key/cert paths are configured. Keys are written to the configured `RSA_KEY_DIR` and persist across restarts (mount as a volume in containerised deployments). Alternatively, pre-existing PEM files can be provided via `RSA_PRIVATE_KEY_PATH` and `RSA_PUBLIC_CERT_PATH` environment variables.

The `GET /api/v1/cert` endpoint outputs the public cert PEM for downstream configuration.

---

## Deployment

This project is deployment-agnostic. The deployment contract is:
- Config via environment variables (see table above)
- RSA key pair mounted as files or provided via secret management
- SQLite database file persists on disk (ensure volume mount if containerised)
- External systems connected via HTTPS (IdP OIDC discovery, downstream proxy, Splunk HEC)

---

## Implemented designs

All subsequent design documents are complete and implemented:

- `design-002-http-api.md` — HTTP endpoint contracts
- `design-003-token-mapping.md` — Claim mapping rules
- `design-004-jwks-management.md` — JWKS caching and refresh
- `design-005-logging.md` — SQLite schema, auto-purge, Splunk export
