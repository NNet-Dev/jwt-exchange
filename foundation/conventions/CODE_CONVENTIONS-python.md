# Code Conventions — Python

**Status:** Living document.
**Audience:** Anyone writing Python code in this project — humans and AI assistants generating code.
**Scope:** Python style, file layout, type hints, error handling, testing. Defers to PEP 8 for things not covered here.
**Companion docs:** Other languages have their own `CODE_CONVENTIONS-<language>.md` (e.g. `-powershell.md`, `-rust.md`) when written. For naming of files and folders, see `NAMING_CONVENTIONS.md`. For HTTP API and JSON wire format conventions, see `API_CONVENTIONS.md`.

When this document and PEP 8 conflict, this document wins (this is rare). When this document is silent, PEP 8 applies.

---

## 1. Python style basics

### 1.1 Naming

- **Modules:** `snake_case.py` (`service_engine.py`, `runtime_adapter.py`).
- **Classes:** `PascalCase` (`ServiceEngine`, `Request`).
- **Functions and methods:** `snake_case` (`acquire_lock`, `provision`).
- **Variables:** `snake_case` (`item_key`, `request_id`).
- **Constants:** `SCREAMING_SNAKE_CASE` (`DEFAULT_TTL_SECONDS`, `MAX_BUDGET_USD`).
- **Private/internal:** prefix with single underscore (`_resolve_id`, `_format_response`). Reserve double underscore for true name mangling, which is rarely needed.

### 1.2 Indentation and line length

- 4 spaces, no tabs.
- 100-character line limit (slightly more permissive than PEP 8's 79).
- Don't enforce mid-expression line breaks for readability sake; a 90-char line that reads cleanly is better than four 30-char lines.

### 1.3 Imports

Order:
1. Standard library imports.
2. Third-party imports.
3. Local application imports.

Each group separated by a blank line. Within a group, alphabetize.

```python
from __future__ import annotations

import json
import os
import sqlite3
from datetime import datetime, timezone
from typing import Any

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, Field

from db import get_connection
from models import ItemCreate
```

`from __future__ import annotations` at the top of every module that uses type hints — it makes `str | None` and `list[X]` work correctly on older Python and avoids forward-reference issues.

### 1.4 String formatting

f-strings everywhere. Don't use `.format()` or `%` formatting unless there's a specific reason (e.g., logging arguments via `%s` for lazy interpolation).

```python
# Good
log.info(f"Acquired lock for {item_key}")

# Bad
log.info("Acquired lock for {}".format(item_key))
log.info("Acquired lock for %s" % item_key)
```

For SQL, **never** use f-strings to interpolate values into queries. Always use parameterized queries:

```python
# Good
conn.execute("SELECT * FROM items WHERE id = ?", (item_id,))

# Bad — SQL injection vector
conn.execute(f"SELECT * FROM items WHERE id = '{item_id}'")
```

The exception is for table or column names that come from a controlled set, and even then, validate against a whitelist.

---

## 2. Type hints

Type hints are required on:
- Public function and method signatures (parameters and return type).
- Module-level constants.
- Class attributes (use dataclass-style or explicit annotations).

Type hints are encouraged but not required on:
- Internal/private function bodies.
- Local variables (only annotate when type isn't obvious from context).

### 2.1 Modern syntax

Use Python 3.10+ syntax:

```python
# Good
def lookup(item_key: str, fallback: str | None = None) -> dict | None:
    ...

# Avoid (older syntax, less readable)
from typing import Optional, Union, Dict
def lookup(item_key: str, fallback: Optional[str] = None) -> Optional[Dict]:
    ...
```

`X | None` reads better than `Optional[X]`. `list[X]` reads better than `List[X]`. Use the modern forms.

### 2.2 Generic containers

Annotate generics:

```python
def list_items(...) -> tuple[list[dict], int]:    # not just `tuple`
def get_history(...) -> dict[str, list[Event]]:    # not just `dict`
```

When you genuinely don't know or care about the inner type, use `Any` explicitly rather than leaving generics bare:

```python
def parse_arbitrary(data: bytes) -> dict[str, Any]:
    ...
```

### 2.3 Dataclasses for structured records

For records with named fields, use `@dataclass` (or Pydantic models for API boundaries):

```python
from dataclasses import dataclass

@dataclass
class StageOutput:
    stage_name: str
    artifact_id: str
    cost_usd: float
    duration_seconds: float
```

Use `frozen=True` if the record is immutable after creation:

```python
@dataclass(frozen=True)
class CallerIdentity:
    consumer: str
    user: str
    trust_level: TrustLevel
```

### 2.4 Enums

```python
from enum import Enum

class ItemStatus(str, Enum):
    DRAFT = "draft"
    PENDING_REVIEW = "pending_review"
    APPROVED = "approved"
    ARCHIVED = "archived"
```

Inherit from `str, Enum` so values serialize as strings (matches DB storage and JSON wire format). Values are lowercase snake_case (matches API enum convention).

---

## 3. Project shape

The project's shape determines its layout. Three shapes are common in this ecosystem; the principle is the same in each — **separate the layers, talk only downward, never upward**.

### 3.1 Service shape (HTTP API + persistence)

```
src/<project>/
  routes/        ← HTTP endpoints. Thin. Calls services.
  services/      ← Business logic. Calls db. No HTTP knowledge.
  db/            ← Database connection management. Called by services.
  models.py      ← Pydantic request/response models. Used by routes only.
  main.py        ← FastAPI app construction; mounts routes.
```

### 3.2 CLI shape (command-line tool)

```
src/<project>/
  commands/      ← Command implementations (one per subcommand).
  services/      ← Business logic. Called by commands.
  io/            ← File/network/stdout/stderr handling.
  cli.py         ← Click/Typer/argparse entry; thin dispatch to commands.
  __main__.py    ← Enables `python -m <project>`. Calls cli.py.
```

Use **Typer** or **Click** for any non-trivial CLI. Both are standard, both produce good `--help` output, both make subcommand grouping straightforward. Plain `argparse` is fine for tiny scripts (one command, a few flags); past that, Typer is the default.

`cli.py` is the entry — argument parsing, dispatch, `--help` text. Real work lives in `commands/`. A `cli.py` longer than ~50 lines is a sign that command logic has leaked in; extract it.

### 3.3 Library shape (importable package)

```
src/<project>/
  __init__.py    ← Public API surface; explicit re-exports.
  _internal/     ← Private modules. Underscore prefix on the folder.
  models.py      ← Public types (dataclasses, Pydantic).
  errors.py      ← Public exception classes.
  py.typed       ← Empty marker file declaring inline type hints.
```

Public API discipline:

- Everything importable from `<project>.<thing>` is part of the public API.
- Anything starting with `_` (underscore) is private — single-underscore for module/function, leading-underscore for folders (`_internal/`).
- `__init__.py` explicitly imports and re-exports the public surface; `__all__` lists it.
- Including a `py.typed` marker file is required so consumers' type-checkers honor the inline hints.

The library shape's contract with consumers is the import surface. Removing or renaming a public symbol is a breaking change in the sense of `../designs/design-000-meta.md` §3.

### 3.4 The shared layered principle

Whatever the shape, layers talk only downward:

- **Routes/Commands/Public-API** — entry points. Thin. Validate input, dispatch, format output.
- **Services** — business logic. Stateless. No knowledge of the entry layer.
- **Storage/IO** — persistence and external IO. Called by services.

Skipping layers (a route calling the database directly, a public-API function dropping straight into a private internal helper) is the smell. The discipline that follows applies to whichever layer name fits your shape.

### 3.5 Routes (service shape)

Route handlers are thin. They:
- Validate input (via Pydantic).
- Delegate to a service function.
- Format the response.
- Translate service-layer exceptions to HTTP errors.

Routes do **not**:
- Contain SQL.
- Contain multi-step business logic.
- Call other route handlers directly.
- Access the database directly.

If a route is more than ~30 lines, it's probably doing too much; extract logic into a service function.

```python
@router.post("/items", status_code=201)
def create_item(body: ItemCreate):
    try:
        item_id = item_svc.create_item(
            item_key=body.item_key,
            payload=body.item_payload,
            status=body.status,
        )
    except DuplicateItemKeyError as e:
        raise _error_response("ITEM_KEY_CONFLICT", str(e), status_code=409)

    return _format_item_response(item_svc.get_item(item_id))
```

### 3.6 Services

Service modules are stateless. Each function:
- Takes typed parameters.
- Returns typed values.
- Uses `db.get_connection()` to access the database.
- Raises typed exceptions for caller to interpret.

Services do **not**:
- Know about HTTP, FastAPI, or request/response shapes.
- Import from `routes/`.
- Maintain global state.
- Call other services that import them (one-way dependency, no cycles).

Function signature pattern:

```python
def acquire_lock(
    item_key: str,
    locked_by: str,
    ttl_seconds: int = 600,
) -> bool:
    """Attempt to acquire a creation lock.

    Returns True if acquired, False if already held.
    Stale locks are auto-released.
    """
    ...
```

Note the docstring — every public service function gets a one-line summary plus brief detail of return semantics. This is the contract; readers shouldn't have to read the body to understand what it returns.

### 3.7 Database layer (service shape)

`db.py` provides connection management:

```python
@contextmanager
def get_connection() -> Generator[sqlite3.Connection, None, None]:
    conn = raw_connection()
    try:
        yield conn
        conn.commit()
    finally:
        conn.close()
```

Use `with get_connection() as conn:` in service code. The context manager handles commit-on-success and close-on-exit. Don't manage connections manually.

### 3.8 No SQL in routes

This is worth its own line. Route handlers never call `db.execute()` directly. SQL belongs in services. If a route needs data, it calls a service function that returns the data.

There are no exceptions. If a route appears to "need" inline SQL — most commonly for a quick FK existence check before delegating — that's a smell. The right move is a service function that does the check (`item_svc.item_exists(item_id)` returning `bool`), called from the route. Inline SQL in routes is one of the bootstrap-residue patterns the design philosophy explicitly flags; don't reproduce it.

---

## 4. Error handling

### 4.1 Use typed exceptions in services

```python
class ServiceError(Exception):
    """Base class for service errors."""

class DuplicateItemKeyError(ServiceError):
    """An item_key already exists."""

class InvalidStatusTransitionError(ServiceError):
    """Status transition is not allowed."""

class ItemNotFoundError(ServiceError):
    """Item does not exist."""
```

Services raise these. Routes catch them and translate to HTTP errors (per `API_CONVENTIONS.md`).

```python
# In service
def update_item(item_id: str, status: str) -> dict:
    current = _get(item_id)
    if status not in ITEM_TRANSITIONS[current.status]:
        raise InvalidStatusTransitionError(
            f"Cannot transition from {current.status} to {status}"
        )
    ...

# In route
try:
    result = item_svc.update_item(item_id, status=status)
except InvalidStatusTransitionError as e:
    raise _error_response("ITEM_INVALID_TRANSITION", str(e), status_code=409)
```

### 4.2 Don't catch and re-raise without value

```python
# Bad — adds nothing
try:
    do_thing()
except Exception:
    raise

# Bad — silently swallows
try:
    do_thing()
except Exception:
    pass

# Good — converts to a typed error
try:
    do_thing()
except sqlite3.IntegrityError as e:
    raise DuplicateItemKeyError(f"Duplicate item_key: {e}") from e
```

Use `raise X from e` to preserve the original exception chain when re-raising as a different type.

### 4.3 Don't use generic exceptions for control flow

```python
# Bad
try:
    parse_thing(x)
    return "ok"
except Exception:
    return "fail"

# Good
result = try_parse(x)
if result is None:
    return "fail"
return "ok"
```

Exceptions are for exceptional conditions, not for routine "is this thing valid" checks.

---

## 5. SQL conventions

### 5.1 Style

```python
conn.execute("""
    SELECT id, item_key, status
    FROM items
    WHERE item_key = ?
    ORDER BY created_at DESC
    LIMIT 1
    """,
    (item_key,)
)
```

- SQL keywords UPPERCASE (`SELECT`, `FROM`, `WHERE`).
- Table and column names lowercase, snake_case (matching DB convention).
- Parameterized queries always; never string interpolation.
- Multi-line queries use triple-quoted strings with a leading newline so the SQL aligns cleanly.

### 5.2 Always close (or use context manager)

```python
# Preferred
with get_connection() as conn:
    rows = conn.execute("SELECT ...").fetchall()

# Acceptable for one-off scripts
conn = sqlite3.connect(...)
try:
    rows = conn.execute("SELECT ...").fetchall()
finally:
    conn.close()
```

Never leave a connection unclosed; under SQLite's WAL mode, this can cause file-handle exhaustion under load.

### 5.3 Transactions

For multi-statement operations that must be atomic, use explicit transactions:

```python
with get_connection() as conn:
    try:
        conn.execute("BEGIN IMMEDIATE")
        conn.execute("INSERT INTO items (...) VALUES (...)", (...))
        conn.execute("INSERT INTO item_versions (...) VALUES (...)", (...))
        conn.execute("UPDATE items SET current_version_id = ? WHERE id = ?", (...))
        conn.commit()
    except Exception:
        conn.rollback()
        raise
```

`BEGIN IMMEDIATE` acquires the write lock upfront, preventing SQLITE_BUSY errors from concurrent readers during the transaction. Use it for any transaction with more than one write.

---

## 6. Async vs sync

The default is **synchronous**. Reasons:

- SQLite drivers are synchronous; converting to async via `run_in_executor` adds complexity without throughput gain.
- Service layer functions are synchronous (`def acquire_lock(...) -> bool`).
- FastAPI handles the sync-to-async transition automatically — sync route handlers run in a thread pool.

Use async only when:
- You're calling an actually-async API (HTTP via httpx, etc.).
- The route involves multiple concurrent external calls that benefit from parallelism.

Service-layer DB code stays sync.

---

## 7. Testing

### 7.1 Layout

```
tests/
  conftest.py              ← shared fixtures
  test_routes.py           ← API-level tests
  test_service.py          ← service-level unit tests
  test_schema.py           ← schema correctness tests
  test_concurrency.py      ← concurrency-specific tests (when relevant)
```

One test file per module under test, named `test_<module>.py`.

### 7.2 Fixture scope

Use function-scoped fixtures by default (`@pytest.fixture` without `scope=`). This isolates tests and prevents leakage. Use session-scoped only when fixture creation is genuinely expensive.

### 7.3 Test naming

Test names describe the behavior:

```python
def test_acquire_lock_succeeds_for_unheld_item_key():
    ...

def test_acquire_lock_fails_when_already_held():
    ...

def test_acquire_lock_succeeds_when_existing_lock_is_stale():
    ...
```

Not:

```python
def test_acquire_lock_1():       # bad — meaningless number
def test_lock():                  # bad — too vague
def TestAcquireLock_HappyPath():  # bad — wrong naming convention
```

### 7.4 Error path coverage

Every happy path needs at least two error path tests covering:
- Bad input (validation failure).
- Resource conflict (duplicate, lock held, invalid transition).

This is a hard rule, not a guideline. Error-path gaps are defects.

---

## 8. Documentation

### 8.1 Module docstrings

Every Python module starts with a docstring:

```python
"""Item Service — Business Logic Layer.

Handles item creation, lookup, status transitions, and lock management.

Layer: services. Imports from db only. No HTTP knowledge.
"""
```

State the module's purpose in one line, optionally followed by clarifying detail.

### 8.2 Function docstrings

Public functions get docstrings:

```python
def acquire_lock(item_key: str, locked_by: str, ttl_seconds: int = 600) -> bool:
    """Attempt to acquire a creation lock.

    Returns True if acquired, False if already held.
    Stale locks (past expires_at) are automatically force-released.
    """
```

One-line summary. Blank line. Optional detail. Document return semantics. Don't document parameters individually unless their meaning isn't obvious from name and type.

### 8.3 Inline comments

Comments explain **why**, not **what**:

```python
# Bad — restates the code
i += 1  # increment i

# Good — explains the why
# Off-by-one: indices in this dataset are 1-based per the upstream spec.
i += 1
```

If the code itself isn't self-explanatory, often the right fix is renaming a variable or extracting a function, not adding a comment.

---

## 9. Why the conventions matter even in a single project

It's tempting to think that conventions only matter when multiple people or multiple systems collaborate. They matter inside a single project too, for two reasons:

- **AI assistants (Claude, Copilot, etc.) generate code that follows the conventions of the code they see.** Drifty conventions produce drifty generated code. Tight conventions produce tight generated code.
- **Future-you is a different collaborator.** The code you'll be reading in six months was written by someone with different context. Conventions are the shared assumptions that let future-you understand what you wrote.

The conventions documents (this one plus `API_CONVENTIONS.md` and `NAMING_CONVENTIONS.md`) are the contract between past, present, and future contributors — human and AI. Reference them in design docs rather than restating them. When they need to change, update the doc and any in-flight designs that depend on them.

---

## 10. Cross-references

- For naming of files and folders (including `.py` modules and SQL migrations): `NAMING_CONVENTIONS.md`.
- For HTTP API and JSON wire format conventions (including the Pydantic alias seam this doc mentions): `API_CONVENTIONS.md`.
- For other languages used in the project: `CODE_CONVENTIONS-<language>.md` (one per language; `-powershell.md`, `-rust.md`, etc., as they're added).
- For the design values that motivate code-level discipline: `../DESIGN_PHILOSOPHY.md` (especially "Conventions are constitutional" and "Bootstrap-residue is a smell").
- For the structural foundation that governs design docs and contracts: `../designs/design-000-meta.md`.
