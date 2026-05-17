# Code Conventions — .NET (C#)

**Status:** Living document.
**Audience:** Anyone writing C# / .NET code in this project — humans and AI assistants generating code.
**Scope:** C# style, project layout, type system usage, error handling, persistence, async, testing.
**Companion docs:** Other languages have their own `CODE_CONVENTIONS-<language>.md`. For naming of files and folders, see `NAMING_CONVENTIONS.md`. For HTTP API and JSON wire format conventions, see `API_CONVENTIONS.md`.

When this document and Microsoft's official C# coding conventions conflict, this document wins (rare). When this document is silent, follow Microsoft's [C# coding conventions](https://learn.microsoft.com/dotnet/csharp/fundamentals/coding-style/coding-conventions) and the .NET runtime team's [coding style](https://github.com/dotnet/runtime/blob/main/docs/coding-guidelines/coding-style.md).

---

## 1. Style basics

### 1.1 Target framework

**.NET 8+ only.** Modern C#, modern features, modern conventions. No .NET Framework 4.x. No .NET Standard targets unless this project is explicitly producing a library that must support older runtimes (and even then, declare and justify in the project's design doc).

What "modern" means here:

- File-scoped namespaces (`namespace MyProject;` not `namespace MyProject { ... }`).
- Implicit usings enabled (`<ImplicitUsings>enable</ImplicitUsings>`).
- Nullable reference types enabled (`<Nullable>enable</Nullable>`).
- Top-level statements for `Program.cs` in service/CLI projects.
- Records for DTOs and value-like types.
- Pattern matching everywhere it improves clarity.
- Required members (`required` keyword, C# 11+).
- Primary constructors for classes (C# 12+) where they don't obscure intent.

### 1.2 Naming

- **Namespaces:** `PascalCase`, dotted hierarchy (`MyProject.Services.Items`).
- **Types** (classes, structs, records, interfaces, enums, delegates): `PascalCase` (`ItemService`, `IRequestHandler`). Interfaces are prefixed with `I`.
- **Methods, properties, events:** `PascalCase` (`AcquireLock`, `IsValid`).
- **Public fields:** `PascalCase`. (Avoid public fields; use properties.)
- **Private fields:** `_camelCase` with leading underscore (`_pool`, `_logger`).
- **Parameters and local variables:** `camelCase` (`itemKey`, `requestId`).
- **Constants:** `PascalCase` (`DefaultTtlSeconds`, `MaxBudgetUsd`). Not `SCREAMING_SNAKE_CASE` — that's a C convention; .NET uses PascalCase for constants.
- **Generic type parameters:** `T` for single, `TKey`/`TValue` for descriptive.
- **Enum members:** `PascalCase` (`ItemStatus.PendingReview`).

### 1.3 File organization

- One top-level type per file. File name matches the type name (`ItemService.cs` contains `class ItemService`).
- File-scoped namespace declaration:

```csharp
namespace MyProject.Services;

public sealed class ItemService
{
    // ...
}
```

- `using` directives at the top, outside the namespace, ordered: `System.*` → other Microsoft → external → local. `dotnet format` enforces this.
- Use `global using` directives sparingly — they hide what's available. Acceptable for truly universal types (`using System;`, `using System.Threading.Tasks;`) which `ImplicitUsings` already covers.

### 1.4 Indentation and line length

- 4 spaces (the .NET default; never tabs).
- Soft 120-character line limit. Don't fight `dotnet format` — if a formatted line is hard to read, restructure the code, not the formatter.
- K&R brace style with opening brace on its own line (the .NET convention; `dotnet format` enforces this). Don't mix Allman and K&R.

### 1.5 Imports (using directives)

```csharp
using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;

using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Logging;

using Dapper;
using FluentValidation;

using MyProject.Domain;
using MyProject.Services;
```

`dotnet format` sorts these. Don't reorder by hand.

### 1.6 String formatting

Interpolated strings (`$""`):

```csharp
// Good
var msg = $"Acquired lock for {itemKey}";
_logger.LogInformation("Acquired lock for {ItemKey}", itemKey);

// Bad — manual concatenation
var msg = "Acquired lock for " + itemKey;

// Bad — old composite formatting where interpolation works
var msg = string.Format("Acquired lock for {0}", itemKey);
```

For logging, use the structured-logging template form (`"... {ItemKey}"`, no `$`) so that the logger captures `ItemKey` as a structured property, not just a formatted string.

For SQL with Dapper or ADO.NET, **never** use string interpolation to inject values into queries. Always use parameters:

```csharp
// Good
var item = await connection.QuerySingleAsync<Item>(
    "SELECT * FROM items WHERE id = @id",
    new { id = itemId });

// Bad — SQL injection vector
var sql = $"SELECT * FROM items WHERE id = '{itemId}'";
```

---

## 2. Type system

C#'s type system in modern .NET is rich. Use it.

### 2.1 Nullable reference types

Enable nullable reference types (`<Nullable>enable</Nullable>`) in every project. The compiler then enforces null-safety:

```csharp
public sealed class ItemService
{
    private readonly PgPool _pool;  // not null

    public Item? FindById(string id) => /* may return null */;

    public Item GetById(string id)  // throws if not found; never null
        => FindById(id) ?? throw new ItemNotFoundException(id);
}
```

Don't suppress nullable warnings with `!` (the null-forgiving operator) without a comment explaining why the value is provably non-null.

### 2.2 Records for DTOs and value-like types

Use `record` (or `record struct`) for DTOs and immutable value-like types. They give you value equality, `with`-expression copying, and concise declarations:

```csharp
public sealed record ItemCreateRequest(
    string ItemKey,
    JsonElement ItemPayload,
    string? OwnerId = null);

public sealed record Item(
    string Id,
    string ItemKey,
    ItemStatus Status,
    DateTimeOffset CreatedAt);
```

Reach for `record struct` when the type is small and used in tight loops; otherwise `record` (a class) is the default.

### 2.3 `sealed` by default for classes

Mark classes `sealed` unless you specifically intend them to be inherited from. Open inheritance is a real commitment (Liskov, fragile-base-class, performance); most types don't need it.

```csharp
public sealed class ItemService { ... }
public abstract class RequestHandlerBase { ... }   // explicitly intended for inheritance
```

### 2.4 Init-only properties and `required`

For DTOs and configuration types, prefer `init` setters and `required` for mandatory members:

```csharp
public sealed class ItemConfig
{
    public required string Name { get; init; }
    public int MaxAgeSeconds { get; init; } = 3600;
}

var cfg = new ItemConfig { Name = "primary" };
```

Object initializers + `required` give you the readability of named arguments and the safety of compiler-enforced required fields, without the ceremony of a constructor.

### 2.5 Strong types over primitives

Like Rust's newtype pattern, wrap domain identifiers so the compiler catches mix-ups:

```csharp
public readonly record struct ItemId(string Value)
{
    public override string ToString() => Value;
}

public readonly record struct UserId(string Value)
{
    public override string ToString() => Value;
}
```

`fn AssignAsync(ItemId item, UserId owner)` cannot be called with the arguments swapped. The cost is a few lines of boilerplate per identifier; the benefit is type-checked correctness for the rest of the codebase.

### 2.6 Pattern matching

Use pattern matching where it improves clarity:

```csharp
// Good
return status switch
{
    ItemStatus.Draft => "Drafting",
    ItemStatus.PendingReview => "Awaiting review",
    ItemStatus.Approved or ItemStatus.Archived => "Final",
    _ => throw new UnreachableException(),
};

// Avoid the if/else cascade for the same shape
```

`is` patterns for null/type narrowing, list patterns for sequences, property patterns for record/object inspection.

---

## 3. Project shape

The project's shape determines its layout. Three shapes are common; the principle is the same in each — **separate the layers, talk only downward**.

### 3.1 Service shape (HTTP API + persistence)

Stack: **ASP.NET Core minimal APIs** (or controllers if the team prefers — both are first-class), **EF Core** or **Dapper** for persistence, **Serilog** or `Microsoft.Extensions.Logging` for logs.

```
src/MyProject/
  Program.cs              ← Entry. App construction, middleware, route mapping.
  Endpoints/              ← Minimal-API endpoint handlers (or Controllers/).
    ItemEndpoints.cs
    UserEndpoints.cs
  Services/               ← Business logic. Stateless. Calls repos.
    ItemService.cs
    UserService.cs
  Repositories/           ← Persistence. Called by services.
    ItemRepository.cs
    DbContext.cs          ← EF Core context; or pool factory for Dapper.
  Models/                 ← API request/response DTOs (records).
    ItemModels.cs
  Domain/                 ← Domain types, value objects, enums.
    ItemId.cs
    ItemStatus.cs
  Errors/                 ← Custom exception types.
    ServiceExceptions.cs
  appsettings.json
  appsettings.Development.json
src/MyProject.Tests/
Migrations/               ← EF Core migration files (if EF Core is used).
```

Project structure on disk:

```
MyProject.sln
src/
  MyProject/
    MyProject.csproj
  MyProject.Tests/
    MyProject.Tests.csproj
```

`Program.cs` uses top-level statements:

```csharp
var builder = WebApplication.CreateBuilder(args);

builder.Services.AddDbContext<AppDbContext>(opts =>
    opts.UseNpgsql(builder.Configuration.GetConnectionString("Default")));
builder.Services.AddScoped<IItemService, ItemService>();

var app = builder.Build();

app.MapItemEndpoints();   // extension method in Endpoints/

await app.RunAsync();
```

All wiring lives near `Program.cs`; endpoint definitions live in `Endpoints/` as extension methods on `WebApplication` or `IEndpointRouteBuilder`.

### 3.2 CLI shape (command-line tool)

Stack: **System.CommandLine** for argument parsing.

```
src/MyProject/
  Program.cs              ← Entry. Builds RootCommand, parses, dispatches.
  Commands/               ← One class per subcommand.
    InitCommand.cs
    RunCommand.cs
    StatusCommand.cs
  Services/               ← Business logic. Called by commands.
  IO/                     ← stdout/stderr formatting helpers.
  Errors/
```

`Program.cs`:

```csharp
var rootCommand = new RootCommand("My project CLI")
{
    new InitCommand(),
    new RunCommand(),
    new StatusCommand(),
};
return await rootCommand.InvokeAsync(args);
```

`Program.cs` stays thin — it builds the command tree and dispatches. Real work lives in `Commands/`.

### 3.3 Library shape (NuGet-distributable assembly)

```
src/MyProject/
  MyProject.csproj        ← <IsPackable>true</IsPackable>; package metadata.
  PublicTypes/            ← Or just at the root — public API surface.
    Item.cs
    ItemService.cs
  Internal/               ← Internal types. internal access modifier.
    ...
  AssemblyInfo.cs         ← InternalsVisibleTo for tests.
```

Public API discipline:

- Everything `public` is part of the public API.
- Everything else is `internal` (the default for top-level types).
- Use `[InternalsVisibleTo("MyProject.Tests")]` to give tests access to internals without making them public.
- Add XML doc comments to every public member; enable `<GenerateDocumentationFile>true</GenerateDocumentationFile>` to surface compiler warnings for missing docs.
- For libraries that may need to support .NET versions other than the latest, declare and justify the multi-targeting in the design doc that introduces the library.

The library shape's contract with consumers is the public surface. Removing or renaming a public member is a breaking change in the sense of `../designs/design-000-meta.md` §3.

### 3.4 The shared layered principle

Whatever the shape, layers talk only downward:

- **Endpoints / Commands / Public-API** — entry points. Thin. Validate input, dispatch, format output.
- **Services** — business logic. No knowledge of the entry layer.
- **Repositories / IO** — persistence and external IO. Called by services.

Skipping layers (an endpoint running raw EF Core queries, a command opening a file directly) is the smell. Push the work into the right layer.

### 3.5 Endpoints (service shape)

Endpoint handlers are thin. They:
- Accept bound parameters (route, query, body).
- Delegate to a service.
- Return a typed result (`Results<Ok<T>, NotFound, BadRequest>`).

Endpoints do **not**:
- Contain SQL or EF Core query expressions.
- Contain multi-step business logic.
- Maintain state outside the request scope.

```csharp
public static class ItemEndpoints
{
    public static IEndpointRouteBuilder MapItemEndpoints(this IEndpointRouteBuilder app)
    {
        var group = app.MapGroup("/api/v1/items").WithTags("Items");

        group.MapPost("/", CreateItem);
        group.MapGet("/{id}", GetItem);

        return app;
    }

    private static async Task<Results<Created<ItemResponse>, Conflict<ErrorResponse>>> CreateItem(
        ItemCreateRequest request,
        IItemService service,
        CancellationToken ct)
    {
        try
        {
            var item = await service.CreateItemAsync(request, ct);
            return TypedResults.Created($"/api/v1/items/{item.Id}", ItemResponse.From(item));
        }
        catch (DuplicateItemKeyException ex)
        {
            return TypedResults.Conflict(ErrorResponse.From("ITEM_KEY_CONFLICT", ex.Message));
        }
    }
}
```

If an endpoint method is more than ~30 lines, it's probably doing too much; extract logic into a service.

### 3.6 Services

Services are the business logic. Each service:
- Is registered with DI (`AddScoped`, typically).
- Takes its dependencies via constructor injection.
- Exposes an interface (`IItemService`) when it has a single implementation; skip the interface if it would be ceremony for one consumer (you can extract later).
- Returns typed values.
- Throws typed exceptions on failure.

Services do **not**:
- Reference HTTP types (`HttpContext`, `IActionResult`, `Results.*`).
- Take a `DbContext` and a `HttpContext` in the same constructor (smell).
- Hold mutable static state.

```csharp
public interface IItemService
{
    Task<Item> CreateItemAsync(ItemCreateRequest request, CancellationToken ct);
    Task<Item?> FindItemAsync(ItemId id, CancellationToken ct);
}

public sealed class ItemService(IItemRepository repo, ILogger<ItemService> logger) : IItemService
{
    public async Task<Item> CreateItemAsync(ItemCreateRequest request, CancellationToken ct)
    {
        var id = ItemId.NewId();
        await repo.InsertAsync(id, request, ct);
        logger.LogInformation("Created item {ItemId}", id);
        return await repo.GetAsync(id, ct);
    }

    public Task<Item?> FindItemAsync(ItemId id, CancellationToken ct) =>
        repo.FindAsync(id, ct);
}
```

Note the **primary constructor** (`(IItemRepository repo, ...)`) — C# 12 feature, removes the boilerplate of declaring fields and assigning them in a constructor.

### 3.7 Repositories (service shape)

Repositories own the persistence. With EF Core:

```csharp
public interface IItemRepository
{
    Task InsertAsync(ItemId id, ItemCreateRequest request, CancellationToken ct);
    Task<Item?> FindAsync(ItemId id, CancellationToken ct);
    Task<Item> GetAsync(ItemId id, CancellationToken ct);
}

public sealed class ItemRepository(AppDbContext db) : IItemRepository
{
    public async Task InsertAsync(ItemId id, ItemCreateRequest request, CancellationToken ct)
    {
        db.Items.Add(new ItemEntity { Id = id.Value, Key = request.ItemKey, ... });
        await db.SaveChangesAsync(ct);
    }

    public Task<Item?> FindAsync(ItemId id, CancellationToken ct) =>
        db.Items
            .Where(i => i.Id == id.Value)
            .Select(i => new Item(i.Id, i.Key, ...))
            .SingleOrDefaultAsync(ct);

    public async Task<Item> GetAsync(ItemId id, CancellationToken ct) =>
        await FindAsync(id, ct) ?? throw new ItemNotFoundException(id);
}
```

For thinner SQL access, **Dapper** is fine — the repository pattern is the same; only the implementation changes.

### 3.8 No SQL or EF Core in endpoints

This deserves its own line. Endpoint handlers never call `dbContext.Items.Where(...)` directly. SQL and query expressions belong in repositories. If an endpoint needs data, it calls a service that calls a repository.

There are no exceptions. Inline data access in endpoints is bootstrap-residue (see `DESIGN_PHILOSOPHY.md`); don't ship it.

---

## 4. Error handling

### 4.1 Throw exceptions for exceptional conditions

Custom exception types per domain:

```csharp
public class ServiceException : Exception
{
    protected ServiceException(string message, Exception? inner = null) : base(message, inner) { }
}

public sealed class DuplicateItemKeyException(string itemKey)
    : ServiceException($"Item with key '{itemKey}' already exists")
{
    public string ItemKey { get; } = itemKey;
}

public sealed class ItemNotFoundException(ItemId id)
    : ServiceException($"Item not found: {id}")
{
    public ItemId Id { get; } = id;
}
```

Services throw these. Endpoints catch them and translate to HTTP responses (per `API_CONVENTIONS.md` §6).

### 4.2 Don't catch general `Exception`

```csharp
// Bad — swallows everything
try { DoThing(); } catch (Exception) { }

// Bad — adds nothing
try { DoThing(); } catch (Exception) { throw; }

// Good — catch what you can handle
try
{
    DoThing();
}
catch (HttpRequestException ex) when (ex.StatusCode == HttpStatusCode.NotFound)
{
    return null;
}
```

The `when` clause (exception filter) is the right tool when you only want to catch in specific conditions.

### 4.3 `Result<T>` pattern (optional)

For domains with many predictable failure modes, a `Result<T, TError>` pattern (via OneOf, ErrorOr, or hand-rolled) avoids the cost of exceptions for routine flow:

```csharp
public sealed record CreateItemResult(Item? Item, ServiceError? Error);
```

This is **opt-in** per design. Don't impose it as a global pattern; choose it where exceptions would be the wrong tool (e.g., a public library function whose failures are part of the expected API surface).

### 4.4 Don't use exceptions for routine validity checks

```csharp
// Bad
try
{
    var id = ItemId.Parse(input);
    return id;
}
catch (FormatException)
{
    return DefaultId();
}

// Good
return ItemId.TryParse(input, out var id) ? id : DefaultId();
```

Provide `Try*` methods on parsers and validators; let callers ask the question without paying the exception-cost.

---

## 5. Persistence

### 5.1 EF Core (rich domain) or Dapper (thin SQL)

Pick one per project. Don't mix unless there's a strong reason.

- **EF Core** when the domain has identity, relationships, and you want change-tracking.
- **Dapper** when the queries are SQL-shaped and the mapping to types is straightforward.

Both are wrapped behind repository interfaces (per §3.7), so swapping isn't a code-rewrite at every call site.

### 5.2 `DbContext` lifetime

`DbContext` is **scoped per request** (the ASP.NET Core default via `AddDbContext`). Don't capture it in a singleton. Don't hold it across multiple requests.

For background work and CLIs, create a scope explicitly:

```csharp
using var scope = serviceProvider.CreateScope();
var db = scope.ServiceProvider.GetRequiredService<AppDbContext>();
// ... use db ...
```

### 5.3 Transactions

For multi-step operations that must be atomic:

```csharp
await using var transaction = await db.Database.BeginTransactionAsync(ct);
try
{
    db.Items.Add(item);
    await db.SaveChangesAsync(ct);

    db.AuditLog.Add(new AuditEntry(item.Id));
    await db.SaveChangesAsync(ct);

    await transaction.CommitAsync(ct);
}
catch
{
    await transaction.RollbackAsync(ct);
    throw;
}
```

EF Core wraps `SaveChangesAsync` in an implicit transaction; you only need explicit transactions when multiple `SaveChangesAsync` calls must be atomic together.

### 5.4 Migrations

EF Core migrations: `dotnet ef migrations add <Name>`. Generated files live in `Migrations/`. Apply at startup or via a separate CLI command — pick one per project.

For Dapper or raw SQL, use a migration tool like FluentMigrator or DbUp. File naming: `<timestamp>_<description>.sql`.

---

## 6. Async

The default is **async**. Modern ASP.NET Core, EF Core, and HttpClient are all async-first.

### 6.1 Async all the way

If a method does I/O, it's `async Task` (or `async Task<T>`). Calling `.Result` or `.Wait()` is a deadlock waiting to happen — never do it in application code.

### 6.2 `CancellationToken` everywhere

Every async method takes a `CancellationToken` (named `ct` or `cancellationToken`) as the last parameter and propagates it to async calls inside:

```csharp
public async Task<Item> GetAsync(ItemId id, CancellationToken ct)
{
    return await db.Items
        .Where(i => i.Id == id.Value)
        .SingleOrDefaultAsync(ct)
        ?? throw new ItemNotFoundException(id);
}
```

ASP.NET Core endpoints can take `CancellationToken` as a parameter; the framework injects the request's cancellation token.

### 6.3 `ConfigureAwait(false)` in libraries

In **library** code (anything packaged for reuse), append `.ConfigureAwait(false)` to every awaited Task to avoid capturing the synchronization context:

```csharp
var data = await client.GetAsync(url).ConfigureAwait(false);
```

In **application** code (services, web apps, CLI tools — anything that's not a library), `ConfigureAwait(false)` is unnecessary noise. ASP.NET Core has no synchronization context.

### 6.4 No `async void`

`async void` swallows exceptions and breaks awaiters. The only legitimate use is event handlers (`button.Click += async (s, e) => ...`); everywhere else, use `async Task`.

### 6.5 `ValueTask<T>` for hot paths only

`Task<T>` allocates; `ValueTask<T>` doesn't (in the synchronous-completion case). But `ValueTask<T>` has subtle rules (await once, don't store). Use it only on measured hot paths, and document the constraint.

---

## 7. Testing

### 7.1 Layout

```
src/MyProject.Tests/
  MyProject.Tests.csproj
  Services/
    ItemServiceTests.cs
  Endpoints/
    ItemEndpointsTests.cs
  Integration/
    ApiTests.cs
  TestFixtures/
    DatabaseFixture.cs
```

One test class per class under test, named `<TypeUnderTest>Tests.cs`. Use **xUnit** by default (it's the .NET community standard); NUnit and MSTest are acceptable if there's a specific reason.

### 7.2 Test naming

Test names describe the behavior. Two acceptable styles:

```csharp
// Style A: MethodName_StateUnderTest_ExpectedBehavior
public void AcquireLock_WhenUnheld_ReturnsTrue() { }
public void AcquireLock_WhenAlreadyHeld_ReturnsFalse() { }
public void AcquireLock_WhenExistingLockIsStale_ReturnsTrue() { }

// Style B: Should_X_When_Y
public void Should_AcquireLock_When_Unheld() { }
public void Should_FailToAcquireLock_When_AlreadyHeld() { }
```

Pick one style per project and stay consistent.

Avoid:
```csharp
public void Test1() { }              // meaningless
public void TestAcquireLock() { }    // too vague
```

### 7.3 Integration tests

Use `WebApplicationFactory<Program>` for end-to-end testing of an ASP.NET Core service:

```csharp
public sealed class ApiTests : IClassFixture<WebApplicationFactory<Program>>
{
    private readonly HttpClient _client;

    public ApiTests(WebApplicationFactory<Program> factory)
    {
        _client = factory.CreateClient();
    }

    [Fact]
    public async Task PostItems_ValidPayload_Returns201()
    {
        var response = await _client.PostAsJsonAsync("/api/v1/items", new { itemKey = "abc" });
        Assert.Equal(HttpStatusCode.Created, response.StatusCode);
    }
}
```

For database-touching tests, use Testcontainers to spin up a real Postgres or SQL Server in a container per test class. Avoid SQLite-as-Postgres-substitute — it lies about behavior in ways that bite later.

### 7.4 Error path coverage

Every happy path needs at least two error path tests covering:
- Bad input (validation failure).
- Resource conflict (duplicate, lock held, invalid transition).

This is a hard rule. Error-path gaps are defects.

---

## 8. Documentation

### 8.1 XML doc comments

Public members of library projects get XML doc comments:

```csharp
/// <summary>
/// Attempts to acquire a creation lock for the given item.
/// </summary>
/// <param name="itemKey">The key of the item to lock.</param>
/// <param name="lockedBy">Identifier of the holder.</param>
/// <param name="ttl">How long the lock should remain valid.</param>
/// <param name="ct">Cancellation token.</param>
/// <returns>
/// <see langword="true"/> if the lock was acquired,
/// <see langword="false"/> if it is held by another holder.
/// Stale locks are automatically force-released and acquired.
/// </returns>
/// <exception cref="ServiceException">If the lock store is unreachable.</exception>
public Task<bool> AcquireLockAsync(
    ItemId itemKey,
    string lockedBy,
    TimeSpan ttl,
    CancellationToken ct) { ... }
```

Enable `<GenerateDocumentationFile>true</GenerateDocumentationFile>` and treat doc warnings as errors in library projects.

For service and CLI projects, docs are encouraged on public service interfaces but not required on every internal type.

### 8.2 Inline comments

Comments explain **why**, not **what**:

```csharp
// Bad — restates the code
i++;  // increment i

// Good — explains the why
// Off-by-one: indices in this dataset are 1-based per the upstream spec.
i++;
```

If the code itself isn't self-explanatory, often the right fix is renaming a variable or extracting a method, not adding a comment.

---

## 9. Project files (`.csproj`)

### 9.1 Standard SDK project

Service or CLI:

```xml
<Project Sdk="Microsoft.NET.Sdk.Web">  <!-- or Microsoft.NET.Sdk for non-web -->

  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>
    <TreatWarningsAsErrors>true</TreatWarningsAsErrors>
    <WarningsAsErrors />
    <NoWarn>$(NoWarn)</NoWarn>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="..." Version="..." />
  </ItemGroup>

</Project>
```

Library:

```xml
<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>
    <TreatWarningsAsErrors>true</TreatWarningsAsErrors>
    <GenerateDocumentationFile>true</GenerateDocumentationFile>
    <IsPackable>true</IsPackable>
    <PackageId>MyProject</PackageId>
    <Authors>...</Authors>
    <Description>Short one-liner.</Description>
  </PropertyGroup>

</Project>
```

### 9.2 `Directory.Packages.props`

For solutions with multiple projects, use **Central Package Management** to declare versions in one place:

```xml
<!-- Directory.Packages.props at solution root -->
<Project>
  <PropertyGroup>
    <ManagePackageVersionsCentrally>true</ManagePackageVersionsCentrally>
  </PropertyGroup>
  <ItemGroup>
    <PackageVersion Include="Microsoft.AspNetCore.OpenApi" Version="8.0.0" />
    <PackageVersion Include="Dapper" Version="2.1.35" />
    <PackageVersion Include="xunit" Version="2.6.6" />
  </ItemGroup>
</Project>
```

Project files then reference packages without versions:

```xml
<PackageReference Include="Dapper" />
```

This keeps versions consistent across projects in the solution.

### 9.3 `EditorConfig`

Use `.editorconfig` at the solution root to enforce style. The defaults from `dotnet new editorconfig` are a good starting point; tighten as needed.

---

## 10. Why conventions matter even in a single project

It's tempting to think conventions only matter when multiple people or systems collaborate. They matter inside a single project too:

- **AI assistants generate code that matches the conventions of the code they see.** Drifty conventions produce drifty generated code. Tight conventions produce tight generated code.
- **`dotnet format` and analyzers are part of the conventions.** Run them in CI; treat warnings as errors. Catching style drift early is cheaper than a rewrite later.
- **Future-you is a different collaborator.** Code you'll be reading in six months was written by someone with different context. Conventions are the shared assumptions that let future-you understand what you wrote.

Reference these conventions in design docs rather than restating them. When they need to change, update the doc and any in-flight designs that depend on them.

---

## 11. Cross-references

- For naming of files and folders (including `.cs` files and migration files): `NAMING_CONVENTIONS.md`.
- For HTTP API and JSON wire format conventions (including `JsonNamingPolicy.CamelCase` and `[JsonPropertyName]` for camelCase wire format): `API_CONVENTIONS.md`.
- For other languages: `CODE_CONVENTIONS-<language>.md`.
- For the design values that motivate code-level discipline: `../DESIGN_PHILOSOPHY.md` (especially "Conventions are constitutional" and "Bootstrap-residue is a smell").
- For the structural foundation that governs design docs and contracts: `../designs/design-000-meta.md`.
- For Microsoft's official C# coding conventions: <https://learn.microsoft.com/dotnet/csharp/fundamentals/coding-style/coding-conventions>.
- For the .NET runtime team's coding style: <https://github.com/dotnet/runtime/blob/main/docs/coding-guidelines/coding-style.md>.
