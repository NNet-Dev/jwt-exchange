
# Code Conventions — Bash

**Status:** Living document.
**Audience:** Anyone writing Bash scripts in this project — humans and AI assistants generating code.
**Scope:** Bash style, script layout, parameter handling, error handling, quoting, subprocess execution. Defers to [Google Shell Style Guide](https://google.github.io/styleguide/shellguide.html) for things not covered here.
**Companion docs:** Other languages have their own `CODE_CONVENTIONS-<language>.md`. For naming of files and folders, see `NAMING_CONVENTIONS.md`. For HTTP API and JSON wire format conventions, see `API_CONVENTIONS.md`.

When this document and the Google Shell Style Guide conflict, this document wins (this is rare). When this document is silent, the Google Shell Style Guide applies.

---

## 1. Shell and version

### 1.1 Target shell

**Primary target:** Bash 4.0+ on Linux. Scripts must declare their interpreter:

```bash
#!/usr/bin/env bash
```

Use `#!/usr/bin/env bash` rather than `#!/bin/bash` — the latter may point to an older Bash on macOS/BSD systems where Bash lives in `/usr/local/bin/bash` or similar. `env` finds the first `bash` in `$PATH`, which is the right behavior for a portable script.

### 1.2 `shellcheck`

**Required.** Run `shellcheck` on every script. Don't add `# shellcheck disable=SCXXXX` without a comment explaining why. The default shellcheck configuration is the convention.

```bash
shellcheck --shell=bash --severity=warning scripts/*.sh
```

Treat all `shellcheck` warnings and errors as build failures.

### 1.3 Strict mode

Every script that is not a trivial one-liner should enable strict mode at the top:

```bash
#!/usr/bin/env bash
set -euo pipefail
```

- `-e`: Exit on any unhandled error. Don't let scripts continue after a command fails.
- `-u`: Treat unset variables as errors. Catch typos early (`$FOO` vs `$FO0`).
- `-o pipefail`: The exit status of a pipeline is the last non-zero exit status of any command in it. Without this, `false | true` returns 0.

**Exceptions to `-e`:** When a command's failure is expected and handled:

```bash
# Good — explicit check
if ! command -v jq &>/dev/null; then
    echo "jq is required" >&2
    exit 1
fi

# Good — expected failure
rm -f "$tmpfile" || true

# Bad — silently ignores failure
some_important_command
```

When you need to intentionally allow a command to fail in a `set -e` script, append `|| true` or wrap in `if`.

---

## 2. Style basics

### 2.1 Naming

- **Script files:** `kebab-case.sh` (`sync-foundation.sh`, `build-artifact.sh`). Per `NAMING_CONVENTIONS.md`, file names are kebab-case.
- **Functions:** `snake_case` (`acquire_lock`, `format_output`, `_internal_helper`).
- **Variables:** `snake_case` in lowercase (`item_key`, `request_id`).
- **Constants and readonly variables:** `UPPERCASE_SNAKE_CASE` (`DEFAULT_TTL_SECONDS`, `MAX_RETRIES`, `SCRIPT_DIR`).
- **Private/internal functions:** prefix with underscore (`_resolve_id`, `_format_response`). This is a convention, not enforcement — the underscore signals "not part of the public interface of this script."

### 2.2 Indentation and line length

- 2 spaces (never tabs). Bash nesting can go deep quickly; 4-space indentation makes wide scripts harder to read in terminals.
- 100-character soft line limit. Long lines in scripts are often a sign that a variable should be extracted or a command split across lines.
- Backslash continuation for long commands:

```bash
# Good
curl -s -X POST "${API_URL}/items" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${API_TOKEN}" \
    -d "${payload}"

# Bad — one massive line
curl -s -X POST "${API_URL}/items" -H "Content-Type: application/json" -H "Authorization: Bearer ${API_TOKEN}" -d "${payload}"
```

### 2.3 Comments

- `#` for comments. No `/* */` (not supported in Bash).
- Functions get a brief comment describing purpose, arguments, and exit status:

```bash
# acquire_lock — Attempt to acquire a file-based lock.
# Arguments:
#   $1 - Lock file path
#   $2 - TTL in seconds (default: 600)
# Returns:
#   0 if lock acquired, 1 if already held
acquire_lock() {
    ...
}
```

- Inline comments explain *why*, not *what*. The code says what; the comment says why this unusual approach was chosen.

---

## 3. Quoting and expansion

### 3.1 Always quote variable expansions

```bash
# Good
echo "${HOME}/projects"
cp "$source" "$destination"

# Bad — breaks on spaces, glob characters, and empty values
echo $HOME/projects
cp $source $destination
```

The rule is: **every variable expansion gets double quotes**, except when you *explicitly* want word splitting and globbing (rare).

```bash
# Correct: intentional word splitting (e.g., passing args to a command)
files=("file one.txt" "file two.txt")
rm -f -- "${files[@]}"

# Correct: intentional globbing
for f in *.log; do
    echo "$f"
done
```

### 3.2 Use `${var}` not `$var`

Always use the brace form inside strings. It's unambiguous and works correctly with adjacent characters:

```bash
# Good
echo "${name}_backup.tar.gz"

# Bad — ambiguous: is this $name_backup or $name followed by _backup?
echo "$name_backup.tar.gz"
```

### 3.3 Default values and error on unset

```bash
# Use default if unset or empty
echo "${PORT:-8080}"

# Error and exit if unset or empty
echo "${API_TOKEN:?API_TOKEN must be set}"

# Use default if unset (but keep empty string)
echo "${NAME-}"
```

### 3.4 Single vs double quotes

- **Double quotes:** when you need variable expansion or command substitution (`"${HOME}"`, "today is $(date +%F)").
- **Single quotes:** for literal strings where no expansion should happen (`'literal $HOME'`, `'*.txt'`).
- **`$'...'`:** for strings with escape sequences (`$'line1\nline2'`, `$'tab\there'`).

---

## 4. Functions

### 4.1 Declaration style

```bash
# Good — portable, clear
my_function() {
    ...
}

# Avoid — looks like other languages, less idiomatic in Bash
function my_function {
    ...
}
```

The `name() { ... }` form is POSIX-compatible and universally recognized.

### 4.2 Arguments

Functions receive positional parameters (`$1`, `$2`, ...). **Never** use global variables to pass data into a function when an argument is cleaner:

```bash
# Good — explicit arguments
format_item() {
    local name="$1"
    local status="$2"
    echo "${name} [${status}]"
}

# Bad — relies on globals
format_item() {
    echo "${NAME} [${STATUS}]"
}
```

### 4.3 Local variables

Always declare function-scoped variables with `local`:

```bash
process_file() {
    local file_path="$1"
    local line_count
    line_count=$(wc -l < "$file_path")
    echo "${file_path}: ${line_count} lines"
}
```

Without `local`, variables leak into the global scope and can overwrite caller state. This is the most common source of Bash bugs in large scripts.

### 4.4 Return values

Bash functions return exit codes (0–255) via `return`. To return data:

```bash
# Good — print to stdout, capture with $()
get_temp_dir() {
    local dir
    dir=$(mktemp -d)
    echo "$dir"
}

temp=$(get_temp_dir)

# Good — set a named variable by reference (for complex return)
parse_config() {
    local config_file="$1"
    # Sets CONFIG_HOST, CONFIG_PORT, CONFIG_DB
    while IFS='=' read -r key value; do
        case "$key" in
            host) CONFIG_HOST="$value" ;;
            port) CONFIG_PORT="$value" ;;
            db)   CONFIG_DB="$value" ;;
        esac
    done < "$config_file"
}

parse_config "config.ini"
echo "Connecting to ${CONFIG_HOST}:${CONFIG_PORT}/${CONFIG_DB}"
```

Never use `echo` for both human-readable output and return data. If a function prints progress messages, send them to stderr:

```bash
build_step() {
    echo "Starting build..." >&2
    local result
    result=$(do_build)
    echo "$result"  # This is the return value
}
```

### 4.5 Argument count validation

```bash
deploy() {
    if [[ $# -ne 2 ]]; then
        echo "Usage: deploy <environment> <version>" >&2
        return 1
    fi
    local env="$1"
    local version="$2"
    ...
}
```

---

## 5. Conditionals

### 5.1 `[[ ]]` over `[ ]`

Always use `[[ ]]` (Bash built-in) over `[ ]` (external `test` command):

```bash
# Good — no quoting needed inside [[ ]], supports && and ||
if [[ -f "$config_file" && -r "$config_file" ]]; then
    ...
fi

# Avoid
if [ -f "$config_file" ] && [ -r "$config_file" ]; then
    ...
fi
```

`[[ ]]` is a Bash keyword, not a command. It doesn't perform word splitting or globbing on its operands, which means fewer quoting pitfalls.

### 5.2 String comparisons

```bash
# Good
if [[ "$status" == "ready" ]]; then
    ...
fi

# String is empty
if [[ -z "$value" ]]; then
    ...
fi

# String is non-empty
if [[ -n "$value" ]]; then
    ...
fi
```

### 5.3 Numeric comparisons

Use `-eq`, `-ne`, `-lt`, `-le`, `-gt`, `-ge` for arithmetic, or `(( ))`:

```bash
# Good — traditional
if [[ $count -gt 0 ]]; then
    ...
fi

# Good — arithmetic context
if (( count > 0 )); then
    ...
fi
```

Inside `(( ))`, variables don't need `$` prefix, and operators are the familiar `>`, `<`, `==`, `!=`.

### 5.4 `case` for multi-way branching

```bash
case "$command" in
    start)
        start_service
        ;;
    stop)
        stop_service
        ;;
    restart)
        stop_service
        start_service
        ;;
    status)
        show_status
        ;;
    *)
        echo "Unknown command: $command" >&2
        exit 1
        ;;
esac
```

### 5.5 Check command availability

```bash
# Good
if ! command -v jq &>/dev/null; then
    echo "Error: jq is required but not installed" >&2
    exit 1
fi
```

Use `command -v` rather than `which` — it's a shell built-in, faster, and more portable across systems.

---

## 6. Loops

### 6.1 `for` over files

```bash
# Good — handles spaces in filenames
for file in *.log; do
    [[ -f "$file" ]] || continue
    process_log "$file"
done
```

Never use `for file in $(ls *.log)` — it breaks on spaces and is an unnecessary subprocess.

### 6.2 `while read` for line processing

```bash
# Good — preserves backslashes, handles last line without newline
while IFS= read -r line || [[ -n "$line" ]]; do
    echo "Processing: $line"
done < "$input_file"
```

- `IFS=` prevents leading/trailing whitespace from being trimmed.
- `-r` prevents backslash interpretation.
- `|| [[ -n "$line" ]]` handles files that don't end with a newline.

### 6.3 `while` with process substitution

```bash
# Good — variables set inside the loop are visible outside
while IFS= read -r line; do
    count=$((count + 1))
done < <(grep -c "pattern" "$file")
```

Prefer `< <(command)` over `command | while read` — the pipe creates a subshell, and variable assignments inside it are lost when the subshell exits.

---

## 7. Arrays

### 7.1 Indexed arrays

```bash
# Declaration
flags=()

# Append
flags+=("--verbose")
flags+=("--output=${output_file}")

# Iterate
for flag in "${flags[@]}"; do
    echo "Flag: $flag"
done

# Length
echo "${#flags[@]} items"
```

Always use `"${array[@]}"` (not `"${array[*]}"`) when iterating — it preserves elements with spaces as separate items.

### 7.2 Associative arrays

```bash
declare -A config
config[host]="localhost"
config[port]="8080"

for key in "${!config[@]}"; do
    echo "${key}=${config[$key]}"
done
```

Associative arrays require Bash 4.0+. Declare the version requirement if using them.

---

## 8. Error handling

### 8.1 `trap` for cleanup

Use `trap` to clean up temporary files on exit:

```bash
#!/usr/bin/env bash
set -euo pipefail

tmpfile=$(mktemp)
trap 'rm -f "$tmpfile"' EXIT

# Do work with $tmpfile
# It gets cleaned up on normal exit, error exit, or Ctrl+C
```

### 8.2 Custom error handler

For scripts that need structured error reporting:

```bash
error() {
    echo "Error: $*" >&2
    exit 1
}

die() {
    local msg="$1"
    local code="${2:-1}"
    echo "Error (exit ${code}): ${msg}" >&2
    exit "$code"
}
```

### 8.3 Trap on ERR

```bash
set -euo pipefail

err_handler() {
    local line="$1"
    echo "Error on line ${line}" >&2
}
trap 'err_handler ${LINENO}' ERR
```

This catches any command failure (covered by `-e`) and reports the line number.

### 8.4 Don't swallow errors in pipelines

```bash
# Good — pipefail catches failures in any stage
result=$(generate_data | filter | transform)

# Bad — without pipefail, only 'transform' exit status matters
result=$(generate_data | filter | transform)  # needs set -o pipefail
```

---

## 9. String manipulation

### 9.1 Bash built-ins over external commands

Use Bash parameter expansion instead of spawning `sed`, `awk`, `cut` when possible:

```bash
# Bash built-in — no subprocess
filename="report.csv.gz"
name="${filename%%.*}"        # report
ext="${filename##*.}"         # gz
base="${filename%.*}"         # report.csv

# Equivalent external commands (avoid)
name=$(echo "$filename" | cut -d. -f1)
ext=$(echo "$filename" | rev | cut -d. -f1 | rev)
```

Common patterns:

```bash
# Remove prefix
path="${url#https://}"          # remove shortest match from start

# Remove suffix
name="${file%.txt}"             # remove shortest match from end

# Replace first occurrence
fixed="${text/foo/bar}"

# Replace all occurrences
fixed="${text//foo/bar}"

# Length
len="${#text}"

# Substring
sub="${text:0:10}"              # first 10 chars
sub="${text: -5}"               # last 5 chars (note the space)
```

### 9.2 String interpolation

```bash
# Good
message="Processing ${count} items from ${source}"

# Bad — string concatenation
message="Processing "
message="${message}${count}"
message="${message} items from ${source}"
```

---

## 10. File and path handling

### 10.1 Always use `--` before arguments

```bash
# Good — handles filenames starting with -
rm -f -- "$file"
cat -- "$file"

# Bad — breaks if $file starts with -
rm -f "$file"
```

### 10.2 Temporary files

```bash
# Good — mktemp handles permissions and collision avoidance
tmpfile=$(mktemp)
tmpdir=$(mktemp -d)

# Bad — predictable names, collision risk
tmpfile="/tmp/my_script_$$"
```

### 10.3 Directory existence

```bash
# Good — create if needed
mkdir -p -- "${output_dir}"

# Check existence
if [[ ! -d "$output_dir" ]]; then
    error "Output directory does not exist: $output_dir"
fi
```

### 10.4 Path resolution

```bash
# Resolve script directory (works with symlinks)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
```

---

## 11. Subprocess execution

### 11.1 Command substitution

```bash
# Good — $() is nestable and readable
count=$(grep -c "pattern" "$file")
config=$(jq -r '.name' <<< "$json")

# Avoid — backticks are hard to nest and less readable
count=`grep -c "pattern" "$file"`
```

### 11.2 Heredocs for multi-line input

```bash
# Good — clear, readable
cat <<EOF > "$config_file"
[database]
host = ${DB_HOST}
port = ${DB_PORT}
name = ${DB_NAME}
EOF
```

Use `<<-EOF` (with a dash) if you want leading tabs to be stripped (useful inside indented blocks). Note: only tabs, not spaces, are stripped.

### 11.3 Here-strings for single-line input

```bash
# Good — no subshell, no echo pipe
result=$(jq -r '.version' <<< "$package_json")

# Equivalent but spawns echo
result=$(echo "$package_json" | jq -r '.version')
```

### 11.4 Exec in place

```bash
# Good — replaces the current process, no zombie shell
exec docker run --rm "${IMAGE_NAME}" "$@"
```

Use `exec` when a script's final action is to hand off to another process and the shell is no longer needed.

---

## 12. Script layout

### 12.1 Standard structure

```bash
#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Script description — what it does and why.
# Usage: script.sh <arg1> <arg2> [--flag]
# =============================================================================

# --- Constants ----------------------------------------------------------------

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_NAME="$(basename "${BASH_SOURCE[0]}")"
readonly DEFAULT_TIMEOUT=30

# --- Functions ----------------------------------------------------------------

usage() {
    cat <<EOF
Usage: ${SCRIPT_NAME} <action> [options]

Actions:
  build    Build the project
  test     Run tests
  deploy   Deploy to target

Options:
  -h, --help    Show this help message
  -v, --verbose Enable verbose output
EOF
}

log() {
    echo "[$(date +%FT%T%z)] $*" >&2
}

error() {
    echo "Error: $*" >&2
    exit 1
}

# --- Argument parsing ---------------------------------------------------------

action=""
verbose=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        build|test|deploy)
            action="$1"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        -v|--verbose)
            verbose=1
            shift
            ;;
        *)
            error "Unknown argument: $1"
            ;;
    esac
done

if [[ -z "$action" ]]; then
    error "No action specified. Use --help for usage."
fi

# --- Main ---------------------------------------------------------------------

main() {
    log "Starting ${SCRIPT_NAME} with action=${action}"

    case "$action" in
        build)  do_build  ;;
        test)   do_test   ;;
        deploy) do_deploy ;;
    esac

    log "Done"
}

main "$@"
```

### 12.2 Section dividers

Use comment blocks to separate sections of a longer script:

```bash
# --- Constants ----------------------------------------------------------------
# --- Functions ----------------------------------------------------------------
# --- Argument parsing ---------------------------------------------------------
# --- Main ---------------------------------------------------------------------
```

This makes navigation in editors straightforward.

### 12.3 Library scripts (sourced, not executed)

Scripts intended to be sourced by other scripts should **not** execute on load:

```bash
#!/usr/bin/env bash
# lib-helpers.sh — Sourced by other scripts. Do not execute directly.

# Guard against direct execution
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "This script should be sourced, not executed" >&2
    exit 1
fi

# ... function definitions ...
```

---

## 13. JSON handling

### 13.1 Always use `jq`

Never use `grep`, `sed`, or `awk` to parse JSON. JSON is not a line-oriented format and regex parsing is fragile:

```bash
# Good
name=$(jq -r '.name' <<< "$json")
count=$(jq '.items | length' <<< "$json")

# Bad — breaks on whitespace changes, nested objects, escaped strings
name=$(echo "$json" | grep '"name"' | sed 's/.*: "//' | sed 's/".*//')
```

### 13.2 Validate JSON

```bash
# Validate before use
if ! jq empty <<< "$json" 2>/dev/null; then
    error "Invalid JSON input"
fi
```

### 13.3 Construct JSON

```bash
# Good — jq builds valid JSON with proper escaping
payload=$(jq -n \
    --arg name "$item_name" \
    --arg status "$item_status" \
    --argjson count "$item_count" \
    '{name: $name, status: $status, count: $count}')

# Bad — manual string construction (breaks on special characters)
payload="{\"name\": \"$item_name\", \"status\": \"$item_status\"}"
```

---

## 14. Portability notes

### 14.1 macOS / BSD differences

When scripts may run on macOS or BSD:

- `sed -i` requires an argument on BSD (`sed -i ''` vs GNU `sed -i`). Prefer:
  ```bash
  # Portable in-place edit
  if sed --version &>/dev/null; then
      sed -i "$pattern" "$file"       # GNU
  else
      sed -i '' "$pattern" "$file"    # BSD/macOS
  fi
  ```
- `stat` has different flags. Use `stat -f` on BSD, `stat -c` on GNU.
- `date` formatting differs. Prefer `date +%s` for epoch timestamps (portable).
- `readlink -f` is GNU-specific. Use `realpath` (may need `brew install coreutils` on macOS) or a custom resolution function.

### 14.2 Declare portability boundaries

If a script is Linux-only, declare it:

```bash
# This script requires GNU/Linux. It uses Bash 4+ features and GNU coreutils.
if [[ "$(uname)" != "Linux" ]]; then
    echo "This script requires Linux" >&2
    exit 1
fi
```

---

## 15. Testing

### 15.1 Use `bats` for script tests

For testable scripts, use [Bats](https://github.com/bats-core/bats-core) (Bash Automated Testing System):

```bash
#!/usr/bin/env bats

@test "acquire_lock creates lock file" {
    run acquire_lock "/tmp/test-lock" 60
    [ "$status" -eq 0 ]
    [ -f "/tmp/test-lock" ]
}

@test "acquire_lock fails when lock is held" {
    touch "/tmp/test-lock"
    run acquire_lock "/tmp/test-lock" 60
    [ "$status" -eq 1 ]
}
```

### 15.2 Test with `shellcheck` in CI

```bash
# Run all shellcheck tests
find . -name '*.sh' -not -path './.git/*' -print0 \
    | xargs -0 shellcheck --shell=bash --severity=warning
```

---

## 16. Anti-patterns

### 16.1 Don't parse `ls`

```bash
# Bad — breaks on spaces, newlines, special chars in filenames
for file in $(ls *.txt); do
    ...
done

# Good
for file in *.txt; do
    [[ -f "$file" ]] || continue
    ...
done
```

### 16.2 Don't use `echo -e`

```bash
# Bad — echo -e behavior varies across implementations
echo -e "line1\nline2"

# Good — use printf or $'' strings
printf "line1\nline2\n"
echo $'line1\nline2'
```

`printf` is more portable and predictable than `echo -e`.

### 16.3 Don't use backticks

```bash
# Bad
result=`command`

# Good
result=$(command)
```

Backticks are harder to nest, harder to read, and harder to quote correctly.

### 16.4 Don't write to files in a pipeline subshell

```bash
# Bad — count is lost (pipe creates subshell)
count=0
echo "a b c" | while read -r word; do
    count=$((count + 1))
done
echo "count=$count"   # Still 0!

# Good — process substitution
count=0
while read -r word; do
    count=$((count + 1))
done < <(echo "a b c")
echo "count=$count"   # 3
```

### 16.5 Don't use `eval` unless necessary

`eval` executes arbitrary strings as code. It's the most dangerous command in Bash. Only use it when there's no alternative, and sanitize inputs first:

```bash
# Bad — arbitrary code execution
eval "$user_input"

# If you must eval, validate first
if [[ "$user_input" =~ ^[a-zA-Z0-9_=-]+$ ]]; then
    eval "$user_input"
else
    error "Invalid input: $user_input"
fi
```

### 16.6 Don't ignore return codes in conditionals

```bash
# Bad — $? captures the test, not the command
some_command
if [[ $? -eq 0 ]]; then
    ...
fi

# Good
if some_command; then
    ...
fi
```

---

## 17. Project shape

### 17.1 Script organization

```
scripts/
  build.sh              ← Main build script.
  test.sh               ← Test runner.
  deploy.sh             ← Deployment script.
  lib/
    helpers.sh          ← Sourced utility functions.
    validators.sh       ← Input validation functions.
```

- Scripts in `scripts/` are executable entry points.
- Files in `scripts/lib/` are sourced by entry points, not executed directly.
- Each entry point sources its dependencies:
  ```bash
  source "$(dirname "${BASH_SOURCE[0]}")/lib/helpers.sh"
  ```

### 17.2 One responsibility per script

A script should do one thing well. If a script is more than ~200 lines, consider splitting it:

- Move helper functions to `lib/helpers.sh`.
- Move validation logic to `lib/validators.sh`.
- Keep the entry point focused on argument parsing and orchestration.
