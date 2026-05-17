# Code Conventions — PowerShell

**Status:** Living document.
**Audience:** Anyone writing PowerShell code in this project — humans and AI assistants generating code.
**Scope:** PowerShell style, module/script layout, parameter design, error handling, testing.
**Companion docs:** Other languages have their own `CODE_CONVENTIONS-<language>.md`. For naming of files and folders, see `NAMING_CONVENTIONS.md`. For HTTP API and JSON wire format conventions, see `API_CONVENTIONS.md`.

When this document and Microsoft's official PowerShell guidelines conflict, this document wins (rare). When this document is silent, follow Microsoft's [PowerShell strongly encouraged development guidelines](https://learn.microsoft.com/powershell/scripting/developer/cmdlet/strongly-encouraged-development-guidelines), the [PowerShell Practice and Style](https://poshcode.gitbook.io/powershell-practice-and-style/) community guide, and the rules enforced by **PSScriptAnalyzer**.

---

## 1. Versions and compatibility

**Primary target:** PowerShell 7+ (cross-platform, modern syntax, modern features).

**Compatibility note:** Windows PowerShell 5.1 is still installed by default on every Windows machine. Code that needs to run on stock Windows without installing PS 7 has different constraints. This doc calls out the deviations explicitly with **`[5.1 note]`** markers.

When a script or module must support 5.1, add this header:

```powershell
#Requires -Version 5.1
```

When PS 7+ is required:

```powershell
#Requires -Version 7.0
```

Always declare the requirement explicitly. Don't rely on consumers guessing.

---

## 2. Style basics

### 2.1 Naming

PowerShell's naming conventions are stricter than most languages — and they're load-bearing for tab completion, `Get-Help`, and module autoloading.

- **Functions and cmdlets:** `Verb-Noun` in `PascalCase` (`Get-Item`, `New-Configuration`, `Remove-StaleLock`).
- **Verbs:** must come from the approved verb list (see §2.2). PSScriptAnalyzer warns on unapproved verbs.
- **Nouns:** singular, `PascalCase` (`Get-Item` not `Get-Items`; the cmdlet's pipeline behavior covers plural usage).
- **Parameters:** `PascalCase` (`-ItemKey`, `-MaxRetryCount`). Match the names PowerShell uses in built-in cmdlets (`-Path`, `-Name`, `-Force`) when your parameter has the same meaning.
- **Variables:** `$PascalCase` for exported / public / important; `$camelCase` is also acceptable for short-lived locals — pick one style per module and stay consistent.
- **Script-scope variables:** `$script:Name` to make the scope explicit.
- **Constants** (set once, never reassigned): `$PascalCase` declared with `Set-Variable -Option Constant` or the `[ValidateNotNull()]` attribute on a script-scope variable.
- **Hashtable keys:** `PascalCase` for keys you control. Keys that map to API wire format follow that wire format (camelCase for JSON bodies per `API_CONVENTIONS.md`).
- **Files:** `PascalCase.ps1`, `PascalCase.psm1`, `PascalCase.psd1` (per PowerShell convention; the `NAMING_CONVENTIONS.md` kebab-case rule does not apply here — PowerShell file names are part of the language ecosystem and follow PS conventions).

### 2.2 Approved verbs

Use only verbs from `Get-Verb`. The common ones, by group:

- **Common:** `Get`, `Set`, `New`, `Remove`, `Add`, `Clear`, `Copy`, `Move`, `Rename`, `Find`.
- **Lifecycle:** `Start`, `Stop`, `Restart`, `Suspend`, `Resume`, `Enable`, `Disable`, `Install`, `Uninstall`.
- **Data:** `Import`, `Export`, `ConvertTo`, `ConvertFrom`, `Backup`, `Restore`, `Compress`, `Expand`.
- **Diagnostic:** `Test`, `Trace`, `Measure`, `Debug`, `Repair`, `Resolve`.
- **Communications:** `Connect`, `Disconnect`, `Send`, `Receive`, `Sync`.

If you're tempted to use a verb that isn't on the list, look at the list again — there's almost always a fit. `Get` instead of `Fetch`. `New` instead of `Create`. `Remove` instead of `Delete`. `Set` instead of `Update`.

### 2.3 Indentation and line length

- 4 spaces (never tabs).
- Soft 120-character line limit.
- Open brace on the same line (K&R / OTBS style); close brace on its own line:

```powershell
function Get-Thing {
    param([string]$Name)

    if ($Name) {
        return $Name
    }
    else {
        return $null
    }
}
```

`elseif` and `else` go on their own lines. PSScriptAnalyzer's `PSPlaceCloseBrace` and `PSPlaceOpenBrace` rules enforce this.

### 2.4 String quoting

- **Single quotes (`'...'`)** when there's no interpolation. Cheaper, no surprises.
- **Double quotes (`"..."`)** when interpolating variables or expressions.
- **Here-strings (`@"..."@` or `@'...'@`)** for multi-line strings.

```powershell
# Good
$message = 'Acquired lock'
$message = "Acquired lock for $itemKey"
$message = "Acquired lock for $($item.Key)"

# Bad — no interpolation needed
$message = "Acquired lock"
```

For SQL via `Invoke-Sqlcmd` or `Invoke-DbaQuery`, **never** use string interpolation to inject values. Use parameters:

```powershell
# Good
Invoke-Sqlcmd -Query 'SELECT * FROM items WHERE id = @id' `
              -Variable @{ id = $itemId }

# Bad — SQL injection vector
Invoke-Sqlcmd -Query "SELECT * FROM items WHERE id = '$itemId'"
```

### 2.5 Aliases

**Don't use aliases in committed code.** They save a few keystrokes at the REPL but harm readability:

```powershell
# Bad — aliases everywhere
gci $path | ? { $_.Length -gt 1kb } | % { $_.Name }

# Good — full names
Get-ChildItem -Path $path |
    Where-Object { $_.Length -gt 1kb } |
    ForEach-Object { $_.Name }
```

PSScriptAnalyzer's `PSAvoidUsingCmdletAliases` rule flags these.

The exception: `?` and `%` are *so* commonly used at the REPL that you'll see them in some codebases. Even so — committed code uses the full names.

### 2.6 Pipeline formatting

Long pipelines break across lines, with the pipe at the end of the previous line:

```powershell
Get-ChildItem -Path $path -Recurse |
    Where-Object { $_.Extension -eq '.log' } |
    Sort-Object -Property LastWriteTime -Descending |
    Select-Object -First 10
```

Don't put the pipe at the start of the next line — PowerShell parses it as the start of a statement.

---

## 3. Type system and parameters

PowerShell is dynamically typed by default but supports strong typing on parameters, variables, and return values. **Use it.**

### 3.1 Always declare parameter types

```powershell
function Get-Item {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory, Position = 0)]
        [ValidateNotNullOrEmpty()]
        [string]$Name,

        [Parameter()]
        [ValidateRange(1, 100)]
        [int]$MaxResults = 10,

        [Parameter()]
        [switch]$IncludeArchived
    )
    # ...
}
```

`[CmdletBinding()]` makes the function behave like a compiled cmdlet — gets `-Verbose`, `-ErrorAction`, etc. for free. **Every function that takes parameters should have it.**

### 3.2 Validation attributes

Use validation attributes; they enforce constraints at parameter binding rather than buried in the body:

- `[ValidateNotNull()]`, `[ValidateNotNullOrEmpty()]`
- `[ValidateRange(min, max)]`
- `[ValidateSet('Foo', 'Bar', 'Baz')]`
- `[ValidatePattern('^[a-z]+$')]`
- `[ValidateLength(min, max)]`
- `[ValidateScript({ Test-Path $_ })]`

```powershell
[Parameter(Mandatory)]
[ValidateSet('Draft', 'PendingReview', 'Approved', 'Archived')]
[string]$Status
```

### 3.3 Pipeline input

Functions that operate on pipeline input declare it explicitly:

```powershell
function Set-ItemStatus {
    [CmdletBinding(SupportsShouldProcess)]
    param(
        [Parameter(Mandatory, ValueFromPipeline, ValueFromPipelineByPropertyName)]
        [string]$Id,

        [Parameter(Mandatory)]
        [ValidateSet('Draft', 'Approved', 'Archived')]
        [string]$Status
    )

    process {
        # called once per pipeline item
        if ($PSCmdlet.ShouldProcess($Id, "Set status to $Status")) {
            # ... do the work ...
        }
    }
}
```

The `begin`/`process`/`end` blocks are required when accepting pipeline input. `process` runs per item; `begin` and `end` run once.

### 3.4 `[OutputType()]`

Declare what the function returns. It's discoverable via `Get-Help` and tab completion sees through it:

```powershell
function Get-Item {
    [CmdletBinding()]
    [OutputType([PSCustomObject])]
    param(...)
    # ...
}
```

For typed objects, use the actual type: `[OutputType([System.IO.FileInfo])]`.

### 3.5 PSCustomObject vs class vs hashtable

- **`[PSCustomObject]@{...}`** — the default for ad-hoc structured output. Has property semantics, plays well with the pipeline.
- **`class`** (PS 5+) — for richer types with methods, validation, inheritance. Use when you need behavior, not just data.
- **Hashtable `@{...}`** — for keyword-argument-style passing (splatting), not for structured return values.

```powershell
# Return PSCustomObject from a function
function Get-Status {
    [PSCustomObject]@{
        Status     = 'Approved'
        UpdatedAt  = Get-Date
        UpdatedBy  = $env:USER
    }
}

# Use a class when you need methods or constructors
class ItemId {
    [string]$Value

    ItemId([string]$value) {
        if ([string]::IsNullOrWhiteSpace($value)) {
            throw "ItemId cannot be empty"
        }
        $this.Value = $value
    }

    [string] ToString() { return $this.Value }
}
```

### 3.6 Splatting

For long parameter lists, use splatting via a hashtable rather than a long single line:

```powershell
$params = @{
    Path        = '/var/data'
    Recurse     = $true
    Filter      = '*.log'
    ErrorAction = 'Stop'
}
Get-ChildItem @params
```

`@params` (with `@`, not `$`) splats the hashtable as named arguments. Read better than ten params on one line.

---

## 4. Project shape

The project's shape determines its layout. Three shapes are common in PowerShell — **module**, **CLI script**, and **library/automation collection** — and the principle is the same in each: separate the layers, talk only downward.

### 4.1 Module shape (preferred for reusable code)

The standard module layout:

```
MyProject/
  MyProject.psd1            ← Module manifest. Required.
  MyProject.psm1            ← Module loader; dot-sources Public/ + Private/.
  Public/                   ← One .ps1 per exported function.
    Get-Item.ps1
    New-Item.ps1
    Set-ItemStatus.ps1
  Private/                  ← One .ps1 per internal helper.
    ConvertTo-InternalShape.ps1
    Resolve-Connection.ps1
  Classes/                  ← PowerShell classes (if any).
    ItemId.ps1
  Tests/                    ← Pester tests.
    Get-Item.Tests.ps1
    Public.Tests.ps1
  README.md
```

**`MyProject.psm1`** dot-sources every `.ps1` in `Public/` and `Private/`, then exports only the public functions:

```powershell
# MyProject.psm1
$Public  = @( Get-ChildItem -Path "$PSScriptRoot/Public/*.ps1"  -ErrorAction SilentlyContinue )
$Private = @( Get-ChildItem -Path "$PSScriptRoot/Private/*.ps1" -ErrorAction SilentlyContinue )

foreach ($file in @($Public + $Private)) {
    try {
        . $file.FullName
    }
    catch {
        Write-Error "Failed to import $($file.FullName): $_"
    }
}

Export-ModuleMember -Function $Public.BaseName
```

**`MyProject.psd1`** is the manifest. Generate with `New-ModuleManifest`, then edit:

```powershell
@{
    RootModule        = 'MyProject.psm1'
    ModuleVersion     = '0.1.0'
    GUID              = '...'
    Author            = '...'
    Description       = 'Short one-liner.'
    PowerShellVersion = '7.0'
    FunctionsToExport = @('Get-Item', 'New-Item', 'Set-ItemStatus')
    CmdletsToExport   = @()
    VariablesToExport = @()
    AliasesToExport   = @()
}
```

**Always declare `FunctionsToExport` explicitly.** The wildcard form (`'*'`) defeats module autoloading and exposes private helpers. PSScriptAnalyzer's `PSUseToExportFieldsInManifest` flags this.

### 4.2 CLI script shape (single-file tool)

For one-off tools or thin entry points:

```powershell
#!/usr/bin/env pwsh
#Requires -Version 7.0

<#
.SYNOPSIS
Initialise the local data directory.

.DESCRIPTION
Creates the directory layout, writes default config, and seeds the database.

.PARAMETER Path
Where to initialise. Defaults to the current directory.

.PARAMETER Force
Overwrite existing files.

.EXAMPLE
./init.ps1 -Path ./data
#>
[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Position = 0)]
    [string]$Path = (Get-Location).Path,

    [switch]$Force
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# ... do the work ...
```

For CLI tools with multiple subcommands, prefer the **module shape** with a wrapper script that dispatches to module functions. Don't pile multiple commands into one script.

### 4.3 Automation collection (related scripts, not a module)

For a folder of related but standalone automation scripts (typical in operations or DBA contexts):

```
Automation/
  Backup-Database.ps1
  Restore-Database.ps1
  Test-BackupIntegrity.ps1
  _shared/
    Get-Connection.ps1     ← dot-sourced by other scripts
    Write-Log.ps1
  Tests/
```

Each top-level `.ps1` is callable on its own. Shared helpers live in `_shared/` and are dot-sourced explicitly:

```powershell
. "$PSScriptRoot/_shared/Get-Connection.ps1"
. "$PSScriptRoot/_shared/Write-Log.ps1"
```

If the collection grows beyond a handful of scripts and the helpers become structural, **promote the collection to a module**. Don't let it grow unbounded as loose scripts.

### 4.4 The shared layered principle

Whatever the shape, layers talk only downward:

- **Public functions / Script entry points** — thin. Validate input, dispatch, format output.
- **Private helpers** — business logic. No knowledge of the entry layer.
- **IO helpers** — file/network/database access. Called by helpers.

A public function that opens a database connection directly, builds a query, and formats output for the console is doing all three layers' jobs. Push them into separate functions.

---

## 5. Error handling

### 5.1 Set strict defaults

Every script and module function starts with:

```powershell
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
```

- `'Stop'` makes non-terminating errors terminating, so `try/catch` actually catches them.
- `Set-StrictMode -Version Latest` turns "undeclared variable" and "missing property" into errors instead of silent `$null`.

These two settings catch a huge class of bugs early. They're effectively required.

### 5.2 `try/catch/finally`

```powershell
try {
    $item = Get-Item -Path $Path -ErrorAction Stop
    Process-Item -Item $item
}
catch [System.IO.FileNotFoundException] {
    Write-Error "Item not found: $Path"
    return
}
catch {
    Write-Error "Unexpected error processing $Path : $_"
    throw   # re-throw to caller
}
finally {
    if ($connection) { $connection.Dispose() }
}
```

Catch specific exception types when you can handle them differently. Use a bare `catch` for the catch-all; either re-throw or return cleanly — don't silently swallow.

### 5.3 `throw` vs `Write-Error`

- **`throw`** — terminates execution. Use inside functions for unrecoverable conditions.
- **`Write-Error`** — emits a non-terminating error record. Use when the function is callable in pipelines and the caller may want to continue with the next item.

```powershell
# Terminating — caller can catch but cannot ignore
if (-not (Test-Path $Path)) {
    throw "Path does not exist: $Path"
}

# Non-terminating — pipeline can continue with -ErrorAction Continue
if ($item.Status -eq 'Invalid') {
    Write-Error "Item $($item.Id) has invalid status; skipping"
    continue
}
```

### 5.4 Don't suppress with `-ErrorAction SilentlyContinue`

Suppressing errors hides bugs. If you genuinely want "ignore if missing," do it explicitly:

```powershell
# Bad
$file = Get-Content -Path $Path -ErrorAction SilentlyContinue

# Good — ask the question, don't swallow the answer
if (Test-Path $Path) {
    $file = Get-Content -Path $Path
}
```

The exception is when the cmdlet *only* signals via error and there's no `Test-*` equivalent — and even then, document why the suppression is correct.

---

## 6. Persistence and external IO

PowerShell talks to many external systems. The principle from `DESIGN_PHILOSOPHY.md` ("the external-dep leak") applies: **wrap external dependencies in your own helper, don't sprinkle external client calls through application code.**

### 6.1 SQL

Use `Invoke-Sqlcmd` (built-in for SQL Server) or **dbatools** (for richer cross-DB support). Wrap the call:

```powershell
function Invoke-ProjectQuery {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Query,

        [hashtable]$Parameters = @{}
    )

    $connectionString = $script:ConnectionString
    Invoke-Sqlcmd -ConnectionString $connectionString `
                  -Query $Query `
                  -Variable $Parameters `
                  -ErrorAction Stop
}
```

Application code calls `Invoke-ProjectQuery`; it doesn't call `Invoke-Sqlcmd` directly. When you swap the database driver, only the wrapper changes.

### 6.2 JSON

```powershell
# Reading
$config = Get-Content -Path $configPath -Raw | ConvertFrom-Json

# Writing — ALWAYS specify Depth (default is 2, which silently truncates)
$config | ConvertTo-Json -Depth 10 | Set-Content -Path $configPath
```

The default `-Depth 2` for `ConvertTo-Json` is a sharp edge — nested objects beyond two levels become `System.Collections.Hashtable` strings. Always set `-Depth` explicitly.

In PS 7+, `ConvertFrom-Json -AsHashtable` returns hashtables instead of `PSCustomObject`s, which is often what you want for editing then re-serializing.

### 6.3 HTTP

`Invoke-RestMethod` for JSON APIs; `Invoke-WebRequest` for raw responses. Wrap the call when used in more than one place:

```powershell
function Invoke-MyApiRequest {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Path,

        [ValidateSet('GET', 'POST', 'PUT', 'PATCH', 'DELETE')]
        [string]$Method = 'GET',

        [hashtable]$Body
    )

    $params = @{
        Uri         = "$script:BaseUrl$Path"
        Method      = $Method
        Headers     = @{ Authorization = "Bearer $script:Token" }
        ContentType = 'application/json'
        ErrorAction = 'Stop'
    }
    if ($Body) {
        $params.Body = $Body | ConvertTo-Json -Depth 10
    }

    Invoke-RestMethod @params
}
```

### 6.4 Files and paths

- Use `Join-Path` to construct paths cross-platform: `Join-Path $PSScriptRoot 'data' 'config.json'`.
- Use `$PSScriptRoot` (the directory of the running script) over the deprecated `$MyInvocation.MyCommand.Path`.
- Use `Resolve-Path` to normalize, but only after confirming the path exists; `Resolve-Path` errors on non-existent paths.

---

## 7. Async and parallelism

PowerShell's concurrency story is more limited than the other languages here.

### 7.1 `ForEach-Object -Parallel` (PS 7+)

For data-parallel work over a collection:

```powershell
# PS 7+ only
$results = $items | ForEach-Object -Parallel {
    Process-Item -Item $_
} -ThrottleLimit 5
```

**`[5.1 note]`** Not available in PowerShell 5.1. The 5.1-compatible alternative is `Start-ThreadJob` or runspace pools — both more code.

### 7.2 Background jobs

`Start-Job` runs work in a separate process; `Start-ThreadJob` (from the `ThreadJob` module, built into PS 7) runs in a separate thread within the same process and is cheaper:

```powershell
$job = Start-ThreadJob -ScriptBlock { Start-Sleep 5; "done" }
$result = Receive-Job -Job $job -Wait -AutoRemoveJob
```

### 7.3 Don't fight PowerShell's concurrency model

If your workload demands genuine async I/O, parallel pipelines, or producer-consumer patterns, **PowerShell may be the wrong tool**. C#, Rust, or even Python will give you better concurrency primitives. Use PowerShell where its strengths are: shell composability, cmdlet ecosystem, ops automation.

---

## 8. Testing

Use **Pester v5** (PS 7+ ships with Pester 5; install explicitly on 5.1).

### 8.1 Layout

```
Tests/
  Get-Item.Tests.ps1
  Set-ItemStatus.Tests.ps1
  Module.Tests.ps1            ← module-level tests (manifest, exports)
```

One test file per public function, named `<FunctionName>.Tests.ps1`. A `Module.Tests.ps1` validates the manifest and exported function list.

### 8.2 Test structure

```powershell
BeforeAll {
    $modulePath = Join-Path $PSScriptRoot '..' 'MyProject.psd1'
    Import-Module $modulePath -Force
}

Describe 'Get-Item' {
    Context 'When the item exists' {
        It 'Returns the item' {
            $result = Get-Item -Id 'abc-123'
            $result | Should -Not -BeNullOrEmpty
            $result.Id | Should -Be 'abc-123'
        }
    }

    Context 'When the item does not exist' {
        It 'Throws ItemNotFoundException' {
            { Get-Item -Id 'missing' } | Should -Throw -ExpectedMessage '*not found*'
        }

        It 'Returns null when -ErrorAction SilentlyContinue' {
            Get-Item -Id 'missing' -ErrorAction SilentlyContinue | Should -BeNullOrEmpty
        }
    }
}
```

`Describe` per function or feature; `Context` per scenario; `It` per assertion. Test names complete the sentence "It...":

```powershell
It 'Returns the item' { ... }
It 'Throws when not found' { ... }
```

Not:

```powershell
It 'Test1' { ... }            # bad — meaningless
It 'GetItemHappy' { ... }     # bad — wrong style
```

### 8.3 Mocking

Pester's `Mock` replaces a cmdlet for the scope:

```powershell
Describe 'Save-Report' {
    BeforeAll {
        Mock Invoke-RestMethod { @{ status = 'ok' } }
    }

    It 'Calls the API once' {
        Save-Report -Data $sample
        Should -Invoke Invoke-RestMethod -Times 1 -Exactly
    }
}
```

Mock at the boundary — the wrapper functions in §6, not the underlying cmdlets, when possible.

### 8.4 Error path coverage

Every happy path needs at least two error path tests covering:
- Bad input (validation failure / mandatory parameter missing).
- External failure (file missing, API error, db error).

This is a hard rule. Error-path gaps are defects.

---

## 9. Documentation

### 9.1 Comment-based help

Every public function has comment-based help directly above the `param()` block (inside the function, before `param`):

```powershell
function Get-Item {
    <#
    .SYNOPSIS
    Retrieve an item by its identifier.

    .DESCRIPTION
    Looks up an item by ID, returning a structured object. If the item is not
    found, throws ItemNotFoundException.

    .PARAMETER Id
    The item identifier. Must be a non-empty string.

    .PARAMETER IncludeArchived
    Include archived items in the lookup. Default is to exclude them.

    .EXAMPLE
    Get-Item -Id 'abc-123'

    Retrieves the item with ID 'abc-123'.

    .EXAMPLE
    Get-Item -Id 'abc-123' -IncludeArchived

    Retrieves the item even if archived.

    .OUTPUTS
    PSCustomObject with properties Id, Key, Status, CreatedAt.

    .NOTES
    Requires connection configured via Set-MyProjectConfiguration.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [ValidateNotNullOrEmpty()]
        [string]$Id,

        [switch]$IncludeArchived
    )
    # ...
}
```

`Get-Help Get-Item -Full` should produce useful output. PSScriptAnalyzer's `PSProvideCommentHelp` rule flags missing help on exported functions.

### 9.2 Inline comments

Comments explain **why**, not **what**:

```powershell
# Bad — restates the code
$i++  # increment i

# Good — explains the why
# Off-by-one: indices in this dataset are 1-based per the upstream spec.
$i++
```

If the code itself isn't self-explanatory, often the right fix is renaming a variable or extracting a function, not adding a comment.

---

## 10. PSScriptAnalyzer

`PSScriptAnalyzer` is the .NET-equivalent linter for PowerShell. **It's required.**

Run it in CI:

```powershell
Invoke-ScriptAnalyzer -Path . -Recurse -Settings PSGallery
```

Treat warnings as errors in CI. Don't `[Diagnostics.CodeAnalysis.SuppressMessageAttribute]` without a comment explaining why the rule doesn't apply.

A `PSScriptAnalyzerSettings.psd1` at the project root pins the rule set:

```powershell
@{
    Severity     = @('Error', 'Warning')
    IncludeRules = @('PSAvoidUsingCmdletAliases', 'PSUseDeclaredVarsMoreThanAssignments', ...)
    ExcludeRules = @()  # if you must exclude one, document why
}
```

---

## 11. Why conventions matter even in a single project

It's tempting to think conventions only matter when multiple people or systems collaborate. They matter inside a single project too:

- **AI assistants generate code that matches the conventions of the code they see.** Drifty conventions produce drifty generated code. Tight conventions produce tight generated code.
- **`PSScriptAnalyzer` is part of the conventions.** Run it in CI; treat warnings as errors. Catching style drift early is cheaper than a rewrite later.
- **PowerShell's tab completion and `Get-Help` reward convention compliance.** `Verb-Noun` naming, `[CmdletBinding()]`, comment-based help, validation attributes — these aren't decoration; they make the cmdlets discoverable and self-documenting.
- **Future-you is a different collaborator.** Code you'll be reading in six months was written by someone with different context. Conventions are the shared assumptions that let future-you understand what you wrote.

Reference these conventions in design docs rather than restating them. When they need to change, update the doc and any in-flight designs that depend on them.

---

## 12. Cross-references

- For naming of files and folders (NOTE: PowerShell file names follow PowerShell convention — `PascalCase.ps1` — and override the kebab-case rule in `NAMING_CONVENTIONS.md` for `.ps1`/`.psm1`/`.psd1` files): `NAMING_CONVENTIONS.md`.
- For HTTP API and JSON wire format conventions: `API_CONVENTIONS.md`.
- For other languages: `CODE_CONVENTIONS-<language>.md`.
- For the design values that motivate code-level discipline: `../DESIGN_PHILOSOPHY.md` (especially "Conventions are constitutional" and "Bootstrap-residue is a smell").
- For the structural foundation that governs design docs and contracts: `../designs/design-000-meta.md`.
- For Microsoft's strongly-encouraged PowerShell development guidelines: <https://learn.microsoft.com/powershell/scripting/developer/cmdlet/strongly-encouraged-development-guidelines>.
- For the PowerShell Practice and Style guide: <https://poshcode.gitbook.io/powershell-practice-and-style/>.
- For PSScriptAnalyzer rules: <https://github.com/PowerShell/PSScriptAnalyzer/blob/main/docs/Rules/README.md>.
