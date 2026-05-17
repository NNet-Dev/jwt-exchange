# design-000-meta.md

**Status:** Draft
**Supersedes:** none (founding document)
**Depends on:** none

## Purpose

This is the structural foundation document for this project. It establishes the design-doc discipline, the contract lifecycle (in light and full modes), and the change-management rules that govern every design decision and contract change that follows.

It is deliberately light on domain detail. The init doc (`design-001-init.md`) and subsequent designs build on this foundation. This document is what they all assume.

If the structural decisions captured here are wrong, future work needs revision. They are therefore worth getting right early.

This document describes a lighter version of the discipline used by the wider engineering ecosystem this project is adapted from. The full ecosystem version assumes multiple sibling repos with independent release cadences and external consumers depending on shared contracts; this project is one repo with no current external consumers, so most of the heavy machinery is deferred. The "upgrade path" section below describes how to graduate to the full discipline if and when external consumers appear.

---

## 1. Design docs

### 1.1 Numbering and naming

Design docs are numbered sequentially: `design-NNN-<category>[-<specific>].md`, starting at `001`. (This doc is `000` because it precedes the init.)

Naming and category vocabulary live in `conventions/NAMING_CONVENTIONS.md` §2. Don't restate them here.

The first design doc is `design-001-init.md`. Every project's design history starts there.

### 1.2 Required frontmatter

Every design doc begins with a **YAML frontmatter** block delimited by `---` lines.
This block is machine-readable and consumed by intake agents and build orchestrators
during the intake stage (stage 01 of the build-orchestrator pipeline).

**Minimal tier — human-written (required):**

| Field | Purpose | Example |
|---|---|---|
| `app` | Scopes the design bundle; identifies which app's init doc is primary | `order-service` |
| `owner` | Attribution and convention routing | `team-alpha` |
| `status` | Intake filters `Superseded` docs and flags `Draft` as unstable | `Draft`, `Active`, `Superseded` |
| `supersedes` | Replacement chain (empty array if none) | `[]` or `[design-001-init.md]` |
| `depends_on` | Reference graph; intake agent verifies paths exist | `[]` or `[design-002-architecture.md]` |
| `owns` | Architect stage reads this for `ownsDatabase`/`exposesHttpApi` flags | `[]`, `[database]`, `[database, http-api]` |

**Discovered tier — agent-generated (optional, backfilled by skill):**

The `build:` and `deployment:` sections are populated automatically by a
`discover-build-meta` skill that scans the codebase for toolchain files,
entry points, and deployment manifests. Humans should not edit these by hand;
re-run the skill after significant project changes.

| Field | Purpose |
|---|---|
| `build.languages` | Runtimes and versions for provisioning |
| `build.tool` | Build tool identifier (`uv`, `cargo`, `npm`, `dotnet`) |
| `build.entry_points` | Where execution starts (path, type, port) |
| `build.commands` | Standardized phase commands (`install`, `test`, `build`, `lint`) |
| `build.artifacts` | What gets produced (`docker_image`, `python_package`, `binary`) |
| `deployment.target` | Where it runs (`kubernetes`, `vm`, `serverless`, `local`) |
| `deployment.config_via` | Configuration mechanism (`env_vars`, `files`, `secrets`) |
| `deployment.requires_env` | Required environment variable names |

**Template:**

```yaml
---
app: [app key]
owner: [team or person]
status: Draft
supersedes: []
depends_on: []
owns: []

# === AGENT-DISCOVERED (do not edit manually) ===
# build:
#   languages:
#     - name: python
#       version: "3.12"
#   tool: uv
#   entry_points: []
#   commands:
#     install: "uv sync"
#     test: "uv run pytest"
#   artifacts: []
# deployment:
#   target: kubernetes
#   config_via: env_vars
#   requires_env: []
---
```

### 1.3 Required sections (after frontmatter)

After the frontmatter, most docs include:

- **Purpose** — what this doc is for, who should read it.
- **The design itself** — whatever sections best convey the decision.
- **Open questions** — things deliberately deferred, with enough context that a future doc can pick them up.
- **Cross-references** — pointers to related docs.

### 1.4 Lifecycle

A design doc's `Status` field tracks where it is:

- **Draft**: in progress; conclusions may shift. Other docs should not depend on its specifics.
- **Active**: the current decision. Other docs depend on this.
- **Superseded**: replaced by a newer doc. Kept for history; not referenced by current docs except via `Supersedes:` chains.

When a design changes substantially, the right move is usually to write a new design doc that supersedes the old one, not to edit the old one in place. Edits to an Active doc are allowed for minor corrections (typos, broken links, clarifications); substantive change writes a new doc.

### 1.5 Bug Fix Documentation

When an agent performs a bug fix, it must document the change so the build remains reproducible and future builds do not repeat the same mistake. The documentation requirements depend on the nature of the fix:

**Implementation bug (design was correct, code was wrong):**
- Add a CHANGELOG entry.
- Document: symptom, root cause, and files changed.
- No design doc change needed — the design describes intent; the bug was in execution.

**Design gap (the design was incomplete, wrong, or made incorrect assumptions):**
- Write a new design doc (e.g. `design-NNN-operational-gaps.md`) categorised as a post-mortem or gap analysis.
- Document for each gap: the gap description, its symptom, root cause, the fix applied, and the recommended design-side change for future builds.
- The new doc references affected designs via `depends_on` in its frontmatter but does **not** `supersedes` them — the original designs remain the source of truth for intended behavior; the new doc records where they fell short.
- Add a CHANGELOG entry referencing the new design doc.

**Minor clarification (typo, broken link, ambiguous wording):**
- Edit the design doc in place.
- Note the correction at the bottom of the relevant section or in a brief "Errata" note.

**CHANGELOG entry format for bug fixes:**

```markdown
### Fixed

- **DEFECT-XXX (live)**: Brief description of the symptom. Root cause
  explanation. What was changed. (`path/to/file`)
```

The `(live)` tag indicates the defect was discovered in a running environment (post-build, post-deploy). Use `(build)` for defects caught during the build process. Number defects sequentially within the release (DEFECT-001, DEFECT-002, …).

**Verification:**

Every bug-fix release should record what was verified — tests run, end-to-end checks performed, or manual validation steps — so the next agent knows what "done" means.

### 1.6 Open questions

Open questions go in the doc that creates them. They get resolved by a later doc (which references them in `Supersedes:` or in its own body) or by a small follow-up edit (which notes the resolution at the bottom of the open-questions section).

Don't let open questions accumulate without ownership. If a question has been open for several design cycles, it's either important enough to deserve its own doc, or unimportant enough to delete.

---

## 2. Contracts (light mode)

While this project has no external consumers, contract discipline is light:

- **In-flux specs live in `contracts/draft/`.** They can be edited freely. Nothing external depends on them.
- **Internal module contracts** (the function signatures, protocol classes, and data shapes that modules use to talk to each other) are documented inline (docstrings, type hints, dataclass declarations). They're contracts in spirit; the discipline is just to keep them honest — declared shapes, real types.
- **JSON Schemas, OpenAPI specs, Python Protocols** that describe wire surfaces or pluggable interfaces live in `contracts/draft/`. They evolve alongside the code.

There is no `CHANGELOG.md` for contracts in light mode — internal-only contracts don't need one. Significant contract changes are described in the design doc that motivates them.

The shape of contract files (filenames, `.schema.json` and `.openapi.yaml` suffixes, `.protocol.py`, etc.) follows `conventions/NAMING_CONVENTIONS.md` §2.5 even in light mode. Consistent naming costs nothing now and saves real grief later.

---

## 3. Contracts (full mode)

When this project gains external consumers — anything that depends on a wire surface this project exposes and would break if that surface changes — the contracts folder graduates from draft-only to versioned.

Full mode means:

```
contracts/
  README.md
  CHANGELOG.md
  draft/                           # in-flux, not depend-on-able
  v1/                              # promoted, depend-on-able
    http-api.openapi.yaml
    request-shape.schema.json
  archive/                         # past versions removed from active service
```

Once a contract is in `contracts/v<N>/`, the rules below apply.

### 3.1 The promotion event

Promotion-from-draft is when consumers can begin to depend. It is a deliberate decision, not an automatic consequence of a contract feeling "done."

Criteria for promotion:

- The shape is settled and unlikely to need breaking changes for at least one major-version cycle.
- The producer (this project) can commit to honoring the deprecation period for this version.
- A CHANGELOG entry is ready to publish.
- A first external consumer is identified (or imminent) — promoting before any consumer exists is premature.

If any of these are missing, the contract stays in draft.

A contract that promotes and immediately needs a new major version is a process failure. Premature promotion is worse than slow promotion.

### 3.2 Versioning

Once at v1.0.0, every contract follows semantic versioning:

- **MAJOR** for breaking changes.
- **MINOR** for additive non-breaking changes.
- **PATCH** for bug fixes that don't change behavior.

No exceptions. No "well, it's only a small break."

A contract starts at **v0.1.0** when first promoted from draft. Promotion-from-draft and v1.0.0 are separate decisions:

- **Promotion** says: a consumer may depend on this; a deprecation period applies if it changes; a CHANGELOG entry is required.
- **v1.0.0** says: we commit to strict semver; breaking changes will go through full major-version discipline.

A contract may live at v0.x for as long as is honest. Pre-stable (v0.x) breaking changes are still recorded in the CHANGELOG even though the version bump itself is "permitted."

### 3.3 Breaking changes

The definition of "breaking" depends on the contract format.

**Schemas (JSON, OpenAPI request/response shapes):**

Breaking:
- Removing a field.
- Renaming a field.
- Changing a field's type.
- Making an optional field required.
- Adding a new required field (no default).
- Tightening validation on input (smaller max, fewer enum values, stricter regex).
- Loosening validation on output (consumers may depend on the tighter constraint).

Non-breaking:
- Adding an optional field with no default behavioral change.
- Loosening validation on input.
- Tightening validation on output, *only if* you can prove no producer was emitting outside the new tight constraint.

**HTTP APIs:**

Breaking:
- Removing an endpoint.
- Changing a URL path or HTTP method.
- Changing required query/body parameters.
- Changing response status codes for the same condition.
- Changing the error envelope shape.

Non-breaking:
- Adding a new endpoint.
- Adding optional parameters with sane defaults.
- Adding new optional response fields.

**Python Protocols:**

Breaking:
- Adding a required method.
- Removing a method.
- Changing a method signature.

Non-breaking:
- Adding an optional method with a default implementation in a base class.
- Adding new keyword arguments with defaults (subject to the kwarg-conflict caveat: if subclasses define overlapping kwargs, this is breaking).

**SQL schemas:**

Breaking:
- Removing a column.
- Renaming a column.
- Changing a column's type.
- Adding a NOT NULL column without a default.
- Tightening a CHECK or FK constraint.

Non-breaking:
- Adding a nullable column.
- Adding a column with a default.
- Loosening a constraint.

### 3.4 The asymmetry rule

The same change can be breaking or non-breaking depending on direction. Tightening validation breaks input contracts but doesn't break output contracts. When writing rules for a specific schema, declare which direction it's used in (request vs response, write vs read) and apply the rules accordingly.

### 3.5 When in doubt, treat as breaking

If there's genuine ambiguity, default to breaking. A major-version bump that turns out to be unnecessary is cheaper than a silent break that lands in production.

### 3.6 Deprecation

When a v(N+1) replaces a v(N), the v(N) version moves to deprecated status:

- Both versions must work in production simultaneously for the deprecation period.
- The deprecated version receives bug fixes for security and correctness, but not feature additions.
- The CHANGELOG records both the deprecation date and the planned removal date.
- The deprecated version's spec includes a `deprecated: true` annotation where the format supports it.

After the deprecation period ends, the deprecated version is moved to `contracts/archive/`. The producer (this project) may remove implementation code for that version.

The default deprecation period for this project is **3 months** unless stated otherwise. Specific contracts may set longer periods if their consumers need more time; shorter periods require explicit consent from all affected consumers.

### 3.7 Migration support

When a breaking change ships, the producer provides:

- **A migration guide.** "v1 said X, v2 says Y, here's how to translate." Lives alongside the new spec, named `MIGRATION-v1-to-v2.md`.
- **A worked example.** At least one realistic before/after.
- **Tooling, where reasonable.** A script that converts v1 payloads to v2, or a shim library. Not always required, but encouraged for high-traffic contracts.

The producer is on the hook for migration support during the entire deprecation period. Migration issues reported during this window are treated as defects.

### 3.8 CHANGELOG discipline

Every change to anything in `contracts/v<N>/` gets a CHANGELOG entry. Format:

```markdown
## [1.2.0] - 2026-05-14

### Added
- `http-api`: new `/api/v1/items/{id}/archive` endpoint.

### Deprecated
- `http-api`: `/api/v1/legacy-items` endpoint. Removal scheduled for v2.0.

## [2.0.0] - 2026-08-01 (BREAKING)

### Removed
- `http-api`: `/api/v1/legacy-items` (deprecated since v1.2.0).

### Changed
- `http-api`: `items` response now returns `itemRef` instead of `itemId`.
  Migration: see MIGRATION-v1-to-v2.md.
```

Two rules:

1. **Breaking-change releases must be tagged `(BREAKING)` in the version line.** Skim-readability matters.
2. **Pre-stable (v0.x) breaking changes get the same treatment.** The future v1.0 reader should see the full history.

### 3.9 Version coexistence

When two major versions of a contract exist simultaneously:

- **HTTP APIs**: URL-segment versioning. `/api/v1/items` and `/api/v2/items` coexist as separate endpoints, served by separate handlers. URL is canonical; no header-based negotiation.
- **Schemas in payloads**: version field in the envelope (`schemaVersion`). Consumers route on the version.
- **Python protocols**: separate Protocol classes per major version. `RuntimeAdapterV1` and `RuntimeAdapterV2` coexist; implementations declare which they support.
- **SQL schemas**: rarely supports parallel versions. Migrations are typically additive (add new columns alongside old, double-write, drop old at end of deprecation). Document the dance explicitly when it arises.

---

## 4. The upgrade path: light mode → full mode

The trigger to upgrade is the appearance of an external consumer — anything outside this project that would break if a wire surface changed. When that happens:

1. **Identify the contracts that consumer depends on.** Usually the HTTP API spec, possibly some JSON schemas. List them.
2. **Promote those contracts.** Move from `contracts/draft/<file>` to `contracts/v1/<file>`. Set `info.version: "0.1.0"` in each spec.
3. **Create `contracts/CHANGELOG.md`.** First entry is `[0.1.0] - <date>` with the promotion noted.
4. **Update this design doc** (or write a new one — `design-NNN-contracts-promoted.md`) noting that full-mode discipline is now active and listing the promoted contracts.
5. **From here onward**, follow §3 for those promoted contracts. Drafts and unpromoted contracts continue to follow §2.

The upgrade is not all-or-nothing. Contracts can be in light mode (still in draft) and full mode (promoted) simultaneously. Only promoted contracts carry the discipline.

---

## 5. Open questions

Deliberately deferred and tracked here for visibility:

- **First external consumer.** Until one exists, the upgrade path in §4 is theoretical. Worth revisiting when a real candidate appears (and at that point, this doc may itself be superseded by a more concrete one).
- **CHANGELOG format conventions.** §3.8 describes the format roughly, but the precise template (header levels, date format, version bracketing) may benefit from being its own section once there's anything to record.
- **Cross-language Protocol equivalents.** This project is currently Python-shaped. If/when other languages join, the Protocol-as-contract pattern needs equivalents (Rust traits, TypeScript interfaces, etc.). Likely a future doc.

## 6. Pointers

- For the project's high-level shape: `../../ARCHITECTURE.md` (project-owned).
- For the design values that motivate these structural decisions: `../DESIGN_PHILOSOPHY.md`.
- For naming, file layout, and document categorization rules: `../conventions/NAMING_CONVENTIONS.md`.
- For HTTP API and JSON wire format conventions: `../conventions/API_CONVENTIONS.md`.
- For language-specific source code conventions: `../conventions/CODE_CONVENTIONS-<language>.md`.
- For the founding design of this project: `../../designs/design-001-init.md` (project-owned, when written).
