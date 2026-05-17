#!/usr/bin/env bash
#
# new-design.sh — Scaffold the next design doc in a project or the foundation.
#
# Auto-detects the designs directory, finds the next available number,
# and generates a new design file with proper frontmatter and structure.
#
# Usage:
#   scripts/new-design.sh <category> [flags]
#
#   <category> is a short hyphenated slug: feature, api, data-model, refactor, etc.
#
# Flags:
#   --depends-on <NNN>   Design this one depends on (e.g. 001)
#   --supersedes <NNN>   Design this one replaces (e.g. 002)
#   --dir <path>         Override designs directory (default: auto-detected)
#   --dry-run            Print the path and content without writing
#   --help, -h           Show this help
#
# Examples:
#   # From a consuming project:
#   ./scripts/new-design.sh feature
#   ./scripts/new-design.sh api --depends-on 001
#   ./scripts/new-design.sh refactor --supersedes 003
#
#   # From the foundation repo itself:
#   scripts/new-design.sh conventions
#
#   # Custom directory:
#   scripts/new-design.sh feature --dir path/to/designs
#
# For agentic agents: after scaffolding, fill in the Purpose and design sections,
# resolve any open questions, and update Status to Active when the design is
# approved. Cross-reference this doc from any designs that depend on it.

set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
CATEGORY=""
DEPENDS_ON=""
SUPERSEDES=""
DESIGNS_DIR=""
DRY_RUN=false

# ---------------------------------------------------------------------------
# Parse args
# ---------------------------------------------------------------------------
usage() {
    sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --depends-on) DEPENDS_ON="$2"; shift 2 ;;
        --supersedes) SUPERSEDES="$2"; shift 2 ;;
        --dir)        DESIGNS_DIR="$2"; shift 2 ;;
        --dry-run)    DRY_RUN=true; shift ;;
        --help|-h)    usage; exit 0 ;;
        -*)           echo "Unknown flag: $1" >&2; usage >&2; exit 1 ;;
        *)            CATEGORY="$1"; shift ;;
    esac
done

if [[ -z "$CATEGORY" ]]; then
    echo "ERROR: category is required (e.g. feature, api, data-model)" >&2
    usage >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Auto-detect designs directory
# ---------------------------------------------------------------------------
if [[ -z "$DESIGNS_DIR" ]]; then
    # Try common locations in priority order:
    # 1. designs/ (consuming project root)
    # 2. payload/designs/ (foundation repo root)
    # 3. ../designs/ (from scripts/ subdirectory)
    for candidate in "designs" "payload/designs" "../designs"; do
        if [[ -d "$candidate" ]]; then
            DESIGNS_DIR="$candidate"
            break
        fi
    done
fi

if [[ -z "$DESIGNS_DIR" ]] || [[ ! -d "$DESIGNS_DIR" ]]; then
    echo "ERROR: could not find designs/ directory." >&2
    echo "       Run this from a project root or the foundation repo," >&2
    echo "       or specify --dir <path>." >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Determine next number
# ---------------------------------------------------------------------------
# Extract numbers from existing design-NNN-*.md files
NEXT_NUM=$(ls "$DESIGNS_DIR"/design-*.md 2>/dev/null \
    | grep -o 'design-[0-9]*' \
    | sed 's/design-//' \
    | sort -n \
    | tail -1 \
    | awk '{printf "%03d", $1 + 1}' || true)

if [[ -z "$NEXT_NUM" ]]; then
    # No existing designs — start at 001
    NEXT_NUM="001"
fi

# Check if this number already exists (safety check)
OUTPUT_FILE="$DESIGNS_DIR/design-${NEXT_NUM}-${CATEGORY}.md"
if [[ -f "$OUTPUT_FILE" ]]; then
    echo "ERROR: $OUTPUT_FILE already exists." >&2
    echo "       Use a different category name." >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Build frontmatter values
# ---------------------------------------------------------------------------
SUPERSEDES_VAL="[]"
if [[ -n "$SUPERSEDES" ]]; then
    SUPERSEDES_VAL="[design-$(printf '%03d' "$SUPERSEDES")-*.md]"
fi

DEPENDS_ON_VAL="[]"
if [[ -n "$DEPENDS_ON" ]]; then
    DEPENDS_ON_VAL="[design-$(printf '%03d' "$DEPENDS_ON")-*.md]"
fi

# ---------------------------------------------------------------------------
# Generate content (shared between stdout and file write)
# ---------------------------------------------------------------------------
TITLE=$(echo "$CATEGORY" | tr '-' ' ' | awk '{for(i=1;i<=NF;i++) $i=toupper(substr($i,1,1)) substr($i,2)} 1')

generate_content() {
cat <<EOF
---
app: [app key — matches your repository or service name]
owner: [team or person accountable]
status: Draft
supersedes: ${SUPERSEDES_VAL}
depends_on: ${DEPENDS_ON_VAL}
owns: []

# === AGENT-DISCOVERED (do not edit manually) ===
# build:
#   languages: []
#   tool: 
#   entry_points: []
#   commands: {}
#   artifacts: []
# deployment:
#   target: 
#   config_via: 
#   requires_env: []
---

# Design ${NEXT_NUM} — ${TITLE}

## Purpose

> State in one sentence what this design addresses and why it matters.
> Reference the problem, not the solution.

[Describe the problem or opportunity this design addresses.]

---

## Context

> What led to this decision? Prior designs, constraints, or external factors.
> Link to relevant docs, issues, or prior art.

[Background context here.]

---

## Design

> The actual decision. Be specific about what changes and what stays the same.
> Use diagrams, examples, and before/after comparisons where helpful.

### What changes

- [Change 1]
- [Change 2]

### What stays the same

- [Unchanged aspect 1]

### Rationale

> Why this approach over alternatives? What trade-offs were considered?

[Rationale here.]

---

## Impact

> What breaks? What needs updating? Who needs to know?

### Affected components

- [Component 1]: [what changes]
- [Component 2]: [what changes]

### Migration notes

> If this is a breaking change, describe the migration path.
> If not breaking, delete this section.

[Migration notes or "N/A — non-breaking change".]

---

## Open questions

> Deliberately deferred items with enough context that a future doc can pick them up.
> Do not let open questions accumulate without ownership.
> See \`design-000-meta.md\` §1.4 for the rules.

- [ ] [Question 1 — who owns this, when does it need resolving?]

---

## Cross-references

- \`design-000-meta.md\` — Design doc discipline and contract lifecycle.
- \`design-001-init.md\` — Founding design of this project.
EOF
}

# ---------------------------------------------------------------------------
# Write or dry-run
# ---------------------------------------------------------------------------
if $DRY_RUN; then
    echo ""
    echo "--- (dry-run) would write to: $OUTPUT_FILE ---"
    echo ""
    generate_content
else
    mkdir -p "$DESIGNS_DIR"
    generate_content > "$OUTPUT_FILE"
    echo "Created: $OUTPUT_FILE"
fi

# ---------------------------------------------------------------------------
# Agentic agent instructions
# ---------------------------------------------------------------------------
if ! $DRY_RUN; then
    echo ""
    echo "--- Agent instructions for this design doc ---"
    echo ""
    echo "1. Fill in Purpose: Describe the PROBLEM, not the solution."
    echo "2. Fill in Context: Link to prior designs, issues, or constraints."
    echo "3. Fill in Design: Be specific. Use diagrams and examples."
    echo "4. Fill in Impact: List affected components and migration notes."
    echo "5. Fill in Open questions: Assign owners and deadlines."
    echo "6. Update \`status\` to 'Active' when the design is approved."
    echo "7. If other designs depend on this one, update their \`depends_on\` field."
    echo "8. If this supersedes an old design, update the old design's \`status\` to 'Superseded'."
    echo ""
    echo "Rules from design-000-meta.md:"
    echo "- Substantive changes write a NEW doc, not edits to an Active doc."
    echo "- Open questions must have ownership; delete if unimportant after several cycles."
    echo "- Breaking changes require a new version number and migration notes."
fi
