# conventions-local/

This folder holds **project-specific overrides and extensions** to the conventions in `../foundation/conventions/`.

It exists because no central foundation can predict every project's needs. Sometimes a project legitimately diverges — different testing framework, different async stance, different logging choice. When that happens, the deviation is documented here, explicitly and visibly, rather than being hidden in code or implied by example.

## When to add a file here

- A foundation convention says "do X" and this project does Y instead.
- A foundation convention is silent on something this project needs to standardize.
- A foundation convention is technically correct but needs project-specific clarification.

## When NOT to add a file here

- A one-off deviation in a single file (just do it in the file with a comment).
- "We might want to do X someday." Add the override when you do X, not before.
- Anything that should change the foundation itself — open a PR upstream instead.

## Naming and structure

Each file in `conventions-local/` corresponds to a foundation convention file by name:

| Foundation file | Override file (if any) |
|---|---|
| `foundation/conventions/CODE_CONVENTIONS-python.md` | `conventions-local/CODE_CONVENTIONS-python.md` |
| `foundation/conventions/API_CONVENTIONS.md` | `conventions-local/API_CONVENTIONS.md` |
| (any other) | (matching name) |

Files in this folder open with an explicit "Extends X" header naming the foundation file and version, so a reader (human or AI) immediately understands the relationship.

## Example shape

```markdown
# Code Conventions — Python (project-local extensions)

**Extends:** `../foundation/conventions/CODE_CONVENTIONS-python.md`
              (foundation v0.2.1).
**Status:** Living document.

## Overrides

### §3.1 Service shape

This project uses **Litestar** instead of FastAPI. The layered architecture
is the same; the route decorator changes. See `../designs/design-002-http-api.md`
for the full mapping.

### §6 Async vs sync

This project is **async-first** (because Litestar is async-native and the
upstream APIs we call are all async). Service-layer functions are `async def`,
not `def`. The "sync by default" guidance in the foundation does not apply.

## Additions

### Logging

Use `structlog` with the configuration in `src/<project>/log.py`. JSON output
in production, human-readable in dev. See `../designs/design-005-logging.md`
for the rationale.
```

## Precedence

Where the foundation and a `conventions-local/` file conflict, the local file wins. AI assistants and human contributors should read both: foundation for the baseline, local for the deviations.

When the foundation publishes a new version, the "Extends X (foundation v0.2.1)" header in each local file is the reminder to check whether the override is still meaningful. If the foundation now says what your override said, delete the override.
