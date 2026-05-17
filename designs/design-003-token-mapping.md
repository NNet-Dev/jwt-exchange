---
app: jwt-exchange
owner: Marc
status: Draft
supersedes: []
depends_on: [design-001-init.md]
owns: []
---

# Design 003 — Token Mapping

---

## Purpose

Define the claim mapping rules from the incoming IdP JWT to the outgoing JWT, including the groups whitelist strategy and TTL configuration.

---

## Claim mapping table

| Qlik claim | Source | Rule |
|---|---|---|
| `userid` | Inbound `sub` | Direct copy. The inbound subject identifier becomes the user ID. |
| `userdirectory` | `QLIK_USER_DIRECTORY` env var | If the env var is set, include it. If unset, **exclude from payload entirely**. |
| `name` | Inbound `name` | Copy if present. If absent, omit. |
| `email` | Inbound `email` | Copy if present. If absent, omit. |
| `groups` | POST request body | The client sends an array of group names in the POST body. Each group is validated against the `GROUPS_WHITELIST` env var. Only whitelisted groups are included in the JWT payload. If none match, the `groups` claim is omitted. |
| `iss` | Hardcoded | `"jwt-exchange"` — our service identity |
| `aud` | `QLIK_AUDIENCE` env var | Mandatory. Must match QMC virtual proxy "Intended audience" setting. |
| `exp` | Computed | `min(iat + TOKEN_TTL_SECONDS, inbound_exp)` — capped to inbound token expiry |
| `nbf` | Computed | Same as `iat` (token is valid immediately) |
| `iat` | Computed | Current time at token mint |
| `jti` | Generated | UUID v4 for uniqueness |

---

## Groups whitelist

### Configuration

```
GROUPS_WHITELIST=Qlik_Admins,Qlik_Developers,Sales_Team,Marketing
```

Comma-separated list of allowed group names. Case-sensitive match.

### POST request body

```json
{
  "token": "<Inbound JWT>",
  "groups": ["Qlik_Admins", "Some_Unknown_Group"]
}
```

The client sends the groups it wants in the minted JWT. The service filters this list against the whitelist:

- `Qlik_Admins` → in whitelist → **included**
- `Some_Unknown_Group` → not in whitelist → **excluded**

Resulting JWT payload groups: `["Qlik_Admins"]`

### Design rationale

- The client (not the IdP) specifies which groups are needed. This gives the consuming application control over the group context without needing to parse the IdP's group structure.
- The whitelist is a security boundary — the service never issues a JWT with a group that isn't explicitly allowed.
- If `GROUPS_WHITELIST` is unset or empty, the `groups` claim is always omitted from the JWT payload.

### Empty or missing groups in request

If the POST body omits `groups` or sends an empty array, the `groups` claim is excluded from the JWT payload. This is a valid scenario — not all Qlik users need group membership.

---

## Incoming token claims

The inbound JWT is expected to contain at minimum:

- `iss` — must match `INBOUND_ISSUER_URI`
- `sub` — the user subject identifier
- `exp` — expiration time
- `iat` — issued at time

Optionally:

- `aud` — audience (validated if `INBOUND_EXPECTED_AUDIENCE` env var is set)
- `name` — user's full name
- `email` — user's email address
- `preferred_username`, `given_name`, `family_name` — additional identity claims

### Claim extraction

The incoming token is fully decoded and validated before claim extraction. The `sub` claim is mandatory — if it's missing, the exchange fails with `MALFORMED_TOKEN`. Other optional claims are extracted if present and included in the outgoing token per the mapping table.

---

## TTL strategy

### Configuration

| Env var | Default | Description |
|---|---|---|
| `TOKEN_TTL_SECONDS` | `3600` | TTL for minted Qlik JWT (1 hour) |

### Constraints

The minted token's TTL is capped to the remaining lifetime of the incoming IdP token:

```
exp_out = min(iat + TOKEN_TTL_SECONDS, inbound_exp)
```

This prevents a scenario where the inbound token has expired but the minted token is still valid — the upstream IdP validation would fail on refresh, but the downstream session would remain active.

---

## Outgoing JWT header

```json
{
  "alg": "RS256",
  "typ": "JWT"
}
```

The `alg` is fixed to RS256 — the only algorithm Qlik Sense JWT virtual proxy is guaranteed to support.

---

## Cross-references

- Init design: `design-001-init.md` — overall architecture
- HTTP API: `design-002-http-api.md` — endpoint shapes (the `/exchange` handler invokes this mapping)
- JWKS management: `design-004-jwks-management.md` — key validation (precedes mapping)
- Logging: `design-005-logging.md` — audit log (records the mapping outcome)
