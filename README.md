# JWT Exchange

An OAuth 2.0 token exchange service written in Rust. Validates incoming IdP-issued JWTs (e.g. Okta, Auth0), extracts user identity and claims, and mints new RSA-signed JWTs tailored for a downstream service (e.g. Qlik Sense JWT virtual proxy).

## Features

- **Token validation**: Verifies inbound JWTs against JWKS with force-refresh and circuit-breaker
- **Replay protection**: Atomic JTI tracking via SQLite — each token can only be exchanged once
- **Group filtering**: Server-side whitelist enforcement on requested group claims
- **Auto-generated keys**: RSA-2048 key pair + X.509 cert generated on first start, persisted to disk
- **Audit logging**: SQLite audit log with configurable retention (default 60 days)
- **Splunk export**: Optional streaming Splunk HEC exporter with batching and retry
- **Distroless container**: 2.4MB final image, runs as non-root

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

### Production (pull from GHCR)

Images are published to `ghcr.io/nnet-dev/jwt-exchange` on version tags (e.g. `v1.0.0`).

```bash
docker pull ghcr.io/nnet-dev/jwt-exchange:v1.0.0

docker run -d \
  --name jwt-exchange \
  -p 8080:8080 \
  -e INBOUND_ISSUER_URI=https://<tenant>.okta.com \
  -e QLIK_AUDIENCE=your-audience \
  -v jwt-data:/data \
  -v jwt-keys:/keys \
  ghcr.io/nnet-dev/jwt-exchange:v1.0.0
```

### Local dev (build from source)

```bash
docker compose -f docker-compose.yml -f docker-compose.dev.yml up --build
```

### Build standalone

```bash
docker build -t jwt-exchange .
```

- RSA keys auto-generate on first start and persist in the mounted `/keys` volume
- SQLite database lives in `/data` inside the container (mount to persist across runs)

## Layout

```
jwt-exchange/
├── src/
│   ├── lib.rs                  ← library crate (pub modules)
│   ├── main.rs                 ← binary entrypoint
│   ├── config.rs               ← environment variable parsing
│   ├── app.rs                  ← server bootstrap
│   ├── error.rs                ← error types + HTTP envelopes
│   ├── middleware.rs           ← X-Request-Id middleware
│   ├── auth/                   ← JWKS cache, RSA key management
│   ├── db/                     ← SQLite pool, audit log, JTI replay
│   ├── handlers/               ← HTTP endpoints (exchange, cert, health)
│   ├── logging/                ← Splunk HEC streaming exporter
│   ├── models/                 ← request/response DTOs
│   └── services/               ← token exchange, audit logging
├── tests/
│   ├── token_service.rs        ← unit tests for token logic
│   └── audit.rs                ← integration tests for JTI replay
├── contracts/                  ← stable API specifications
│   ├── http-api.openapi.yaml   ← OpenAPI 3.1 spec
│   ├── exchange-request.schema.json
│   └── downstream-jwt-payload.schema.json
├── migrations/                 ← SQL schema migrations
├── .sqlx/                      ← offline query cache (compile-time)
├── designs/                    ← design documentation
├── foundation/                 ← NNet engineering foundation (synced)
├── Dockerfile                  ← multi-stage musl + distroless build
├── docker-compose.yml          ← production compose (pulls GHCR image)
├── docker-compose.dev.yml      ← dev override (local build)
└── .github/workflows/          ← CI/CD (tag-triggered Docker build)
```

## API

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/v1/exchange` | Validate inbound JWT, mint and return downstream JWT |
| `GET` | `/api/v1/cert` | Public X.509 certificate (PEM) for signature verification |
| `GET` | `/api/v1/health` | Health/readiness probe |

### Exchange endpoint

`POST /api/v1/exchange` — accepts an inbound IdP JWT via `Authorization: Bearer` header (preferred) or request body `{"token": "..."}`.

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
| `400` | `missing_token` | No token provided |
| `400` | `malformed_token` | Invalid JWT structure or input limits exceeded |
| `401` | `invalid_signature` | Signature doesn't match any JWKS key |
| `401` | `expired_token` | Token `exp` claim is in the past |
| `401` | `invalid_issuer` | `iss` claim doesn't match configured issuer |
| `401` | `unknown_kid` | Key ID not found in JWKS after refresh |
| `401` | `replay_detected` | Token JTI has already been used |
| `503` | `idp_unavailable` | IdP JWKS fetch failed |

All errors follow the standard envelope:
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

Requested groups are filtered server-side against `GROUPS_WHITELIST`. Only whitelisted groups appear in the minted JWT. If none match, the `groups` claim is omitted entirely.

### Replay protection

Each token's `jti` (JWT ID) is atomically recorded before minting. Reusing the same `jti` returns `401 replay_detected`. Tokens without `jti` are tracked by SHA-256 hash of the raw token. Expired entries are purged hourly.

### Health endpoint

Returns `200 OK` when SQLite, RSA keys, and JWKS cache are all operational. Used as the Docker/Kubernetes readiness probe.

### Certificate endpoint

Returns the RSA public key as a PEM-encoded X.509 certificate. Downstream services use this to verify signatures on minted tokens.

## Configuration

| Variable | Required | Default | Description |
|---|---|---|---|
| `INBOUND_ISSUER_URI` | Yes | — | IdP issuer URL (JWKS discovery base) |
| `INBOUND_AUDIENCE_VALIDATION` | No | `true` | Validate `aud` claim on inbound tokens |
| `INBOUND_EXPECTED_AUDIENCE` | If validation enabled | — | Expected audience value |
| `QLIK_AUDIENCE` | Yes | — | Audience for minted downstream JWTs |
| `QLIK_USER_DIRECTORY` | No | — | User directory for downstream `sub` claim |
| `GROUPS_WHITELIST` | No | — | Comma-separated group whitelist |
| `RSA_PRIVATE_KEY_PATH` | No | — | Path to existing RSA private key (PEM) |
| `RSA_PUBLIC_CERT_PATH` | No | — | Path to existing X.509 cert (PEM) |
| `RSA_KEY_DIR` | No | `/app/data/keys` | Directory for auto-generated key pair |
| `DB_PATH` | No | `/data/jwt-exchange.db` | SQLite audit log path |
| `LOG_RETENTION_DAYS` | No | `60` | Audit log retention period |
| `SPLUNK_HEC_URL` | No | — | Splunk HEC endpoint |
| `SPLUNK_HEC_TOKEN` | No | — | Splunk HEC authentication token |
| `SPLUNK_HEC_SKIP_TLS_VERIFY` | No | `false` | Disable TLS verification for Splunk |
| `ALLOW_REPLAY` | No | `false` | Allow same JTI twice (once with groups, once without) |
| `MAX_TOKEN_SIZE` | No | `10240` | Maximum inbound token size (bytes) |
| `MAX_GROUPS_COUNT` | No | `50` | Maximum groups in a single request |
| `MAX_GROUP_NAME_LENGTH` | No | `256` | Maximum group name length (chars) |

## Tests

```bash
cargo test
```

23 tests total:
- **15 token service tests** — group filtering, base64url decoding, SHA-256 hashing
- **8 audit integration tests** — JTI replay detection (strict and replay modes)

## Contracts

Stable API specifications in `contracts/`:

| File | Description |
|---|---|
| `http-api.openapi.yaml` | OpenAPI 3.1 spec for all endpoints |
| `exchange-request.schema.json` | JSON Schema for `/exchange` request body |
| `downstream-jwt-payload.schema.json` | JSON Schema for minted JWT payload |

## Foundation sync

This project consumes the NNet engineering foundation via sync (not a submodule). To update:

```bash
./scripts/sync-foundation.sh                  # latest tagged release
./scripts/sync-foundation.sh --version 0.2.0  # pin to specific version
```

Files inside `foundation/` are synced and **must not be edited locally**.

## CI/CD

GitHub Actions builds and publishes Docker images to GHCR on version tag pushes:

```bash
git tag v1.0.1 && git push origin v1.0.1
```
