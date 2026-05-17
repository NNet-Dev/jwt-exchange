# API Conventions

**Status:** Living document. Update when conventions change; never deviate without an entry here.
**Audience:** Anyone designing or implementing HTTP APIs in this project.
**Scope:** Wire format, naming, status codes, error shape, pagination, versioning, and the seam between API and code.

When an API design or implementation has to choose between this document and another source, this document wins. When this document is wrong or unclear, fix this document first, then propagate the change.

This doc is most relevant once the project has an HTTP API surface. Until then, the headline rule is enough: **camelCase JSON, kebab-case URLs, snake_case code, with a single translation seam (Pydantic for Python).** When the API actually lands, this doc's full machinery is here waiting.

---

## 1. The headline rule

**Wire format is camelCase. URL paths and query parameters are kebab-case. Code is snake_case. The Pydantic model layer is the only place that translates between them.**

That sentence resolves most decisions. The rest of this document is the long form of why and how.

---

## 2. Wire format (request/response bodies)

### 2.1 JSON keys are camelCase

Every field in every request body and response body uses **camelCase**:

```json
{
  "itemKey": "abc-123",
  "itemPayload": { ... },
  "createdAt": "2026-04-27T09:00:00Z"
}
```

Not snake_case. Not PascalCase. Not SCREAMING_SNAKE.

This matches the convention used by Stripe, GitHub, Slack, and most modern APIs. JS consumers expect it; mobile SDKs expect it; OpenAPI tooling generates it cleanly. snake_case in JSON is a Python-codebase smell that leaks into the wire format when the convention isn't enforced.

### 2.2 Date-times are ISO 8601 UTC strings

```json
{
  "createdAt": "2026-04-27T09:00:00Z",
  "expiresAt": "2026-04-27T09:10:00Z"
}
```

Always UTC. Always with the `Z` suffix. Always strings, never Unix timestamps. The timestamps in the database use the same format (`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')` for SQLite, `to_char(... , 'YYYY-MM-DD"T"HH24:MI:SS"Z"')` for Postgres); the API layer doesn't need to translate.

### 2.3 IDs are typed prefixed strings

Pattern: short type prefix, underscore, identifier body.

```
itm_a1b2c3d4    ← item
usr_e5f6a7b8    ← user
req_a1b2c3d4    ← request
```

The prefix tells you what kind of resource it is at a glance. Pure UUIDs lose that affordance.

If you're adding a new resource type, pick a 2–3 letter prefix that doesn't collide with existing ones. Document it in the design doc that introduces the resource.

### 2.4 Enums are lowercase snake_case strings

```json
{
  "status": "pending_review",
  "verdict": "pass",
  "policy": "auto_retry"
}
```

Lowercase. snake_case for multi-word values. No capitalization. No mixed case.

This makes enum values easily greppable in logs and CHECK constraints in SQL, and matches the database storage convention. The exception is when the value is a recognized string from an external system — `claude-sonnet-4-7` is a model identifier, not an enum, and follows that system's convention.

### 2.5 Optional fields are omitted or null

```json
{
  "itemKey": "abc-123",
  "ownerId": null,
  "ownerName": null
}
```

Either the field is present with a value, or it's `null`, or it's absent. Don't use empty strings or `0` as sentinel values for "not set."

For request bodies, prefer omitting optional fields entirely. For response bodies, include them as `null` so consumers can rely on the field always being present in the schema.

### 2.6 Lists use plural nouns; singletons use singular

```json
{
  "items": [...],          // list
  "subItems": [...],
  "owner": {...}           // singleton
}
```

`items` (plural) for the list endpoint, `item` (singular) when referring to a specific one. This is conventional but easy to violate; check before merging.

---

## 3. URLs

### 3.1 Path segments are kebab-case

```
/api/v1/items
/api/v1/sub-items
/api/v1/admin/feature-flags
```

Not `/api/v1/subItems`. Not `/api/v1/sub_items`.

URLs should read like prose. Kebab-case does that; the others don't.

### 3.2 Resources are plural

```
/api/v1/items              ← list and create
/api/v1/items/{itemId}     ← get, update, delete one
```

Even when there's only one resource of a kind expected, the collection is plural. This is the REST convention; deviating from it is rare and should be deliberately justified in a design doc.

### 3.3 Sub-resources are nested

```
/api/v1/items/{itemId}/comments        ← comments belong to an item
/api/v1/users/{userId}/sessions        ← sessions belong to a user
```

Nesting is one level deep typically, two at most. If you find yourself wanting three-level nesting, the resource model probably needs flattening — there's an implicit query you should be exposing as a query parameter instead.

### 3.4 Query parameters are snake_case

```
GET /api/v1/items?status=approved&offset=0&limit=50
GET /api/v1/items?owner_id=usr_a1b2c3d4&status=pending_review
```

Yes, this is inconsistent with the camelCase JSON bodies. It's deliberate: query params are part of the URL, and URLs are kebab/snake-case territory. They get URL-encoded, they appear in logs, they're written by hand in test scripts. snake_case is friendlier for that than camelCase.

The Pydantic model layer can accept either via `populate_by_name=True`, but the documented contract is snake_case in query strings.

### 3.5 Path parameters are snake_case

```
/api/v1/items/{item_id}
/api/v1/users/{user_id}/sessions/{session_id}
```

Same reason as query parameters — path is URL territory, snake_case is the URL idiom.

### 3.6 Verbs in paths are limited and explicit

REST is "nouns and HTTP methods" in theory. In practice, some operations don't fit cleanly. When you need a verb in the path, make it explicit and at the end:

```
POST /api/v1/items/{item_id}/publish
POST /api/v1/items/{item_id}/archive
DELETE /api/v1/sessions/{session_id}/release
```

Don't put verbs at the start (`GET /items/get-item/{id}` is wrong). Don't use generic names (`do`, `process`, `handle`). Use the verb that names the action.

---

## 4. HTTP methods

| Method | Use for | Idempotent? |
|---|---|---|
| GET | Read; never modifies state | Yes |
| POST | Create; trigger non-idempotent action | No |
| PATCH | Update some fields of an existing resource | Yes (typically) |
| PUT | Replace entire resource (use rarely; prefer PATCH) | Yes |
| DELETE | Remove a resource or release a lock | Yes |

Use the right method. Don't `POST /api/v1/users/delete/{id}` because someone read that GETs shouldn't have side effects — use `DELETE /api/v1/users/{id}`.

---

## 5. Status codes

| Code | Meaning | When to use |
|---|---|---|
| 200 | OK | GET success; PATCH/PUT success |
| 201 | Created | POST that creates a new resource |
| 202 | Accepted | POST that queues async work; final result will be delivered later |
| 204 | No Content | DELETE success when there's no body to return |
| 400 | Bad Request | Generic client error; prefer 422 for validation specifically |
| 401 | Unauthorized | Auth missing or invalid (when auth is implemented) |
| 403 | Forbidden | Auth valid but caller lacks permission |
| 404 | Not Found | Resource doesn't exist |
| 409 | Conflict | State conflict — duplicate, lock held by another, invalid state transition |
| 422 | Unprocessable Entity | Schema/format validation failure (request shape was wrong) |
| 429 | Too Many Requests | Rate limit exceeded (when rate limiting is implemented) |
| 500 | Server Error | Unhandled server-side problem |
| 503 | Service Unavailable | Server temporarily unable (DB locked, dependency down) |

Two specific points worth emphasizing:

- **422 vs 400.** 422 is for validation failures: missing required fields, wrong types, pattern mismatches. 400 is for "the request was malformed in some way I can't otherwise classify." Prefer 422 for validation; 400 is a fallback.
- **409 for state conflicts.** Trying to create a duplicate, trying to release a lock you don't hold, trying to modify a finalized resource — these are 409, not 400.

---

## 6. Error response shape

Every error response — regardless of status code — uses the same envelope:

```json
{
  "error": "ITEM_KEY_CONFLICT",
  "message": "An item with key 'abc-123' already exists",
  "details": [
    {
      "field": "itemKey",
      "message": "item_key 'abc-123' is not unique",
      "code": "ITEM_KEY_CONFLICT"
    }
  ]
}
```

Required fields:
- **`error`** — machine-readable code, UPPER_SNAKE_CASE. Programmatic consumers branch on this. (When the project grows enough surface area to warrant prefixes, prefix by area: `AUTH_*`, `BILLING_*`, etc.)
- **`message`** — human-readable description.

Optional:
- **`details`** — array of per-field error specifics. Used for validation errors, present otherwise only when meaningful.

The full taxonomy of error codes lives in a per-feature error model document (e.g., the design doc that introduces the endpoints, or a dedicated `error-model.json` once the codes proliferate).

---

## 7. Pagination

List endpoints support pagination via `offset` and `limit` query params with bounded defaults:

```
GET /api/v1/items?offset=0&limit=50
```

Defaults: `offset=0`, `limit=50`. Maximum: `limit=200`.

Response shape includes a `meta` envelope:

```json
{
  "data": [ ... ],
  "meta": {
    "total": 42,
    "limit": 50,
    "offset": 0
  }
}
```

`total` is the total matching records across all pages, not just the current page. Consumers use it to render "page X of Y" or to know when they've reached the end.

If a list has no pagination requirement (it's always small), still wrap in the same `data`/`meta` envelope so consumers don't special-case different shapes.

---

## 8. Versioning

URLs include the version: `/api/v1/`. Major version changes happen rarely and only when a backward-incompatible change is required. Within a version, additions are non-breaking and don't require coordination.

While there are no external consumers, version bumps are inexpensive — you change the API shape and the (single) consumer at the same time. The discipline still matters: include `/v1/` in URLs from day one, so the moment external consumers appear, the versioning machinery is already in place.

When external consumers exist and v2 becomes necessary, the discipline from `../designs/design-000-meta.md` applies. The relevant rules:

- v1 endpoints continue to work for the deprecation period defined in the meta doc.
- v1 docs are marked deprecated but not removed during the period.
- Each v1 response includes a `Deprecation` header pointing at the v2 equivalent (per RFC 8594).
- A migration guide is published alongside the v2 spec.

URL-segment versioning (`/api/v1/...` vs `/api/v2/...`) is the canonical mechanism. No header-based content negotiation; no query-parameter version selection. The URL is the version.

---

## 9. The Pydantic seam

The translation between API conventions and code conventions happens at exactly one place: the Pydantic model layer.

(This section is Python-specific. For other languages, see the equivalent section in §13.)

### 9.1 Configure all API models with the alias generator

Every Pydantic model used as a request body or response body uses:

```python
from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel

class APIModel(BaseModel):
    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
    )
```

`alias_generator=to_camel` translates Python's snake_case field names to camelCase aliases for serialization. `populate_by_name=True` means the model accepts either the snake_case Python name OR the camelCase alias on input.

Every API model inherits from `APIModel`.

### 9.2 Field names in models are snake_case

Inside the model, field names are snake_case (Python convention). The `alias_generator` produces the camelCase aliases automatically:

```python
class ItemCreate(APIModel):
    item_key: str           # serializes as "itemKey"
    item_payload: dict      # serializes as "itemPayload"
    owner_id: str | None = None  # serializes as "ownerId"
```

### 9.3 The route handler reads camelCase from the wire

```python
@router.post("/items")
def create_item(body: ItemCreate):
    # body.item_key — Python-side, snake_case
    # JSON had {"itemKey": "..."} — Pydantic translated it
    return _format_response(...)
```

Route handlers never see camelCase strings directly. They work with snake_case attributes on validated Pydantic objects. The wire format is the Pydantic layer's job.

### 9.4 Response serialization

When serializing a response, FastAPI uses the alias by default if `model_config` has `populate_by_name=True` and the response is a Pydantic model:

```python
return ItemResponse(
    id=item.id,
    item_key=item.item_key,
    item_payload=item.item_payload,
)
# JSON output: {"id": "...", "itemKey": "...", "itemPayload": {...}}
```

If you're returning a dict directly (not recommended; do this only for ad-hoc shapes), use camelCase keys explicitly. The Pydantic model approach is preferred — it gives you schema validation, OpenAPI generation, and consistent wire format for free.

---

## 10. OpenAPI / contract-first

Every API endpoint should have an OpenAPI 3.1 contract. The contract is the source of truth; route handlers and tests are validated against it.

While there are no external consumers, the OpenAPI spec lives in `contracts/draft/http-api.openapi.yaml` and can churn freely.

When external consumers appear and the full contract lifecycle activates (per `../designs/design-000-meta.md`), the spec promotes to `contracts/v1/http-api.openapi.yaml` and starts following the strict change discipline.

The contract should include:

- All endpoint paths and methods.
- Request body schemas with examples.
- Response body schemas with examples.
- Error response examples for each error code that applies.

The contract uses the conventions in this doc — camelCase JSON, kebab-case URLs, ISO 8601 timestamps, prefixed IDs. When a contract violates conventions, fix the contract.

For Python projects using FastAPI, the auto-generated OpenAPI spec is a useful starting point but is not the contract — the hand-curated spec in `contracts/` is. FastAPI's spec is generated from code; the contract spec is what code is generated against.

---

## 11. Examples — putting it together

### A correctly-shaped POST endpoint

**Path:** `POST /api/v1/items`

**Request body:**
```json
{
  "id": "itm_a1b2c3d4",
  "itemKey": "abc-123",
  "itemPayload": {
    "summary": "An example item"
  },
  "ownerId": null,
  "status": "draft"
}
```

**Response (201 Created):**
```json
{
  "id": "itm_a1b2c3d4",
  "itemKey": "abc-123",
  "itemPayload": { ... },
  "ownerId": null,
  "status": "draft",
  "createdAt": "2026-04-27T09:00:00Z",
  "updatedAt": "2026-04-27T09:00:00Z"
}
```

**Error response (409 Conflict):**
```json
{
  "error": "ITEM_KEY_CONFLICT",
  "message": "An item with key 'abc-123' already exists",
  "details": [
    {
      "field": "itemKey",
      "message": "item_key 'abc-123' is not unique",
      "code": "ITEM_KEY_CONFLICT"
    }
  ]
}
```

Notice:
- Path is plural noun (`items`).
- JSON keys are camelCase (`itemKey`, `itemPayload`, `createdAt`).
- ID is prefixed (`itm_`).
- Status is lowercase enum (`draft`).
- Date-time is ISO 8601 UTC.
- Error envelope has `error`, `message`, `details`.

### A correctly-shaped GET with query params

**Path:** `GET /api/v1/items?owner_id=usr_a1b2c3d4&status=pending_review&offset=0&limit=50`

Notice:
- Path is plural noun.
- Query params are snake_case.
- Enum value is lowercase.

**Response (200 OK):**
```json
{
  "data": [
    {
      "id": "itm_a1b2c3d4",
      "itemKey": "abc-123",
      "status": "pending_review"
    }
  ],
  "meta": {
    "total": 1,
    "limit": 50,
    "offset": 0
  }
}
```

JSON body uses camelCase even though the query that produced it used snake_case. The translation happens at the Pydantic layer.

---

## 12. What this doc does not specify

- **Authentication scheme.** When auth is added, this doc grows a section on credential format and header conventions.
- **Rate limiting.** When added, a `429 Too Many Requests` response shape goes here.
- **Webhook payload shape.** When async callbacks land, the webhook payload conventions go here.
- **Streaming/SSE conventions.** If/when server-sent events arrive, conventions for event format and reconnect behavior land here.

These aren't oversights — they're not needed yet. Add them when the underlying capability lands.

---

## 13. A note on §9 (the Pydantic seam)

Section 9 describes how Python translates between camelCase wire format and snake_case Python identifiers via Pydantic's `alias_generator=to_camel`. This is Python-specific.

When other languages need to consume or produce these APIs, the same wire-format conventions apply (camelCase JSON, kebab-case URLs, etc.) but the translation seam is implementation-specific:

- **TypeScript/JavaScript**: typically uses the wire format directly (camelCase matches JS convention). No seam needed.
- **PowerShell**: typically uses `ConvertTo-Json -CamelCase` and `ConvertFrom-Json` with property-name conversion. Conventions live in `CODE_CONVENTIONS-powershell.md` (when written).
- **Rust**: typically uses `serde` with `#[serde(rename_all = "camelCase")]`. Conventions live in `CODE_CONVENTIONS-rust.md` (when written).
- **C#/.NET**: typically uses `JsonNamingPolicy.CamelCase`. Conventions live in `CODE_CONVENTIONS-csharp.md` (when written).

The wire format is universal; the translation seam is per-language.

---

## 14. Cross-references

- For the structural foundation that governs API contracts: `../designs/design-000-meta.md`.
- For naming of files (including OpenAPI specs and contract files): `NAMING_CONVENTIONS.md`.
- For Python style (including the Pydantic seam in detail): `CODE_CONVENTIONS-python.md`.
- For the design philosophy on contracts and stability: `../DESIGN_PHILOSOPHY.md`.
