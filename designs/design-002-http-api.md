---
app: jwt-exchange
owner: Marc
status: Active
supersedes: []
depends_on: [design-001-init.md]
owns: [http-api]
---

# Design 002 — HTTP API

---

## Purpose

Define the HTTP API contract for JWT Exchange: endpoint paths, request shapes, response shapes, and error envelope.

---

## Endpoints

| Method | Path | Description | Auth |
|---|---|---|---|
| `POST` | `/api/v1/exchange` | Accept inbound JWT, return downstream-compatible JWT | None |
| `GET` | `/api/v1/cert` | Return public X.509 certificate (PEM) | None |
| `GET` | `/api/v1/health` | Health check (startup + readiness probe) | None |

---

## POST /api/v1/exchange

### Request

**Content-Type:** `application/json`

```json
{
  "token": "<base64-encoded inbound JWT>",
  "groups": ["Group A", "Group B"]
}
```

Alternatively, the token can be sent in the `Authorization` header:

```
Authorization: Bearer <JWT>
```

If both are present, the header takes precedence. The `groups` array is always read from the request body.

### Response — Success (200)

```json
{
  "accessToken": "<base64-encoded downstream JWT>",
  "tokenType": "Bearer",
  "expiresIn": 3600,
  "issuedAt": "2026-05-17T12:00:00Z"
}
```

| Field | Type | Description |
|---|---|---|
| `accessToken` | string | The minted downstream-compatible JWT |
| `tokenType` | string | Always `"Bearer"` |
| `expiresIn` | integer | Token TTL in seconds |
| `issuedAt` | string | ISO 8601 timestamp of token issuance |

### Response — Errors

All errors use the standard error envelope (see § Error Envelope below).

| Status | Error Code | Scenario |
|---|---|---|
| `400` | `missing_token` | No token provided in body or header |
| `400` | `malformed_token` | Token is not a valid JWT structure or exceeds size limits |
| `401` | `invalid_signature` | JWT signature does not match any key in JWKS |
| `401` | `expired_token` | JWT `exp` claim is in the past |
| `401` | `invalid_issuer` | JWT `iss` does not match configured IdP issuer |
| `401` | `invalid_audience` | JWT `aud` does not match expected audience (if configured) |
| `401` | `unknown_kid` | JWT `kid` not found in JWKS and refresh also didn't find it |
| `401` | `replay_detected` | Token JTI (or SHA-256 hash) has already been used |
| `403` | `groups_not_allowed` | All requested groups filtered out by whitelist |
| `500` | `internal_error` | Unexpected server error |
| `503` | `idp_unavailable` | Failed to fetch IdP well-known config or JWKS |

### Rate limiting

No rate limiting at the service level. If needed, apply at the reverse proxy/load balancer layer.

---

## GET /api/v1/cert

### Request

No request body or parameters.

### Response — Success (200)

**Content-Type:** `application/x-pem-file`

```
-----BEGIN CERTIFICATE-----
MIIFazCCA1OgAwIBAgIUfG...
...
-----END CERTIFICATE-----
```

The response is the raw PEM-formatted X.509 public certificate.

### Response — Errors

| Status | Error Code | Scenario |
|---|---|---|
| `500` | `internal_error` | Certificate not loaded or unreadable |

---

## GET /api/v1/health

### Request

No request body or parameters.

### Response — Success (200)

```json
{
  "status": "healthy",
  "checks": {
    "jwks": "ok",
    "signing_key": "ok",
    "database": "ok"
  },
  "uptime_seconds": 86400
}
```

### Response — Degraded (503)

```json
{
  "status": "degraded",
  "checks": {
    "jwks": "ok",
    "signing_key": "ok",
    "database": "error: unable to open SQLite file"
  },
  "uptime_seconds": 86400
}
```

The health check verifies:
- **jwks** — JWKS is loaded and cached (at least one key present)
- **signing_key** — RSA private key is loaded and usable
- **database** — SQLite database file is accessible

The service returns `503` if any check fails. Individual check status is reported so the caller can distinguish a JWKS failure from a database failure.

---

## Error Envelope

All error responses follow a consistent flat shape:

```json
{
  "error": "invalid_signature",
  "message": "JWT signature verification failed",
  "detail": "kid 'abc123' not found in JWKS",
  "requestId": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": "2026-05-17T12:00:00Z"
}
```

| Field | Type | Description |
|---|---|---|
| `error` | string | Machine-readable error code (lowercase snake_case) |
| `message` | string | Human-readable description |
| `detail` | string | Optional additional context (may be null) |
| `requestId` | string | Unique request identifier (for log correlation) |
| `timestamp` | string | ISO 8601 timestamp of the error |

**Security note:** Error details must not leak sensitive information. For validation failures, the message describes the failure category but never reveals the token content, key material, or internal configuration values.

---

## Request ID

Every request is assigned a UUID at the entry point (via middleware). This ID is:
- Logged to SQLite with the request
- Included in the error response envelope as `requestId`
- Available in response headers as `X-Request-Id` for successful responses

```
X-Request-Id: 550e8400-e29b-41d4-a716-446655440000
```

---

## Cross-references

- Init design: `design-001-init.md` — overall architecture
- Token mapping: `design-003-token-mapping.md` — claim transformation rules (invoked by `/api/v1/exchange`)
- JWKS management: `design-004-jwks-management.md` — key caching and validation
- Logging: `design-005-logging.md` — audit log schema (populated by all endpoints)
