---
app: jwt-exchange
owner: Marc
status: Active
supersedes: []
depends_on: [design-001-init.md]
owns: []
---

# Design 004 — JWKS Management

---

## Purpose

Define the JWKS (JSON Web Key Set) lifecycle: discovery at startup, caching in memory, periodic refresh, and handling of unknown key IDs during validation.

---

## Discovery

At startup, the service fetches the inbound IdP well-known OIDC configuration:

```
GET {INBOUND_ISSUER_URI}/.well-known/openid-configuration
```

From the response, extract `jwks_uri`:

```json
{
  "issuer": "https://<tenant>.okta.com",
  "jwks_uri": "https://<tenant>.okta.com/oauth2/v1/keys",
  ...
}
```

Then fetch the JWKS:

```
GET {jwks_uri}
```

Response shape:

```json
{
  "keys": [
    {
      "kty": "RSA",
      "alg": "RS256",
      "kid": "68GfudqZumyQ_Ct4t9Z75xiTSEwwwEO-wdvDElaH8A4",
      "use": "sig",
      "e": "AQAB",
      "n": "wCRcOhrvK_yJiDSLqBecoh6AJ3TLzx88..."
    }
  ]
}
```

### Startup failure

If the well-known endpoint or JWKS endpoint is unreachable at startup:
- Log the error.
- Retry with exponential backoff (1s, 2s, 4s, max 30s) for up to 60 seconds.
- If still unreachable after 60s, the service fails to start (exit with error). The service cannot validate tokens without JWKS.

---

## In-memory caching

The JWKS is cached in memory as an `Arc<RwLock<Jwks>>` (or equivalent concurrent structure). This allows:
- Concurrent read access during token validation (read lock held briefly).
- Atomic swap on refresh (write lock briefly held).

### Cache structure

```rust
struct CachedJwks {
    keys: HashMap<String, JsonWebKey>,  // kid → key
    fetched_at: Instant,
    etag: Option<String>,               // ETag for conditional requests
}
```

---

## Periodic refresh

### Mechanism

A background Tokio task runs on a configured interval:
1. Fetch the JWKS from the `jwks_uri`, sending `If-None-Match` with the stored ETag if available.
2. On `200 OK` with new content, atomically swap the in-memory cache and update the ETag.
3. On `304 Not Modified`, skip the cache update (saves bandwidth and parsing).
4. On failure, log the error and **keep the existing cache**. A stale cache is better than no cache.
5. The refresh task runs independently and never blocks request handling.

### ETag / If-None-Match

If the IdP supports ETag headers on the JWKS endpoint (common for CDN-backed endpoints), use them:
- Store the ETag from the last successful fetch.
- Send `If-None-Match` on subsequent fetches.
- On `304 Not Modified`, skip the cache update.

If the endpoint doesn't support ETag, fall back to unconditional fetch.

---

## Unknown kid handling

### The problem

The IdP rotates signing keys. A new key may appear in the JWKS at any time. If a client presents a JWT signed with a key that isn't in our cache yet, validation fails with "unknown kid".

### Strategy: retry with refresh

When signature validation fails because the JWT's `kid` is not found in the cached JWKS:

1. **Log** the unknown kid, available cache KIDs, and the JWKS URI (diagnostic logging).
2. **Force-refresh** the JWKS (fetch from IdP immediately, bypassing the refresh interval).
3. **Retry** validation with the refreshed JWKS.
4. If the kid is now found → validation proceeds normally.
5. If the kid is still not found → return `unknown_kid` error (401).

### Why this works

The IdP's key rotation is not instantaneous. Old keys remain in the JWKS for a grace period (typically 24-48 hours) while new keys are added. By refreshing on unknown kid, we catch keys that were recently added but not yet in our cache.

### Rate limiting the refresh

To prevent abuse (an attacker sending many JWTs with fake kids, triggering refreshes):
- Cooldown period: 60 seconds. If a refresh was triggered within the last 60 seconds, skip the refresh and return the error immediately.
- Max consecutive failures: 5. After 5 consecutive unknown-kid failures within the cooldown window, log a warning and stop refreshing until the cooldown expires.

---

## Startup sequence

```
1. Fetch well-known config
   ├── Success → extract jwks_uri
   └── Failure → retry (exponential backoff, max 60s) → exit if still failing

2. Fetch JWKS from jwks_uri
   ├── Success → cache in memory, start refresh task
   └── Failure → retry (same backoff) → exit if still failing

3. Service ready → start HTTP server
```

---

## Cross-references

- Init design: `design-001-init.md` — overall architecture
- HTTP API: `design-002-http-api.md` — error codes (`unknown_kid`, `invalid_signature`)
- Token mapping: `design-003-token-mapping.md` — validation precedes mapping
- Logging: `design-005-logging.md` — JWKS fetch failures are logged as `idp_unavailable`
