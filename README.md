# JWT Exchange

An OAuth 2.0 token exchange service written in Rust. Validates incoming IdP-issued JWTs, extracts user identity and claims, and mints new RSA-signed JWTs tailored for a downstream service (e.g. Qlik Sense JWT virtual proxy).

## Quick start

```bash
# Copy and configure environment variables
cp .env.example .env

# Run locally (SQLite auto-created, RSA keys auto-generated)
cargo run

# Or with Docker (production)
docker compose up -d

# Or with Docker (dev, builds from source)
docker compose -f docker-compose.yml -f docker-compose.dev.yml up --build
```

## Docker

### Quick commands

| Scenario | Command |
|---|---|
| **Local dev (build from source)** | `docker compose -f docker-compose.yml -f docker-compose.dev.yml up --build` |
| **Production (pull from ghcr.io)** | `docker compose up -d` |

### Build standalone

```bash
docker build -t jwt-exchange .
```

### Run standalone

```bash
docker run -d \
  --name jwt-exchange \
  -p 8080:8080 \
  -e INBOUND_ISSUER_URI=https://<tenant>.okta.com \
  -e QLIK_AUDIENCE=your-audience \
  -e RSA_KEY_PATH=/keys \
  -v jwt-keys:/keys \
  jwt-exchange
```

- RSA keys auto-generate on first start and persist in the `jwt-keys` volume
- SQLite database lives in `/data` inside the container (bind-mount to persist across runs)

### Pull from ghcr.io

Images are published to `ghcr.io/nnet-dev/jwt-exchange`. Tagged by branch (e.g. `main`).

```bash
docker pull ghcr.io/nnet-dev/jwt-exchange:main

docker run -d \
  --name jwt-exchange \
  -p 8080:8080 \
  ghcr.io/nnet-dev/jwt-exchange:main
```

## Where to start

If you're new to this project, read in this order:

1. `ARCHITECTURE.md` — what this project is and how it's shaped.
2. `foundation/DESIGN_PHILOSOPHY.md` — the values that drive every design decision.
3. `designs/design-001-init.md` — the founding design.

If you're touching code, read `foundation/conventions/CODE_CONVENTIONS-rust.md`.

## Foundation sync

This project consumes the NNet engineering foundation via sync (not a submodule). To update:

```bash
./scripts/sync-foundation.sh                  # latest tagged release
./scripts/sync-foundation.sh --version 0.2.0  # pin to specific version
```

Files inside `foundation/` are synced and **must not be edited locally**. For project-specific overrides, add documents to `conventions-local/`.

## Layout

```
jwt-exchange/
├── README.md                    ← this file
├── ARCHITECTURE.md              ← project shape
├── Cargo.toml                   ← Rust package manifest
├── .env.example                 ← environment variables template
├── .foundation                  ← foundation metadata (JSON)
├── foundation/                  ← SYNCED, do not edit
├── conventions-local/           ← project overrides/extensions
├── designs/                     ← project design docs (numbered)
├── contracts/                   ← API contracts and schemas
│   └── draft/                   ← in-flux specs (OpenAPI + JSON schemas)
├── Dockerfile                   ← multi-stage distroless build
├── docker-compose.yml           ← local dev orchestration
├── src/                         ← source code
│   ├── main.rs                  ← async entrypoint
│   ├── config.rs                ← env var loading
│   ├── app.rs                   ← application state
│   ├── auth/                    ← JWKS cache, RSA key management
│   ├── services/                ← token exchange logic
│   ├── handlers/                ← HTTP endpoints
│   ├── db/                      ← SQLite pool & audit log
│   ├── logging/                 ← Splunk HEC export
│   ├── models/                  ← request/response DTOs
│   └── error.rs                 ← error types
├── migrations/                  ← SQL migrations
└── scripts/                     ← utility scripts
```

## Contracts

Draft API specifications live in `contracts/draft/`:

| File | Description |
|---|---|
| `http-api.openapi.yaml` | OpenAPI 3.0 spec for all endpoints |
| `exchange-request.schema.json` | JSON Schema for `/exchange` request body |
| `downstream-jwt-payload.schema.json` | JSON Schema for minted JWT payload |

These are working drafts — not yet frozen for public consumption.

## API

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/v1/exchange` | Validate incoming JWT, mint and return new JWT |
| `GET` | `/api/v1/cert` | Return the public X.509 certificate (PEM) for downstream signature verification |
| `GET` | `/api/v1/health` | Health/readiness probe — returns `200 OK` when the service is ready to accept requests |

### Exchange endpoint

`POST /api/v1/exchange` validates an incoming IdP JWT and returns a minted RSA-signed JWT for the downstream service.

**Authentication:** The incoming token can be provided either in the `Authorization: Bearer <token>` header (preferred) or in the request body as `{ "token": "..." }`. Header takes precedence.

**Request body (optional when using Bearer header):**
```json
{
  "token": "eyJhbG...",
  "groups": ["admins", "analysts"]
}
```

**Response (200):**
```json
{
  "accessToken": "eyJhbG...",
  "tokenType": "Bearer",
  "expiresIn": 3600,
  "issuedAt": "2026-05-17T12:00:00Z"
}
```

**Error responses:**

| Status | Error Code | Description |
|---|---|---|
| `400` | `missing_token` | No token in body or Authorization header |
| `400` | `malformed_token` | Token is not a valid JWT structure |
| `401` | `invalid_signature` | JWT signature doesn't match any JWKS key |
| `401` | `expired_token` | Token `exp` claim is in the past |
| `401` | `invalid_issuer` | `iss` claim doesn't match configured issuer |
| `401` | `unknown_kid` | Key ID not found in JWKS (even after refresh) |
| `401` | `replay_detected` | Token JTI has already been used |
| `500` | `internal_error` | Unexpected server error |
| `503` | `idp_unavailable` | IdP JWKS fetch failed |

All errors follow the standard envelope format:
```json
{
  "error": "invalid_signature",
  "message": "JWT signature verification failed",
  "detail": "signature did not match any JWKS key",
  "requestId": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": "2026-05-17T12:00:00Z"
}
```

### Group filtering

Requested groups are filtered server-side against the `GROUPS_WHITELIST` environment variable. Only groups present in the whitelist are included in the minted JWT. If none of the requested groups match, the `groups` claim is omitted from the downstream token entirely.

### Replay protection

Each incoming token's `jti` (JWT ID) is atomically recorded in a SQLite `used_jti` table before a downstream token is minted. If the same `jti` is seen again, the exchange is rejected with `401 REPLAY_DETECTED`. Expired JTIs are purged hourly to keep the table bounded. This means each inbound token can only be exchanged once — replaying a captured token will fail.

### Health endpoint

`GET /api/v1/health` is the primary readiness probe used by Docker Compose and Kubernetes. It returns `200 OK` when:
- The SQLite audit log database is accessible
- RSA keys are loaded (or auto-generated on first start)
- The IdP JWKS cache has been initialised

### Certificate endpoint

`GET /api/v1/cert` returns the RSA public key as a PEM-encoded X.509 certificate. Downstream services (e.g. Qlik Sense JWT virtual proxy) use this to verify signatures on minted tokens.

## Tests

```bash
cargo test
```
