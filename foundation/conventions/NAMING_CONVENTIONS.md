# Naming Conventions

**Status:** Living document. Update when conventions change; never deviate without an entry here.
**Audience:** Anyone — human or AI assistant — creating files, folders, or documents in this project.
**Scope:** File and document naming, folder layout, contract file naming, versioned artifact naming.

This doc covers files, folders, and document filenames. For HTTP API and JSON wire format conventions, see `API_CONVENTIONS.md`. For language-specific code style (Python identifiers, file structure, etc.), see `CODE_CONVENTIONS-<language>.md`. When a naming question is unclear, this doc is the tiebreaker for files and folders; other docs are tiebreakers for their respective surfaces.

When a convention needs to change, update this doc first, then enforce the change.

---

## 1. The headline rules

**File and folder names: kebab-case.** `design-001-init.md`, `agent-outcome.schema.json`, `service-engine.yaml`. Lowercase, hyphen-separated, no spaces, no underscores, no special characters.

**Reference documents: SHOUTY_CAPS_WITH_UNDERSCORES.md.** `ARCHITECTURE.md`, `DESIGN_PHILOSOPHY.md`, `NAMING_CONVENTIONS.md`. Visible at a glance as foundational/reference rather than time-ordered design records.

**Implementation artifacts follow their language's natural conventions.** Python modules: `service_engine.py` (snake_case per PEP 8). PowerShell: `Get-Item.ps1` (PascalCase per PowerShell convention). Each language has its own conventions doc.

These three rules cover most cases. The rest of this document is the long form.

---

## 2. File and document naming

### 2.1 Design documents

Pattern: `design-<NNN>-<category>[-<specific>].md`

- `<NNN>` — three-digit zero-padded sequence number, scoped to this project. Starts at `001`.
- `<category>` — short noun describing what the doc is about. See §2.2 for the in-use vocabulary.
- `<specific>` — optional. Used when distinguishing siblings within a category (e.g., `ingest-csv` vs `ingest-api`). Omitted when there's only one of the category.

Examples:

```
design-000-meta.md
design-001-init.md
design-002-data-model.md
design-003-http-api.md
design-004-ingest-csv.md
design-004-ingest-api.md
```

(Note: this project does not use a per-app infix because there is only one app. If this project later spawns sibling repos under a wider ecosystem, the multi-repo naming pattern with the `<app>` infix becomes the right choice.)

Filename rules:

- Lowercase only.
- Words separated by hyphens (kebab-case).
- No spaces, underscores, or special characters.
- No abbreviations unless universally understood (`api`, `db`, `id` are fine; `rcptn` is not).
- Three to five meaningful words is the target length. If you're hitting six, the slug is probably saying too much.

### 2.2 Category vocabulary (in-use)

Categories evolve as new design types are needed. Reuse where possible; add new categories sparingly.

The most important category is **`init`** — the founding design. The project's design history starts with `design-001-init.md`. Subsequent designs build on the init.

Other categories that commonly appear:

- `meta` — structural foundation (this is `design-000-meta.md`, before init).
- `data-model` — how persistent state is shaped.
- `http-api` — public HTTP surface design.
- `ingest-<source>` — for systems that ingest external data (e.g., `ingest-csv`, `ingest-api`).
- `provider-<name>` — for pluggable providers (LLM provider, storage backend, etc.).
- `query-<topic>` — for significant query/analytics surfaces.
- `report-<topic>` — for dashboard or reporting designs.
- `migration-<version>` — for substantive schema or behavior migrations.
- `<feature-name>` — specific features built on top of init.

When a new category is needed, add it to this list with a one-sentence description.

### 2.3 Reference and instruction documents

Reference documents that don't fit the design-record pattern use **SHOUTY_CAPS_WITH_UNDERSCORES.md** for the filename:

```
ARCHITECTURE.md
DESIGN_PHILOSOPHY.md
README.md
NAMING_CONVENTIONS.md             ← this file
API_CONVENTIONS.md
CODE_CONVENTIONS-<language>.md    (one per language used in the project)
```

These don't get numbered because they're reference material, not sequential design records. They evolve organically rather than as a versioned sequence.

The exception: language-suffixed code conventions use kebab-case for the language qualifier (`CODE_CONVENTIONS-python.md`, `CODE_CONVENTIONS-bash.md`, `CODE_CONVENTIONS-powershell.md`). The base name stays SHOUTY_CAPS; only the language suffix is kebab.

### 2.4 Implementation artifacts

Code, configs, prompts, and other deliverables follow their natural conventions:

- **Python modules**: `service_engine.py`, `runtime_adapter.py` (snake_case, per PEP 8 and `CODE_CONVENTIONS-python.md`).
- **PowerShell scripts, modules, and manifests**: `Get-Item.ps1`, `MyProject.psm1`, `MyProject.psd1` (PascalCase per PowerShell convention; `Verb-Noun` for scripts that are callable cmdlets). This is a deliberate exception to the kebab-case rule — PowerShell tooling, tab completion, and module autoloading depend on these specific shapes. See `CODE_CONVENTIONS-powershell.md` §2.1.
- **Rust modules**: `service_engine.rs` (snake_case, per Rust convention).
- **C# / .NET source files**: `ItemService.cs` (PascalCase, matching the contained type name, per .NET convention; project files like `MyProject.csproj` follow the same convention). This is also a deliberate exception to the kebab-case rule. See `CODE_CONVENTIONS-dotnet.md` §1.3.
- **JSON Schemas**: `request.schema.json` (kebab-case, ending in `.schema.json` to mark it as a schema definition rather than a data file).
- **OpenAPI specs**: `http-api.openapi.yaml` (kebab-case, ending in `.openapi.yaml`).
- **Python Protocol definitions** (when used as a contract): `runtime-adapter.protocol.py` (kebab-case file with `.protocol.py` suffix to mark it as a protocol contract rather than ordinary code).
- **YAML configs**: `service.yaml`, `feature-flags.yaml` (kebab-case).
- **SQL migrations**: `001-init.sql` or `001_init.py` for Alembic/equivalent (numeric prefix; kebab-case for SQL or snake_case for Python per language convention).

These don't get the `design-` prefix — they aren't design records, they're working artifacts.

### 2.5 Contract files

Contracts in this project are intentionally light while there are no external consumers. Drafts live in `contracts/draft/` and can churn freely.

When this project gains external consumers and adopts the full contract lifecycle (see `../designs/design-000-meta.md`), contract files take this shape:

```
contracts/
  draft/                           # in-flux, not depend-on-able
    http-api.openapi.yaml
    request-shape.schema.json
  v1/                              # promoted, depend-on-able
    http-api.openapi.yaml
    request-shape.schema.json
  archive/                         # removed-but-preserved past versions
```

Each contract file uses a single descriptive name with a format suffix. No category prefix needed because there's only one project's worth of contracts.

### 2.6 Versioned contract files

When two major versions of a contract coexist (only relevant once external consumers exist), file-level versioning makes the coexistence visible:

```
contracts/v1/http-api.v1.openapi.yaml          ← deprecated
contracts/v2/http-api.v2.openapi.yaml          ← current
```

Pattern: insert `.v<N>` before the format extension. The `<N>` is the major version only; minor and patch versions live inside the spec (`info.version: "2.1.3"`).

This applies only when versions actually coexist. A contract at v1.x with no v2 in flight stays as `http-api.openapi.yaml` (no version in filename) — the version is in the spec's `info.version` field. The `.v<N>` filename only appears when two majors must coexist on disk.

---

## 3. Folder layout

```
<project>/
├── README.md
├── ARCHITECTURE.md
├── DESIGN_PHILOSOPHY.md
├── conventions/
│   ├── NAMING_CONVENTIONS.md           ← this file
│   ├── API_CONVENTIONS.md
│   └── CODE_CONVENTIONS-<language>.md  (one file per language used)
├── designs/
│   ├── design-000-meta.md
│   ├── design-001-init.md
│   └── design-NNN-*.md
├── contracts/
│   └── draft/                          # in-flux specs
├── src/                                # source (shape per language convention)
├── tests/
└── migrations/                         # if applicable
```

The exact shape of `src/`, `tests/`, etc. depends on the language and framework. The constants are `designs/`, `contracts/`, the SHOUTY_CAPS reference docs at the root, and the `conventions/` subfolder.

---

## 4. Renaming protocol

If you need to rename an existing design doc:

1. Update this convention doc first (if a new pattern needs to be added).
2. Rename the file.
3. Update all cross-references in other docs (grep for the old name).
4. Note the rename in the doc's revision history if it has one.
5. Don't reuse old numbers. If `design-002-x.md` is renamed and you later add a new doc, give it the next available number, not the freed-up `002`.

For contract files, a rename is more significant — once contracts have external consumers, a rename may be a breaking change. Consult `../designs/design-000-meta.md` before renaming a stable contract.

---

## 5. Why these specific choices

A few decisions might look arbitrary. They're not.

**Numbered prefix.** Lets `ls` show the docs in chronological order without needing a separate index file. Anyone scanning the directory sees the sequence at a glance.

**Kebab-case for filenames.** Works on every operating system, is case-stable, doesn't trigger shell escaping issues. Underscores are valid but visually noisier; spaces are unambiguously wrong on the command line.

**Sub-categories with `<category>-<specific>`.** The `ingest-csv` / `ingest-api` pattern groups related docs together when sorted. All `ingest-*` docs cluster; all `provider-*` docs cluster. Useful when one category has multiple instances.

**SHOUTY_CAPS for reference docs.** Visually distinguishes "read this first" material from time-ordered design records. A directory listing shows both groups separately at a glance.

**`init` as the founding category.** Honest naming. The first doc starts from nothing; calling it `init` says exactly that. Avoids the bootstrap-residue trap of having `update-plan` or `phase-1` style names that imply something predates them.

**No app infix in design names.** This project is one project. Adding an `<app>` infix to every design doc would be ceremony without information. If this project ever spawns sibling repos as a multi-app ecosystem, the infix becomes the right choice — but until then, it's noise.

**File-level versioning for contracts that coexist.** When two majors of a contract are live simultaneously (only relevant once external consumers exist), having both files visible on disk makes the coexistence obvious. The alternative (single file with branching content based on version field) hides the situation.

---

## 6. Where other naming conventions live

This doc covers files, folders, and document filenames. Other naming surfaces are covered elsewhere:

- **HTTP URLs and JSON wire format** (kebab-case paths, snake_case query/path params, camelCase JSON keys, prefixed IDs, lowercase enums): `API_CONVENTIONS.md`.
- **Python identifiers** (snake_case modules/functions, PascalCase classes, SCREAMING_SNAKE constants): `CODE_CONVENTIONS-python.md`.
- **Other language identifiers**: `CODE_CONVENTIONS-<language>.md` (one per language; `-bash.md`, `-powershell.md`, `-rust.md`, etc., as they're added).
- **Database tables and columns** (snake_case): the relevant `CODE_CONVENTIONS` doc covers this for the language doing the database access.

When this doc and a more-specific doc disagree, the more-specific doc wins for its surface. When they're silent, this doc applies.

---

## 7. Cross-references

- For the structural foundation that governs design docs and contracts: `../designs/design-000-meta.md`.
- For the design values that motivate naming-as-discipline: `../DESIGN_PHILOSOPHY.md` (especially "Conventions are constitutional").
- For the architecture that this layout supports: `../../ARCHITECTURE.md` (project-owned).
