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

JWT Exchange is a token exchange service. It receives an IdP-issued JWT, validates it against the IdP JWKS, extracts the user identity, and mints a new RSA-signed JWT tailored for a downstream JWT virtual proxy. All requests are logged to SQLite with a 60-day auto-purge and a Splunk HEC export stream.

---

## What this project owns

- **Domain:** Token exchange — IdP JWT in, Qlik JWT out
- **Data:** 
  - IdP JWKS (fetched at startup, cached with refresh)
  - RSA signing key pair (private key held, public cert exported as PEM)
  - Request audit log (SQLite, 60-day retention with auto-purge)
- **Services:** 
  - HTTP endpoint that accepts an inbound JWT and returns a Qlik-compatible JWT
  - HTTP endpoint that returns the public X.509 certificate (PEM)
  - Background task for log auto-purge
  - Splunk HEC export stream for log forwarding
- **External systems:**
  - **IdP issuer** (e.g. `https://<tenant>.okta.com`) — validates incoming tokens via `/.well-known/openid-configuration` → `jwks_uri`
  - **Qlik Sense Enterprise on Windows** (self-hosted) — JWT virtual proxy trusts the public X.509 cert of our signing key pair
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
                    └──▶ SQLite (audit log write)
                    └──▶ Splunk HEC (log export stream)
                    └──▶ Auto-purge (background, 60-day retention)
```

### HTTP endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/exchange` | Accept inbound JWT, validate, mint and return Qlik JWT |
| `GET` | `/cert` | Return the public X.509 certificate in PEM format |
| `GET` | `/health` | Health check (startup probe, readiness probe) |

### Step by step

1. **Startup**
   - Load configuration from environment variables.
   - Fetch `/.well-known/openid-configuration` from the IdP issuer.
   - Extract `jwks_uri` and fetch the JWKS. Cache in memory.
   - Load the RSA private key from disk (PEM format).
   - Initialize SQLite database for request logging.
   - Start Splunk HEC export stream (if configured).
   - Start background auto-purge task (60-day retention).

2. **Incoming request** (`POST /exchange`)
   - Accept an inbound JWT (from `Authorization: Bearer` header or request body).
   - Log the request to SQLite (timestamp, source IP, inbound subject, validation result).
   - Forward log event to Splunk HEC (async, non-blocking).
   - Validate signature against the cached JWKS.
   - Verify standard claims: `exp` (not expired), `iss` (matches IdP issuer), `aud` (if configured).
   - Extract `sub` and any mapped claims (e.g., groups, email, name).

3. **Token exchange**
   - Build a new JWT payload with Qlik-required claims:
     - `userid` — mapped from IdP `sub` (or a custom claim)
     - `userdirectory` — from env var, if set; excluded from payload if not
     - `name`, `email` — carried from IdP claims if present
     - `groups` — mapped from IdP groups claim (TBD mapping)
     - Standard JWT claims: `iss`, `aud` (mandatory), `exp`, `nbf`, `iat`, `jti`
   - Sign with RSA (RS256) using the local private key.

4. **Response**
   - Return the new JWT to the caller.
   - Caller uses it to authenticate against the Qlik Sense JWT virtual proxy via `Authorization: Bearer <token>`.

### Key design decisions

- **RSA signing** — Qlik Sense JWT virtual proxy only supports RS256, RS384, RS512. RS256 is the standard.
- **Public cert endpoint** — `GET /cert` returns the PEM-formatted public X.509 certificate for pasting into the QMC virtual proxy's "JWT certificate" field.
- **No relay/proxy** — the service returns the new token to the caller; the caller is responsible for sending it to Qlik Sense. This keeps the service stateless and avoids session management.
- **JWKS caching** — JWKS is cached in memory with periodic refresh. If signature validation fails with an unknown `kid`, re-fetch the JWKS and retry once (handles key rotation).
- **SQLite for logging** — lightweight, file-based, no external database dependency. Auto-purge via background task deleting rows older than 60 days.
- **Splunk HEC export** — async, non-blocking stream. Logs are written to SQLite first, then forwarded to Splunk. If Splunk is unavailable, logs stay in SQLite and are not lost.

### Configuration

| Key | Required | Default | Description |
|---|---|---|---|
| `INBOUND_ISSUER_URI` | Yes | — | IdP tenant base URL |
| `RSA_PRIVATE_KEY_PATH` | Yes | — | Path to RSA private key (PEM) |
| `RSA_PUBLIC_CERT_PATH` | Yes | — | Path to public X.509 cert (PEM) |
| `QLIK_AUDIENCE` | Yes | — | Value for the `aud` claim (must match QMC config) |
| `QLIK_USER_DIRECTORY` | No | _(excluded)_ | Qlik user directory name. If unset, excluded from JWT payload. |
| `TOKEN_TTL_SECONDS` | No | `3600` | TTL for minted Qlik JWT |
| `LISTEN_HOST` | No | `0.0.0.0` | Service bind address |
| `LISTEN_PORT` | No | `8080` | Service listen port |
| `DB_PATH` | No | `./jwt-exchange.db` | SQLite database path for request logs |
| `LOG_RETENTION_DAYS` | No | `60` | Days to retain request logs before auto-purge |
| `SPLUNK_HEC_URL` | No | _(disabled)_ | Splunk HEC endpoint URL (e.g. `https://splunk:8088/services/collector`) |
| `SPLUNK_HEC_TOKEN` | No | _(disabled)_ | Splunk HEC authentication token |

### Qlik Sense JWT payload shape

Per Qlik documentation, the JWT payload must include:

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

The `userid` and `userdirectory` claim names are configurable in the QMC virtual proxy settings — they can be different keys if needed, but these are the defaults shown in Qlik's docs.

### Certificate management

The RSA key pair is **auto-generated at first startup** if no existing key/cert paths are configured. Keys are written to the configured `RSA_KEY_DIR` and persist across restarts (mount as a volume in containerised deployments). Alternatively, pre-existing PEM files can be provided via `RSA_PRIVATE_KEY_PATH` and `RSA_PUBLIC_CERT_PATH` environment variables.

The `GET /cert` endpoint outputs the public cert PEM for QMC configuration.

```bash
# Manual generation (optional — service auto-generates if keys are absent)
openssl genrsa -out privatekey.pem 4096
openssl req -new -x509 -key privatekey.pem -out publickey.cer -days 1825 \
  -subj "/CN=jwt-exchange/O=jwt-exchange"
```

---

## Deployment

This project is deployment-agnostic. The deployment contract is:
- Config via environment variables (see table above)
- RSA key pair mounted as files or provided via secret management
- SQLite database file persists on disk (ensure volume mount if containerised)
- External systems connected via HTTPS (IdP OIDC discovery, Qlik Sense proxy, Splunk HEC)

---

## Next

Subsequent design docs will define:

- `design-002-http-api.md` — HTTP endpoint contracts (request shapes, response shapes, error envelope)
- `design-003-token-mapping.md` — Claim mapping rules (IdP → Qlik), groups mapping, TTL strategy
- `design-004-jwks-management.md` — JWKS caching, refresh strategy, unknown-kid retry
- `design-005-logging.md` — SQLite schema, auto-purge, Splunk HEC export stream
