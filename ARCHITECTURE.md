# Architecture

**Status:** Reference (read me first).
**Audience:** Anyone new to this project.
**Purpose:** Orient. Then go to the deeper docs.

---

## What this is

JWT Exchange is a Rust service for receiving, validating, transforming, and minting JWT tokens. It sits between clients and downstream services, handling token verification, claim mapping, and exchange logic.

This project deploys independently. Its foundation (philosophy, conventions, structural rules) is synced from the central foundation repo into the `foundation/` folder; this project owns its source, its data, its deployment, and its release cadence.

This document is the entry point. It does not describe the project's domain in detail; it tells you what's here and where to find what. The domain-level description lives in `designs/design-001-init.md`.

## High-level shape

```
jwt-exchange/
├── README.md
├── ARCHITECTURE.md             ← this file
├── Cargo.toml                  ← Rust package manifest
├── .foundation                 ← foundation metadata (JSON)
├── foundation/                 ← SYNCED. Read it; do not edit it.
│   ├── DESIGN_PHILOSOPHY.md
│   ├── conventions/
│   └── designs/design-000-meta.md
├── conventions-local/          ← project's overrides/extensions to foundation
├── designs/                    ← project design docs in numbered sequence
├── contracts/draft/            ← in-flux contract specs (light-mode)
├── src/                        ← source code (Rust service shape)
├── tests/
└── scripts/sync-foundation.sh
```

## How this project consumes the foundation

The `foundation/` folder is synced from a central repo (see `.foundation` for pinned version/profile metadata). All files inside `foundation/` are read-only — edits are lost on the next sync.

When this project needs to deviate from a foundation convention, the deviation goes in `conventions-local/` as a document that explicitly extends the relevant foundation file. Both files coexist; the project-local one takes precedence where they conflict.

## Where to start

**If you're new and want orientation:**

1. This doc.
2. `foundation/DESIGN_PHILOSOPHY.md` — universal design values.
3. `designs/design-001-init.md` — what this project actually does.

**If you're adding a new design doc:**

1. This doc.
2. `foundation/designs/design-000-meta.md` — the structural rules for design docs.
3. Existing design docs in numerical order, as needed for context.

**If you're touching code:**

1. `foundation/conventions/CODE_CONVENTIONS-rust.md`.
2. `conventions-local/CODE_CONVENTIONS-rust.md` if it exists (project overrides).
3. The design doc that introduced the code you're touching.

**If you're touching an API or wire surface:**

1. `foundation/conventions/API_CONVENTIONS.md`.
2. `foundation/conventions/NAMING_CONVENTIONS.md`.

## Documentation map

| What you need | Where to look |
|---|---|
| Orient to this project | This doc, then `designs/design-001-init.md` |
| Universal design values | `foundation/DESIGN_PHILOSOPHY.md` |
| Structural rules (design docs, contracts, versioning) | `foundation/designs/design-000-meta.md` |
| File and folder naming | `foundation/conventions/NAMING_CONVENTIONS.md` |
| HTTP API and JSON wire format | `foundation/conventions/API_CONVENTIONS.md` |
| Rust source code style | `foundation/conventions/CODE_CONVENTIONS-rust.md` |
| Project-specific convention overrides | `conventions-local/` |
| What this project does | `designs/design-001-init.md` |
| A specific feature design | `designs/design-NNN-<category>.md` |

## Operational picture

_Still being defined — see `designs/design-001-init.md` for the founding design._

## Status

Foundation in place (v0.5.1, standard profile, Rust). Init design in progress.
