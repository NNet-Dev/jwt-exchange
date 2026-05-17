---
app: jwt-exchange
owner: Marc
status: Draft
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
| `POST` | `/exchange` | Accept inbound JWT, return Qlik-compatible JWT | None |
| `GET` | `/cert` | Return public X.509 certificate (PEM) | None |
| `GET` | `/health` | Health check (startup + readiness probe) | None |

---

## POST /exchange

### Request

**Content-Type:** `application/json`

```json
{
  "token": "<base64-encoded inbound JWT>"
}
```

Alternatively, the token can be sent in the `Authorization` header:

```
Authorization: Bearer <inbound JWT>
```

If both are present, the header takes precedence.

**Note:** The token is the raw JWT string (three base64url segments joined by dots), not a wrapped object. The JSON body approach exists for clients that prefer POST bodies over headers.

### Response — Success (200)

```json
{
  "access_token": "<base64-encoded Qlik JWT>",
  "token_type": "Bearer",
  "expires_in": 3600,
  "issued_at": "2026-05-17T12:00:00Z"
}
```

| Field | Type | Description |
|---|---|---|
| `access_token` | string | The minted Qlik-compatible JWT |
| `token_type` | string | Always `"Bearer"` |
| `expires_in` | integer | Token TTL in seconds |
| `issued_at` | string | ISO 8601 timestamp of token issuance |

### Response — Errors

All errors use the standard error envelope (see § Error Envelope below).

| Status | Error Code | Scenario |
|---|---|---|
| `400` | `MISSING_TOKEN` | No token provided in body or header |
| `400` | `MALFORMED_TOKEN` | Token is not a valid JWT structure (not 3 segments) |
| `401` | `INVALID_SIGNATURE` | JWT signature does not match any key in JWKS |
| `401` | `EXPIRED_TOKEN` | JWT `exp` claim is in the past |
| `401` | `INVALID_ISSUER` | JWT `iss` does not match configured IdP issuer |
| `401` | `INVALID_AUDIENCE` | JWT `aud` does not match expected audience (if configured) |
| `401` | `UNKNOWN_KID` | JWT `kid` not found in JWKS and refresh also didn't find it |
| `500` | `INTERNAL_ERROR` | Unexpected server error |
| `503` | `IDP_UNAVAILABLE` | Failed to fetch IdP well-known config or JWKS at startup/retry |

### Rate limiting

No rate limiting at the service level. If needed, apply at the reverse proxy/load balancer layer.

---

## GET /cert

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

The response is the raw PEM-formatted X.509 public certificate. This is designed to be copy-pasteable into the QMC virtual proxy's "JWT certificate" field, or programmatically consumed by automation.

### Response — Errors

| Status | Error Code | Scenario |
|---|---|---|
| `500` | `CERT_NOT_LOADED` | Public certificate file not found or unreadable at startup |

---

## GET /health

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

All error responses follow a consistent shape:

```json
{
  "error": {
    "code": "INVALID_SIGNATURE",
    "message": "JWT signature verification failed",
    "detail": "kid 'abc123' not found in JWKS",
    "request_id": "req-a1b2c3d4",
    "timestamp": "2026-05-17T12:00:00Z"
  }
}
```

| Field | Type | Description |
|---|---|---|
| `error.code` | string | Machine-readable error code (uppercase snake_case) |
| `error.message` | string | Human-readable description |
| `error.detail` | string | Optional additional context (may be null) |
| `error.request_id` | string | Unique request identifier (for log correlation) |
| `error.timestamp` | string | ISO 8601 timestamp of the error |

**Security note:** Error details must not leak sensitive information. For validation failures, the message describes the failure category but never reveals the token content, key material, or internal configuration values.

---

## Request ID

Every request is assigned a UUID at the entry point (via middleware). This ID is:
- Logged to SQLite with the request
- Included in the error response envelope
- Available in response headers as `X-Request-Id` for successful responses

```
X-Request-Id: req-a1b2c3d4-e5f6-7890-abcd-ef1234567890
```

---

## Cross-references

- Init design: `design-001-init.md` — overall architecture
- Token mapping: `design-003-token-mapping.md` — claim transformation rules (invoked by `/exchange`)
- JWKS management: `design-004-jwks-management.md` — key caching and validation
- Logging: `design-005-logging.md` — audit log schema (populated by all endpoints)
