#!/usr/bin/env bash
#
# sync-foundation.sh - Update the foundation/ folder in a consuming project.
#
# Reads .foundation from the project root, clones each upstream in the sync
# chain, and overlays files into foundation/, prepending a "synced from /
# do not edit" header.
#
# Supports a multi-upstream chain (foundation → conventions-org) where later
# upstreams overlay files from earlier ones. This lets organisations publish
# convention overrides without forking the foundation.
#
# Priority chain (lowest → highest):
#   1. foundation/     — synced from foundation repo via profile manifest
#   2. conventions-org — overlaid on foundation/; org files win at same path,
#                        org-only files become defaults in foundation/
#   3. conventions-local/ — project-owned, not synced, highest priority
#
# This script is itself synced — running it copies the latest version into
# scripts/sync-foundation.sh.
#
# Usage:
#   ./scripts/sync-foundation.sh                          # sync all upstreams at pinned versions
#   ./scripts/sync-foundation.sh --version 0.2.0          # sync foundation at specific version
#   ./scripts/sync-foundation.sh --version latest         # sync foundation at latest tagged release
#   ./scripts/sync-foundation.sh --branch NAME            # sync foundation from a branch (overrides --version)
#   ./scripts/sync-foundation.sh --repo URL               # override foundation repo URL
#   ./scripts/sync-foundation.sh --org-url URL            # add/override org conventions URL
#   ./scripts/sync-foundation.sh --org-version VER        # pin org conventions version
#   ./scripts/sync-foundation.sh --add-upstream name url  # add a new upstream to .foundation
#   ./scripts/sync-foundation.sh --dry-run                # show what would change
#   ./scripts/sync-foundation.sh --help
#
# Multi-upstream chain:
#   Each upstream in the chain is synced in order. If an upstream provides
#   a file that already exists from an earlier upstream, the later upstream
#   wins. The sync header on each file records which upstream it came from.
#
#   Priority chain (lowest → highest):
#     1. Foundation conventions (base layer, profile-selected files)
#     2. conventions-org (org defaults; overlays foundation, adds new files)
#     3. conventions-local/ (project-owned, not synced, highest priority)

set -euo pipefail

# ---------------------------------------------------------------------------
# Wrap the entire body in a compound block so bash parses it all into memory
# before executing. This is necessary because the script self-updates partway
# through (overwriting itself on disk); without the wrap, bash would re-read
# the changed file mid-execution and crash on garbled bytes.
# ---------------------------------------------------------------------------
{

# ---------------------------------------------------------------------------
# Defaults — override at top of script if you fork the foundation repo.
# ---------------------------------------------------------------------------
DEFAULT_REPO_URL="https://github.com/NNet-Dev/foundation"

# Use a short-lived credential cache during sync so repeated git
# operations do not prompt for credentials multiple times.
GIT_CREDENTIAL_HELPER="${FOUNDATION_GIT_CREDENTIAL_HELPER:-cache --timeout=3600}"
git_auth() {
    if [[ -n "$GIT_CREDENTIAL_HELPER" ]]; then
        git -c credential.helper="$GIT_CREDENTIAL_HELPER" "$@"
    else
        git "$@"
    fi
}

# ---------------------------------------------------------------------------
# Parse args
# ---------------------------------------------------------------------------
TARGET_VERSION=""
REPO_URL=""
ORG_URL=""
ORG_VERSION=""
DRY_RUN=false
ADD_UPSTREAM_NAME=""
ADD_UPSTREAM_URL=""
ADD_UPSTREAM_TYPE="org"
TARGET_BRANCH=""

usage() {
    sed -n '3,31p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)       TARGET_VERSION="$2"; shift 2 ;;
        --branch)        TARGET_BRANCH="$2"; shift 2 ;;
        --repo)          REPO_URL="$2"; shift 2 ;;
        --org-url)       ORG_URL="$2"; shift 2 ;;
        --org-version)   ORG_VERSION="$2"; shift 2 ;;
        --dry-run)       DRY_RUN=true; shift ;;
        --add-upstream)  ADD_UPSTREAM_NAME="$2"; ADD_UPSTREAM_URL="$3"; shift 3 ;;
        --help|-h)       usage; exit 0 ;;
        *)               echo "Unknown argument: $1" >&2; usage >&2; exit 1 ;;
    esac
done

# ---------------------------------------------------------------------------
# Locate project root (where .foundation lives)
# ---------------------------------------------------------------------------
find_project_root() {
    local dir="$PWD"
    while [[ "$dir" != "/" ]]; do
        if [[ -f "$dir/.foundation" ]]; then
            echo "$dir"
            return 0
        fi
        dir="$(dirname "$dir")"
    done
    return 1
}

if ! PROJECT_ROOT=$(find_project_root); then
    echo "ERROR: not inside a foundation-consuming project (no .foundation found)" >&2
    echo "       run from inside the project, or use init-project.sh to create a new project" >&2
    exit 1
fi

cd "$PROJECT_ROOT"

# ---------------------------------------------------------------------------
# Read pinned state from .foundation JSON
# ---------------------------------------------------------------------------
FOUNDEOF="$PROJECT_ROOT/.foundation"

# Helper: read a value from the .foundation JSON using python3.
# Supports both flat keys ("repo.url") and nested lookups.
json_get() {
    local file="$1" jq_path="$2" default="${3:-}"
    python3 -c "
import json, sys
d = json.load(open(sys.argv[1]))
parts = sys.argv[2].split('.')
for p in parts:
    if isinstance(d, dict) and p in d:
        d = d[p]
    else:
        print('', end='')
        sys.exit(0)
if isinstance(d, (str, int, float, bool)):
    print(d, end='')
elif isinstance(d, list):
    print(json.dumps(d), end='')
" "$file" "$jq_path" 2>/dev/null || echo "$default"
}

# ---------------------------------------------------------------------------
# Handle --add-upstream: append a new upstream to .foundation and exit
# ---------------------------------------------------------------------------
if [[ -n "$ADD_UPSTREAM_NAME" && -n "$ADD_UPSTREAM_URL" ]]; then
    python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    d = json.load(f)
if 'sync' not in d:
    d['sync'] = {}
if 'upstreams' not in d['sync']:
    d['sync']['upstreams'] = []
for u in d['sync']['upstreams']:
    if u.get('name') == sys.argv[2]:
        u['url'] = sys.argv[3]
        u['version'] = u.get('version', 'latest')
        break
else:
    d['sync']['upstreams'].append({
        'name': sys.argv[2],
        'url': sys.argv[3],
        'version': 'latest',
        'type': sys.argv[4] if len(sys.argv) > 4 else 'org'
    })
with open(sys.argv[1], 'w') as f:
    json.dump(d, f, indent=2)
    f.write('\n')
print('Added upstream:', sys.argv[2], '->', sys.argv[3])
" "$FOUNDEOF" "$ADD_UPSTREAM_NAME" "$ADD_UPSTREAM_URL" "$ADD_UPSTREAM_TYPE"
    exit 0
fi

# ---------------------------------------------------------------------------
# Build the upstream chain
# ---------------------------------------------------------------------------
# Upstreams can come from:
#   1. New schema: sync.upstreams array in .foundation
#   2. Legacy schema: repo.url + project.version (single upstream)
#   3. CLI overrides: --repo, --org-url, --version, --org-version
# ---------------------------------------------------------------------------
UPSTREAMS_JSON=$(json_get "$FOUNDEOF" "sync.upstreams" "")

if [[ -n "$UPSTREAMS_JSON" && "$UPSTREAMS_JSON" != "null" ]]; then
    # New schema: parse the upstreams array
    # Apply CLI overrides
    UPSTREAMS_JSON=$(python3 -c "
import json, sys
upstreams = json.loads(sys.argv[1])
overrides = {'repo': (sys.argv[2], sys.argv[3]), 'org': (sys.argv[4], sys.argv[5])}
for u in upstreams:
    t = u.get('type', 'foundation')
    if t in overrides:
        url, ver = overrides[t]
        if url: u['url'] = url
        if ver: u['version'] = ver
print(json.dumps(upstreams))
" "$UPSTREAMS_JSON" "$REPO_URL" "$TARGET_VERSION" "$ORG_URL" "$ORG_VERSION")
else
    # Legacy schema: build a single upstream from repo.url + project.version
    LEGACY_URL=$(json_get "$FOUNDEOF" "repo.url" "$DEFAULT_REPO_URL")
    LEGACY_VERSION=$(json_get "$FOUNDEOF" "project.version" "")
    [[ -z "$LEGACY_URL" ]] && LEGACY_URL="$DEFAULT_REPO_URL"
    # Apply CLI overrides
    [[ -n "$REPO_URL" ]] && LEGACY_URL="$REPO_URL"
    [[ -n "$TARGET_VERSION" ]] && LEGACY_VERSION="$TARGET_VERSION"

    UPSTREAMS_JSON="[{\"name\":\"foundation\",\"url\":\"$LEGACY_URL\",\"version\":\"${LEGACY_VERSION:-latest}\",\"type\":\"foundation\"}]"

    # Add org upstream if specified via CLI
    if [[ -n "$ORG_URL" ]]; then
        UPSTREAMS_JSON=$(python3 -c "
import json
upstreams = json.loads('$UPSTREAMS_JSON')
upstreams.append({'name':'conventions-org','url':sys.argv[1],'version':sys.argv[2],'type':'org'})
print(json.dumps(upstreams))
" "$ORG_URL" "${ORG_VERSION:-latest}")
    fi
fi

# ---------------------------------------------------------------------------
# Resolve "latest" versions for each upstream
# ---------------------------------------------------------------------------
resolve_version() {
    local url="$1" ver="$2"
    if [[ "$ver" == branch:* ]]; then
        # Branch reference — return as-is; caller must clone by name, not tag
        echo "$ver"
    elif [[ "$ver" == "latest" ]]; then
        echo "Looking up latest tagged release of $url ..." >&2
        local result
        result=$(git_auth ls-remote --tags --refs "$url" 2>/dev/null \
            | awk -F/ '{print $NF}' \
            | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' \
            | sort -V \
            | tail -1 \
            | sed 's/^v//')
        if [[ -z "$result" ]]; then
            echo "ERROR: could not determine latest version from $url" >&2
            return 1
        fi
        echo "$result"
    else
        echo "$ver"
    fi
}

# Parse the upstreams array into parallel arrays for iteration
UPSTREAM_COUNT=$(python3 -c "import json; print(len(json.loads('$UPSTREAMS_JSON')))")

declare -a UP_NAMES UP_URLS UP_VERSIONS UP_TYPES
for i in $(seq 0 $((UPSTREAM_COUNT - 1))); do
    UP_NAMES[$i]=$(python3 -c "import json; u=json.loads('$UPSTREAMS_JSON')[$i]; print(u['name'])")
    UP_URLS[$i]=$(python3 -c "import json; u=json.loads('$UPSTREAMS_JSON')[$i]; print(u['url'])")
    UP_VERSIONS[$i]=$(python3 -c "import json; u=json.loads('$UPSTREAMS_JSON')[$i]; print(u.get('version','latest'))")
    UP_TYPES[$i]=$(python3 -c "import json; u=json.loads('$UPSTREAMS_JSON')[$i]; print(u.get('type','foundation'))")
done

# Resolve versions
for i in $(seq 0 $((UPSTREAM_COUNT - 1))); do
    UP_VERSIONS[$i]=$(resolve_version "${UP_URLS[$i]}" "${UP_VERSIONS[$i]}") || exit 1
done

# ---------------------------------------------------------------------------
# Read profile and languages
# ---------------------------------------------------------------------------
PROFILE=$(json_get "$FOUNDEOF" "project.profile" "standard")
LANGUAGES=$(python3 -c "
import json
d = json.load(open('$FOUNDEOF'))
langs = d.get('project',{}).get('languages', ['python'])
if isinstance(langs, str):
    langs = [langs]
print(' '.join(langs))
" 2>/dev/null || echo "python")

# ---------------------------------------------------------------------------
# Display sync plan
# ---------------------------------------------------------------------------
echo "Project root:    $PROJECT_ROOT"
echo "Profile:         $PROFILE"
echo "Languages:       $LANGUAGES"
echo "Upstreams:"
for i in $(seq 0 $((UPSTREAM_COUNT - 1))); do
    ver="${UP_VERSIONS[$i]}"
    if [[ "$ver" == branch:* ]]; then
        echo "  $((i+1)). ${UP_NAMES[$i]} (${UP_TYPES[$i]}): ${UP_URLS[$i]} @ ${ver}"
    else
        echo "  $((i+1)). ${UP_NAMES[$i]} (${UP_TYPES[$i]}): ${UP_URLS[$i]} @ v${ver}"
    fi
done
echo ""

# ---------------------------------------------------------------------------
# Clone each upstream
# ---------------------------------------------------------------------------
MAIN_TMPDIR=$(mktemp -d)
trap 'rm -rf "$MAIN_TMPDIR"' EXIT

declare -a UP_TMPDIRS
for i in $(seq 0 $((UPSTREAM_COUNT - 1))); do
    name="${UP_NAMES[$i]}"
    url="${UP_URLS[$i]}"
    ver="${UP_VERSIONS[$i]}"
    tmpdir="$MAIN_TMPDIR/$name"

    if [[ "$ver" == branch:* ]]; then
        clone_ref="${ver#branch:}"
        echo "Fetching $name branch '$clone_ref' ..."
    else
        clone_ref="v$ver"
        echo "Fetching $name v$ver ..."
    fi
    if ! git_auth clone --quiet --depth 1 --branch "$clone_ref" "$url" "$tmpdir" 2>/dev/null; then
        if [[ "$ver" == branch:* ]]; then
            echo "ERROR: could not clone $url at branch '${ver#branch:}'" >&2
        else
            echo "ERROR: could not clone $url at tag v$ver" >&2
            echo "       check the version exists: git ls-remote --tags $url" >&2
        fi
        exit 1
    fi
    UP_TMPDIRS[$i]="$tmpdir"
done

# ---------------------------------------------------------------------------
# Validate profile exists (in the first/upstream=0 clone)
# ---------------------------------------------------------------------------
PROFILE_FILE="${UP_TMPDIRS[0]}/profiles/$PROFILE.txt"
if [[ ! -f "$PROFILE_FILE" ]]; then
    echo "ERROR: profile '$PROFILE' not found in foundation v${UP_VERSIONS[0]}" >&2
    echo "       available profiles:" >&2
    ls "${UP_TMPDIRS[0]}/profiles/" 2>/dev/null | sed 's|\.txt$||;s|^|         |' >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Build the SYNC header (prepended to each synced file)
# ---------------------------------------------------------------------------
TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)

build_header() {
    local src_name="$1" src_url="$2" src_version="$3"
    cat <<EOF
<!--
============================================================================
SYNCED FROM: $src_url
VERSION:     v$src_version
LAST SYNCED: $TIMESTAMP
PROFILE:     $PROFILE
SOURCE:      $src_name

DO NOT EDIT THIS FILE LOCALLY — edits will be lost on the next sync.

For project-specific extensions or overrides, create a sibling file in
\`conventions-local/\` (or another non-foundation/ folder) that explicitly
documents the deviation. See \`conventions-local/README.md\` (created at
project init) for the override pattern.

To update this file: \`./scripts/sync-foundation.sh\`
============================================================================
-->

EOF
}

# ---------------------------------------------------------------------------
# Expand profile paths with language substitutions
# ---------------------------------------------------------------------------
expand_paths() {
    local profile_file="$1"
    while IFS= read -r line; do
        relpath="$(echo "$line" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
        [[ -z "$relpath" || "$relpath" =~ ^# ]] && continue
        # Expand {language} placeholder for each configured language
        if [[ "$relpath" == *"{language}"* ]]; then
            for lang in $LANGUAGES; do
                echo "${relpath//\{language\}/$lang}"
            done
        else
            echo "$relpath"
        fi
    done < "$profile_file"
}

# ---------------------------------------------------------------------------
# Sync files: overlay each upstream in order
# ---------------------------------------------------------------------------
SYNC_COUNT=0
SKIP_COUNT=0
mkdir -p foundation

# Track which files have been synced (for later upstream overlays)
declare -A SYNCED_FILES

# First upstream uses the profile manifest
FIRST_PROFILE_FILE="$PROFILE_FILE"

for i in $(seq 0 $((UPSTREAM_COUNT - 1))); do
    name="${UP_NAMES[$i]}"
    url="${UP_URLS[$i]}"
    ver="${UP_VERSIONS[$i]}"
    tmpdir="${UP_TMPDIRS[$i]}"

    if [[ $i -eq 0 ]]; then
        # Foundation upstream: use profile manifest
        echo ""
        echo "--- Syncing $name (foundation profile: $PROFILE) ---"
        EXPANDED_PATHS=$(expand_paths "$FIRST_PROFILE_FILE")
    else
        # Subsequent upstreams: overlay all payload files (org conventions)
        echo ""
        echo "--- Overlaying $name (org conventions) ---"
        if [[ -d "$tmpdir/payload" ]]; then
            EXPANDED_PATHS=$(cd "$tmpdir/payload" && find . -type f | sed 's|^\./||')
        else
            echo "  (no payload directory found, skipping)"
            continue
        fi
    fi

    while IFS= read -r relpath; do
        [[ -z "$relpath" ]] && continue

        src="$tmpdir/payload/$relpath"
        dst="foundation/$relpath"

        if [[ ! -f "$src" ]]; then
            # For foundation upstream, this is a profile issue
            if [[ $i -eq 0 ]]; then
                echo "  SKIP (not in payload): $relpath"
                SKIP_COUNT=$((SKIP_COUNT + 1))
            fi
            continue
        fi

        if $DRY_RUN; then
            if [[ -f "$dst" ]]; then
                echo "  WOULD UPDATE: foundation/$relpath (from $name)"
            else
                echo "  WOULD CREATE: foundation/$relpath (from $name)"
            fi
        else
            mkdir -p "$(dirname "$dst")"
            { build_header "$name" "$url" "$ver"; cat "$src"; } > "$dst"
            SYNCED_FILES["$relpath"]=1
            if [[ $i -gt 0 ]]; then
                echo "  overlaid ($name): foundation/$relpath"
            else
                echo "  synced: foundation/$relpath"
            fi
        fi

        SYNC_COUNT=$((SYNC_COUNT + 1))
    done <<< "$EXPANDED_PATHS"
done

# ---------------------------------------------------------------------------
# Self-update: copy the new sync.sh from the foundation upstream.
# Skip on rollback: if we're going BACK to an older version, keep the more
# recent sync.sh we already have.
# ---------------------------------------------------------------------------
if [[ -f "${UP_TMPDIRS[0]}/scripts/sync.sh" ]] && ! $DRY_RUN; then
    CURRENT_VERSION=$(json_get "$FOUNDEOF" "project.version" "")
    TARGET_V="${UP_VERSIONS[0]}"

    if [[ -n "$CURRENT_VERSION" && "$CURRENT_VERSION" != "$TARGET_V" ]]; then
        NEWER=$(printf '%s\n%s\n' "$CURRENT_VERSION" "$TARGET_V" | sort -V | tail -1)
        if [[ "$NEWER" == "$CURRENT_VERSION" ]]; then
            echo "  skip self-update: keeping sync.sh from v$CURRENT_VERSION (newer than target v$TARGET_V)"
        else
            mkdir -p scripts
            cp "${UP_TMPDIRS[0]}/scripts/sync.sh" scripts/sync-foundation.sh
            chmod +x scripts/sync-foundation.sh
            echo "  self-updated: scripts/sync-foundation.sh"
        fi
    else
        mkdir -p scripts
        cp "${UP_TMPDIRS[0]}/scripts/sync.sh" scripts/sync-foundation.sh
        chmod +x scripts/sync-foundation.sh
        echo "  self-updated: scripts/sync-foundation.sh"
    fi
fi

# ---------------------------------------------------------------------------
# Update .foundation with new versions
# ---------------------------------------------------------------------------
if ! $DRY_RUN; then
    TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # Build updated upstreams with resolved versions
    UPDATED_UPSTREAMS_JSON=$(python3 -c "
import json, sys
upstreams = json.loads(sys.argv[1])
for i, ver in enumerate(sys.argv[2:]):
    if i < len(upstreams):
        upstreams[i]['version'] = ver
print(json.dumps(upstreams))
" "$UPSTREAMS_JSON" "${UP_VERSIONS[@]}")

    python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    d = json.load(f)
d['project']['version'] = sys.argv[2]
d['project']['appliedAt'] = sys.argv[3]
d['sync']['upstreams'] = json.loads(sys.argv[4])
with open(sys.argv[1], 'w') as f:
    json.dump(d, f, indent=2)
    f.write('\n')
" "$FOUNDEOF" "${UP_VERSIONS[0]}" "$TIMESTAMP" "$UPDATED_UPSTREAMS_JSON"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
if $DRY_RUN; then
    echo "DRY RUN: $SYNC_COUNT files would be synced, $SKIP_COUNT skipped (not in payload)."
    echo "Run without --dry-run to apply."
else
    echo "Synced $SYNC_COUNT files (profile: $PROFILE, upstreams: ${UP_NAMES[*]})."
    if [[ "$SKIP_COUNT" -gt 0 ]]; then
        echo "  ($SKIP_COUNT files in profile were not present in the payload — see warnings above.)"
    fi
    echo ""
    echo "Review changes:  git diff foundation/"
    echo "Roll back:       git restore foundation/ .foundation"
fi

} # end of in-memory body wrapper
exit 0
