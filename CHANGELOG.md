# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.0.0] — 2026-05-21

Initial release — production-ready JWT exchange service.

### Added

- **Token exchange**: `POST /api/v1/exchange` validates inbound IdP JWTs against JWKS and mints RSA-signed downstream JWTs
- **JWKS management**: ETag-based caching with force-refresh cooldown (60s) and circuit-breaker (5 consecutive failures)
- **RSA key management**: Auto-generates RSA-2048 key pair + X.509 cert on first start; loads existing PEM files when provided
- **Replay protection**: Atomic JTI tracking via SQLite `INSERT OR IGNORE`. Tokens without `jti` tracked by SHA-256 hash. Expired entries purged hourly.
- **Optional replay mode** (`ALLOW_REPLAY=true`): Same JTI may be exchanged twice — once with groups, once without — via composite key `(jti, has_groups)`
- **Group filtering**: Server-side whitelist enforcement on requested group claims with input validation (count, length limits)
- **Audit logging**: SQLite audit log with configurable retention (default 60 days), tracks all exchange attempts with claims context
- **Splunk HEC export**: Optional streaming exporter with batching, retry (3 attempts with exponential backoff), and TLS verification
- **Health endpoint**: `GET /api/v1/health` readiness probe checking SQLite, RSA keys, and JWKS cache
- **Certificate endpoint**: `GET /api/v1/cert` returns PEM-encoded X.509 public certificate
- **Request ID middleware**: `X-Request-Id` injected into all responses
- **X-Forwarded-For support**: Extracts first non-empty IP from comma-separated header
- **Input validation**: `MAX_TOKEN_SIZE`, `MAX_GROUPS_COUNT`, `MAX_GROUP_NAME_LENGTH` limits
- **Integration tests**: 23 tests covering token logic and JTI replay detection
- **OpenAPI 3.1 specification** in `contracts/`
- **Multi-stage Docker build**: musl static binary in `gcr.io/distroless/static-debian12:nonroot` (2.4MB)
- **Docker Compose variants**: `docker-compose.yml` for production (pulls GHCR image), `docker-compose.dev.yml` for local builds
- **GitHub Actions CI/CD**: Auto-builds and publishes to `ghcr.io/nnet-dev/jwt-exchange` on version tags

### Security

- **Audience validation**: Inbound `aud` claim validated against configured expected audience (enabled by default)
- **RSA key permissions**: Private keys created with `0600` permissions
- **No raw tokens in logs**: JTI values truncated to first 8 hex chars in Splunk events
- **Distroless non-root container**: Runs as UID 1000, no shell or package manager
- **WAL mode**: SQLite journaling for concurrent access safety
