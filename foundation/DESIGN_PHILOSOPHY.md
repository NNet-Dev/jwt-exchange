# Design Philosophy

**Status:** Reference (top-level, not a numbered design doc).
**Companion to:** `../ARCHITECTURE.md` (project-owned), `conventions/*`, `designs/design-000-meta.md`.

## Purpose

This document captures the values that govern design decisions in this project. It is project-internal but adapted from a wider engineering ecosystem; the values are universal and apply equally to a single project.

When a new design decision needs to be made and the conventions don't speak to it directly, this document provides the values to apply. When two values conflict, this document does not always tell you which wins — that's a judgment call — but it does tell you what's at stake.

The relationship between layers:

- **Philosophy** (this doc): why we hold the values we hold.
- **Conventions** (`conventions/*`): the rules those values produce.
- **Architecture** (`../ARCHITECTURE.md`): the structure those rules produce.
- **Design docs** (`designs/*`): specific decisions that apply philosophy + conventions + architecture to a particular problem.

Each layer informs the layer below it. None of them stand alone.

---

## Architectural principles

### Separation of concerns is structural, not aspirational

Even within one project, modules do one thing. When responsibilities blur, the structure forces a hard question: which module does this belong to?

The answer is determined by what state the work mutates. A function that writes to a particular table, a particular external system, or a particular in-memory cache belongs to the module that owns that state. The runtime location of code is a deployment detail; the state it mutates is the architectural truth.

This applies inside one project (which module owns this?) the same way it applies across projects (which app owns this?). The pattern is the same.

### Components have contracts

Every component — module, library, internal service — has a contract: purpose, inputs, output schema, dependencies, observability requirements. The contract is the interface; the implementation is replaceable.

This is what makes integration testable. It is what makes substitution possible. Without contracts, components are bespoke functions tied to specific call sites; with contracts, they are interchangeable units.

A component that violates its contract is broken even if it produces "correct-looking" output. The contract is the agreement; the output is just the deliverable.

For now, the project's contracts are mostly internal — between modules. As the project grows external consumers, internal contracts may be promoted to versioned external ones. See `designs/design-000-meta.md` for that lifecycle.

### Standalone form first, integration later

The project is designed to work in its standalone form before any integrations are wired. External integrations are features added on top of working standalone components, not preconditions for the components to function.

This means the project can be built, tested, and deployed independently. It also means the project degrades gracefully: when an external dependency is unavailable, the standalone form still works wherever possible.

---

## Code and design principles

### Conventions are constitutional

The naming, API, and code conventions are not suggestions. They are the contract that everyone — humans, AI assistants generating code, future maintainers — follows. When two contributors use the same conventions, their work composes. When they don't, every integration becomes a translation problem.

This means conventions are precious. Drift is the enemy. Small inconsistencies become real defects. (The canonical example from the wider ecosystem: a `lockedBy` vs `locked_by` mismatch between two parts of a system that disagreed about a shared shape.)

If you find yourself wanting to deviate from a convention, the right move is usually to update the convention (carefully, with discussion) rather than to deviate locally.

### Bootstrap-residue is a smell

Code written to bootstrap a system into existence often contains patterns that don't belong in the steady-state system. Placeholder helpers, dual-mode parameters, mixed conventions, "we'll fix this later" branches.

When the bootstrap is complete, the residue should be removed. Not deferred. Not "tracked as tech debt." Removed.

The criterion: would a fresh-eyes reviewer understand why this code exists? If the answer requires explaining the bootstrap process, the code is residue. Clean it.

### Internal contracts can churn; external contracts cannot

While this project has no external consumers, internal module contracts can be reshaped freely. The discipline is just to keep them honest — declared shapes, real types, no implicit coupling through "everyone happens to use the same dict shape."

When external consumers appear, the relevant internal contracts get promoted to versioned external ones, and from that promotion event onward they follow the strict lifecycle in `designs/design-000-meta.md`. Until then, treat the contracts directory as a workspace, not a published surface.

### When in doubt, treat it as breaking

For any change to a contract that *does* have external consumers: if there's genuine ambiguity about whether it's breaking, default to breaking. A major-version bump that turns out to be unnecessary is cheaper than a silent break in production.

This applies to schemas, APIs, protocols, SQL — any contract surface. The cost of being conservative is small. The cost of being wrong is high.

### Duplication is cheaper than premature coupling

Inside one project, the same instinct as in a multi-project ecosystem applies: when two modules need similar functionality, the first instinct is often to share — extract a common helper, put it in `utils.py`, import from both sides.

This is sometimes the wrong move. The cost of maintaining two slightly-different copies is sometimes lower than the cost of a shared helper that has to satisfy two sets of requirements and stay backward-compatible across both call sites.

Coupling is earned, not assumed. If duplication becomes painful, that's evidence the abstraction is needed. If duplication is fine, the abstraction wasn't.

This rule is weaker inside one project than across projects (because there's only one repo, one release cadence, one team), but it's worth keeping in mind: shared utilities accumulate complexity over time.

---

## Operational principles

### Cost discipline matters

Every system has costs — compute, memory, network, dollars, attention. The project tracks costs at every level so they're visible, attributable, and controllable.

Designs that ignore cost are not finished designs. Budget controls are first-class concerns. When evaluating a design choice, "what does this cost?" is one of the questions that always applies.

### One concern per change

A pull request, a design doc, a migration — each addresses one concern. Combining concerns makes review harder, makes rollback harder, and makes the history harder to read.

This is harder than it sounds. A concern can be tempting to expand: "while I'm here, let me also fix this other thing." Resist. File the other thing as a separate change.

The discipline pays off when something goes wrong: rollback to the last known-good change is meaningful only if changes are scoped tightly.

### Self-hosted by preference

Where reasonable, the project favors self-hosted infrastructure. This is a values choice, not just a technical one: control over data, predictable cost, freedom to modify, no vendor lock-in. It comes with operational cost (you maintain it; you debug it; you upgrade it) but the tradeoff is consistent.

When external services are necessary, the integration is wrapped in a contract the project owns, rather than letting external dependencies leak into application code. Swapping a provider becomes a contract-level concern, not a code-level rewrite.

### Deployment portability

Application code is deployment-agnostic. The project can run as a bare-metal process (managed by systemd or equivalent), as a Docker container, in a container orchestrator, or in a developer's terminal via a direct command. Lock-in to a specific runtime is rejected.

Deployment-specific concerns — process management, log routing, secret injection, reverse proxy, TLS termination, file system layout — are handled by the deployment layer, not the application. The application reads config from env vars and an optional config file (path configurable), logs to stdout, provides CLI subcommands for common operations (migrate, run, health-check), and makes no assumptions about where its data lives.

Two consequences worth naming:

- **Default config values must work in multiple modes.** A default of `/data/db.sqlite` only makes sense in a container; a default of `~/.local/share/<app>/db.sqlite` only makes sense bare-metal. Pick defaults that work in either context (typically a CWD-relative path, or a clearly-flagged "must be set" with no default).
- **Packaging produces multiple artifacts.** Where applicable, the build pipeline produces both a Dockerfile (for container deploys) and an installable package (for direct deploys), in tandem. Neither is treated as primary.

---

## Anti-patterns to recognize

A few patterns that look reasonable in isolation but corrode over time:

**The cross-module utility.** A "helper" function that gets called by two or more modules, lives in neither, and ends up in `utils.py` as a kitchen sink. This is how module boundaries erode. If it's truly cross-cutting, give it a real home with a real contract. If it's module-specific helper logic that "happens to also be useful for the other module," consider duplicating instead of extracting.

**The "we'll version it later."** A schema that ships without an explicit version, on the assumption that "v1 is implicit." When v2 needs to ship, there's no clean way to negotiate which version a payload conforms to. Always include `schemaVersion` from the start, even for purely internal contracts — it costs nothing and saves real grief.

**The bootstrap-permanent.** Code written for the bootstrap that never gets cleaned up. The placeholder helper that "works fine," the dual-mode parameter that "isn't hurting anything." These accumulate. Each is small; together they are the technical debt that makes the next refactor painful.

**The hidden coupling.** Two modules that share state and start "coordinating" through clever queries against each other's internals. This isn't a contract — it's a side door. If modules need to share state structurally, make the contract explicit so the relationship is visible.

**The external-dep leak.** A module that imports `boto3` (or `requests`, or any external client) without a wrapping contract. When the external service changes its API or you decide to swap providers, the change touches every call site instead of one wrapper. Always wrap external dependencies behind a contract you own — even within one project.

---

## How to apply these in practice

When making a design decision, ask:

1. Does this respect the separation of concerns? Which module owns this work?
2. Does this give the component a clear contract?
3. Does this work in standalone form?
4. Does this respect the conventions?
5. Is this accumulating bootstrap-residue?
6. Does this measure cost as a system property?
7. Is this introducing coupling that should be a contract instead?

If any answer is unclear, the design is unfinished. Iterate.

---

## When values conflict

Sometimes values pull in opposite directions. A few common conflicts:

**Speed vs convention compliance.** Convention compliance wins, almost always. The few cases where speed wins should be temporary and labeled as such.

**Generality vs specificity.** Specificity wins for new features; generality is earned by seeing the same shape multiple times. Don't generalize on a single example.

**Cost vs correctness.** Correctness wins. A cheaper but wrong design is not cheaper.

**Backward compatibility vs cleanliness.** For internal-only contracts, cleanliness wins (refactor freely). For contracts with external consumers, backward compatibility wins (follow the lifecycle in `designs/design-000-meta.md`).

**Coupling vs duplication.** Lean toward duplication, especially across module boundaries. Coupling is earned.

**Standalone form vs integration value.** Standalone form wins for foundational designs. Integration is added later, on top of working components.

**Self-hosted vs convenience.** Self-hosted wins by default. Use external services where the cost-benefit is clearly favorable, and wrap them in contracts so they're swappable.

**Deployment portability vs single-runtime convenience.** Portability wins. Every shortcut that ties code to one runtime closes off another deployment option. The application stays runtime-agnostic; the deployment layer adapts.

These are heuristics, not laws. Real situations call for real judgment.
