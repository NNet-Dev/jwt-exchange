
# Code Conventions — Node.js / TypeScript

**Status:** Living document.
**Audience:** Anyone writing Node.js or TypeScript code in this project — humans and AI assistants generating code.
**Scope:** TypeScript style, project layout, type system usage, error handling, persistence, async patterns, testing.
**Companion docs:** Other languages have their own `CODE_CONVENTIONS-<language>.md`. For naming of files and folders, see `NAMING_CONVENTIONS.md`. For HTTP API and JSON wire format conventions, see `API_CONVENTIONS.md`.

When this document and the official TypeScript ESLint / Prettier rules conflict, this document wins (this is rare). When this document is silent, follow Prettier defaults, `typescript-eslint/recommended`, and the TypeScript compiler with `strict: true`.

---

## 1. Style basics

### 1.1 Naming

- **Files and modules:** `kebab-case.ts` (`item-service.ts`, `runtime-adapter.ts`).
- **Classes:** `PascalCase` (`ServiceEngine`, `Request`).
- **Interfaces and type aliases:** `PascalCase` (`ItemCreate`, `PaginatedResult<T>`).
- **Functions and methods:** `camelCase` (`acquireLock`, `provision`).
- **Variables and parameters:** `camelCase` (`itemKey`, `requestId`).
- **Constants:** `SCREAMING_SNAKE_CASE` (`DEFAULT_TTL_SECONDS`, `MAX_BUDGET_USD`).
- **Private/internal:** prefix with `#` for class-private fields; `_` prefix for module-internal exports that shouldn't leak (`_resolveId`, `_formatResponse`).
- **Enum members:** `PascalCase` (`ItemStatus.Draft`, `ItemStatus.PendingReview`).

### 1.2 Toolchain

- **Node.js:** 20 LTS minimum. Don't target EOL versions. Declare the engine constraint in `package.json`:

```json
{
  "engines": {
    "node": ">=20.0.0"
  }
}
```

- **TypeScript:** 5.x. `strict: true` in `tsconfig.json` is non-negotiable.
- **Module system:** ESM by default. `"type": "module"` in `package.json`. Use `.js` extensions in imports per ESM spec. No CommonJS unless interop with a dependency that doesn't support ESM requires it.

```json
{
  "type": "module"
}
```

- **Prettier:** required for formatting. `.prettierrc` only when defaults need overriding (rare). The defaults are the conventions.
- **ESLint:** required with `typescript-eslint`. Run with `-c` config that treats warnings as errors in CI.

### 1.3 Indentation and line length

- 2 spaces (Prettier default; never tabs).
- 100-character line limit (Prettier default).
- Don't fight Prettier. If a formatted line reads badly, the code structure usually needs changing, not the formatter.

### 1.4 Imports

Order: standard library → external → internal/relative. Each group separated by a blank line. Within a group, alphabetize.

```typescript
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';

import { Router } from 'express';
import { z } from 'zod';

import { ItemService } from '../services/item-service.js';
import { ServiceError } from '../error.js';
import type { ItemCreate } from '../domain/item.js';
```

- Use `.js` extension in all relative imports, even though source files are `.ts`. This is required by ESM and prevents runtime import failures.
- Use `import type` for type-only imports — it's erased at compile time and avoids circular dependency issues.
- Barrel exports (`index.ts` that re-exports everything) are discouraged. Use them sparingly, only for library public API surfaces. For internal modules, import directly from the source file.

```typescript
// Good — direct import
import { ItemService } from '../services/item-service.js';

// Bad — barrel indirection
import { ItemService } from '../services/index.js';
```

### 1.5 String formatting

Template literals everywhere. Don't use `+` concatenation or `String.prototype.concat()` for interpolation.

```typescript
// Good
const msg = `Acquired lock for ${itemKey}`;
logger.info(`Acquired lock for ${itemKey}`);

// Bad — manual concatenation
const msg = 'Acquired lock for ' + itemKey;
```

For SQL, **never** use template literals to interpolate values into queries. Always use parameterized queries:

```typescript
// Good — parameterized
const row = await db.query<ItemRow>(
  'SELECT * FROM items WHERE id = $1',
  [itemId]
);

// Bad — SQL injection vector
const row = await db.query<ItemRow>(
  `SELECT * FROM items WHERE id = '${itemId}'`
);
```

---

## 2. Type system

TypeScript's type system is a tool. Use it.

### 2.1 Strict mode always

`strict: true` in `tsconfig.json` is the floor, not the ceiling. Enable these explicitly (they're included in `strict`, but name them so the intent is visible):

```json
{
  "compilerOptions": {
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": true
  }
}
```

- `noUncheckedIndexedAccess`: accessing `obj[key]` returns `T | undefined`, forcing you to handle the missing-key case. This catches a huge class of runtime errors.
- `exactOptionalPropertyTypes`: `foo({ bar: undefined })` is a type error when `bar` is optional. Prevents confusing `undefined` with "not provided."

### 2.2 No `any`

`any` is a type-system escape hatch that disables all safety. Don't use it.

```typescript
// Bad
function handle(data: any) {
  return data.items[0].name;
}

// Good — use unknown and narrow
function handle(data: unknown): string | undefined {
  if (isItemPayload(data)) {
    return data.items[0]?.name;
  }
  return undefined;
}

function isItemPayload(x: unknown): x is ItemPayload {
  return (
    typeof x === 'object' &&
    x !== null &&
    'items' in x &&
    Array.isArray((x as ItemPayload).items)
  );
}
```

The escape hatch is `@ts-expect-error`, but it requires a comment explaining why:

```typescript
// @ts-expect-error — upstream library returns a nested array we need to flatten;
// the type definitions are stale (issue #42 in their repo)
const flat = (legacyResponse as unknown as string[][]).flat();
```

### 2.3 Interfaces vs type aliases

- **Use `interface`** for shapes that may be extended, implemented, or merged (open declaration merging):

```typescript
export interface ItemCreate {
  itemKey: string;
  payload: Record<string, unknown>;
}

// Extension is natural
export interface ItemCreateWithOwner extends ItemCreate {
  ownerId: string;
}
```

- **Use `type`** for unions, intersections, mapped types, tuples, and anything that can't be expressed as an interface:

```typescript
export type ItemStatus = 'draft' | 'pending_review' | 'approved' | 'archived';

export type Result<T, E = Error> =
  | { ok: true; value: T }
  | { ok: false; error: E };

export type PaginatedResult<T> = {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
};
```

When in doubt, prefer `interface` for object shapes and `type` for everything else. Consistency within a codebase matters more than the theoretical distinction.

### 2.4 Branded types for domain identifiers

Don't pass `string` everywhere. Brand domain identifiers so the compiler catches mix-ups between `UserId` and `ItemId` at compile time — TypeScript's answer to Rust's newtype pattern:

```typescript
// Branded type pattern
declare const __brand: unique symbol;
type Brand<B> = { readonly [__brand]: B };
type Branded<T, B> = T & Brand<B>;

export type ItemId = Branded<string, 'ItemId'>;
export type UserId = Branded<string, 'UserId'>;

// Factory functions to create branded values
export function createItemId(value: string): ItemId {
  return value as ItemId;
}

export function createUserId(value: string): UserId {
  return value as UserId;
}
```

Now `function assign(item: ItemId, owner: UserId)` cannot be called with the arguments swapped — the compiler rejects it:

```typescript
// Error: Argument of type 'UserId' is not assignable to parameter of type 'ItemId'
assign(createUserId('usr_xyz'), createItemId('abc-123'));
```

### 2.5 Discriminated unions for state/variant types

If a value can be in one of several distinct states, a discriminated union is almost always better than a struct with optional fields:

```typescript
// Good — invalid states unrepresentable
type ItemStatus =
  | { kind: 'draft' }
  | { kind: 'pending_review'; submittedAt: Date }
  | { kind: 'approved'; approvedBy: UserId; approvedAt: Date }
  | { kind: 'archived'; reason: string };

function formatStatus(status: ItemStatus): string {
  switch (status.kind) {
    case 'draft':
      return 'Draft';
    case 'pending_review':
      return `Submitted ${status.submittedAt.toISOString()}`;
    case 'approved':
      return `Approved by ${status.approvedBy}`;
    case 'archived':
      return `Archived: ${status.reason}`;
  }
}

// Bad — every consumer has to check which fields are populated
interface ItemStatusLegacy {
  state: string;
  submittedAt?: Date;
  approvedBy?: UserId;
  approvedAt?: Date;
  archiveReason?: string;
}
```

The `kind` property is the discriminant. TypeScript narrows the union automatically inside `switch` or `if` blocks, making it impossible to access a field that doesn't exist on the current variant.

### 2.6 Const assertions for enums and config objects

Use `as const` for configuration objects and enum-like values to get exact literal types:

```typescript
const ITEM_STATUS = {
  DRAFT: 'draft',
  PENDING_REVIEW: 'pending_review',
  APPROVED: 'approved',
  ARCHIVED: 'archived',
} as const;

// Type is { readonly DRAFT: "draft"; readonly PENDING_REVIEW: "pending_review"; ... }
type ItemStatusValue = (typeof ITEM_STATUS)[keyof typeof ITEM_STATUS];
// Resolves to: "draft" | "pending_review" | "approved" | "archived"
```

For array constants:

```typescript
const VALID_PRIORITIES = ['low', 'medium', 'high', 'critical'] as const;
// Type: readonly ["low", "medium", "high", "critical"]
```

---

## 3. Project shape

The project's shape determines its layout. Three shapes are common in this ecosystem; the principle is the same in each — **separate the layers, talk only downward, never upward**.

### 3.1 Service shape (HTTP API + persistence)

Stack: **Express / Fastify / Hono** for HTTP, **Prisma** or **Drizzle** for database access.

```
src/
  main.ts                ← Binary entry. Constructs app, starts the server.
  app.ts                 ← App construction; route mounting; middleware pipeline.
  routes/                ← HTTP endpoints. Thin. Calls services.
    items.ts
    users.ts
    index.ts
  services/              ← Business logic. Stateless. Calls data-access. No HTTP knowledge.
    item-service.ts
  data-access/           ← Database queries, migrations. Called by services.
    item-repository.ts
    db.ts                ← Pool/connection management.
  domain/                ← Domain types (branded types, discriminated unions, value objects).
    item.ts
    index.ts
  error.ts               ← Application error classes.
  config.ts              ← Configuration loaded from env.
```

### 3.2 CLI shape (command-line tool)

Stack: **cac** or **commander** for argument parsing.

```
src/
  main.ts                ← Binary entry. Calls cli.run().
  cli.ts                 ← CLI entry; argument parsing; dispatch to commands.
  commands/              ← One module per subcommand.
    init.ts
    run.ts
    status.ts
  services/              ← Business logic. Called by commands.
    item-service.ts
  io.ts                  ← stdout/stderr formatting helpers.
  error.ts               ← Application error classes.
```

`main.ts` stays minimal — parse args, dispatch, handle errors:

```typescript
import { run } from './cli.js';

run().catch((err) => {
  console.error(err.message);
  process.exit(1);
});
```

### 3.3 Library shape (importable package consumed by other code)

```
src/
  index.ts               ← Public API surface. Explicit re-exports.
  item-service.ts        ← Public service classes/functions.
  error.ts               ← Public error classes.
  domain/                ← Public domain types.
  internal/              ← Private modules. NOT re-exported from index.ts.
    validation.ts
    helpers.ts
```

Public API discipline:

- Everything importable from the package's entry point (`index.ts`) is part of the public API.
- Anything that should not leak lives in `internal/` or is not re-exported.
- `index.ts` explicitly imports and re-exports the public surface. No `export * from './internal/*.js'`.
- Include `"types"` in `package.json` pointing to the compiled `.d.ts` file so consumers' type-checkers work.

```json
{
  "main": "dist/index.js",
  "types": "dist/index.d.ts"
}
```

The library shape's contract with consumers is the import surface. Removing or renaming a public symbol is a breaking change.

### 3.4 The shared layered principle

Whatever the shape, layers talk only downward:

- **Routes / Commands / Public-API** — entry points. Thin. Validate input, dispatch, format output.
- **Services** — business logic. No knowledge of the entry layer.
- **Data access / Storage / IO** — persistence and external IO. Called by services.

Skipping layers (a route running raw SQL, a CLI command opening a database connection directly) is the smell. Push the work into the right layer.

### 3.5 Route handlers (service shape)

Route handlers are thin. They:

- Validate input (via Zod or similar schema library).
- Delegate to a service function.
- Format the response.
- Translate service-layer errors to HTTP responses.

Route handlers do **not**:

- Contain SQL or data-access calls.
- Contain multi-step business logic.
- Call other route handlers directly.
- Access the database directly.

If a route handler is more than ~30 lines, it's probably doing too much; extract logic into a service function.

```typescript
import { Router, Request, Response } from 'express';
import { z } from 'zod';
import { ItemService } from '../services/item-service.js';
import { ServiceError, NotFoundError } from '../error.js';
import { createItemId } from '../domain/item.js';

const router = Router();

const CreateItemSchema = z.object({
  itemKey: z.string().min(1),
  payload: z.record(z.unknown()),
});

router.post('/items', async (req: Request, res: Response) => {
  const parsed = CreateItemSchema.safeParse(req.body);
  if (!parsed.success) {
    return res.status(400).json({ error: 'VALIDATION_ERROR', message: parsed.error.message });
  }

  try {
    const item = await itemService.createItem(parsed.data);
    res.status(201).json(item);
  } catch (err) {
    if (err instanceof ServiceError) {
      return res.status(err.statusCode).json({ error: err.code, message: err.message });
    }
    throw err; // Let the error handler deal with it
  }
});
```

### 3.6 Services

Service modules contain the business logic. Each function or method:

- Takes typed parameters (including the data-access layer for DB access).
- Returns typed values.
- Has no awareness of HTTP, CLI, or any entry-layer concept.
- Raises typed errors for the caller to interpret.

Services do **not**:

- Import from `routes/` or `commands/`.
- Maintain global mutable state (use dependency injection or pass dependencies explicitly).
- Call other services that import them (one-way dependency, no cycles).

```typescript
import { ItemRepository } from '../data-access/item-repository.js';
import { NotFoundError, DuplicateItemKeyError, ServiceError } from '../error.js';
import { createItemId } from '../domain/item.js';
import type { ItemCreate, Item } from '../domain/item.js';

export class ItemService {
  constructor(private readonly repo: ItemRepository) {}

  async createItem(input: ItemCreate): Promise<Item> {
    const id = createItemId(crypto.randomUUID());

    const existing = await this.repo.findByKey(input.itemKey);
    if (existing) {
      throw new DuplicateItemKeyError(input.itemKey);
    }

    return this.repo.insert(id, input);
  }

  async getItem(id: string): Promise<Item> {
    const item = await this.repo.findById(id);
    if (!item) {
      throw new NotFoundError(`item ${id}`);
    }
    return item;
  }
}
```

### 3.7 Data access layer (service shape)

The data-access module owns database connection management and queries. Use **Prisma** (recommended) or **Drizzle** as the ORM/query builder.

With Prisma:

```typescript
import { PrismaClient } from '@prisma/client';

// Create once at startup; pass into services
const prisma = new PrismaClient();

export class ItemRepository {
  constructor(private readonly db: PrismaClient) {}

  async findById(id: string): Promise<Item | null> {
    const row = await this.db.item.findUnique({ where: { id } });
    if (!row) return null;
    return this.mapRow(row);
  }

  async findByKey(itemKey: string): Promise<Item | null> {
    const row = await this.db.item.findUnique({ where: { itemKey } });
    if (!row) return null;
    return this.mapRow(row);
  }

  async insert(id: string, input: ItemCreate): Promise<Item> {
    const row = await this.db.item.create({
      data: { id, itemKey: input.itemKey, payload: input.payload },
    });
    return this.mapRow(row);
  }

  private mapRow(row: PrismaItem): Item {
    return {
      id: row.id,
      itemKey: row.itemKey,
      payload: row.payload as Record<string, unknown>,
      status: row.status,
      createdAt: row.createdAt,
    };
  }
}
```

With Drizzle (parameterized, no raw SQL interpolation):

```typescript
import { eq } from 'drizzle-orm';
import { db, itemsTable } from './db.js';
import type { ItemCreate, Item } from '../domain/item.js';

export class ItemRepository {
  async findById(id: string): Promise<Item | null> {
    const [row] = await db.select().from(itemsTable).where(eq(itemsTable.id, id));
    return row ?? null;
  }

  async insert(id: string, input: ItemCreate): Promise<Item> {
    const [row] = await db
      .insert(itemsTable)
      .values({ id, itemKey: input.itemKey, payload: input.payload })
      .returning();
    return row;
  }
}
```

### 3.8 No SQL in route handlers

This is worth its own line. Route handlers never call the database directly — not through Prisma, not through Drizzle, not through a raw query method. SQL (or the ORM equivalent) belongs in the data-access layer. If a route needs data, it calls a service function that returns the data.

There are no exceptions. If a route appears to "need" inline DB access — most commonly for a quick FK existence check before delegating — that's a smell. The right move is a service function that does the check (`itemService.itemExists(itemId)` returning `boolean`), called from the route. Inline DB access in routes is bootstrap-residue (see `DESIGN_PHILOSOPHY.md`); don't ship it.

---

## 4. Error handling

### 4.1 Custom error classes

Extend `Error` for application-level errors. Set `name` so `instanceof` works for branching:

```typescript
export class ServiceError extends Error {
  public readonly code: string;
  public readonly statusCode: number;

  constructor(code: string, message: string, statusCode: number = 500) {
    super(message);
    this.name = 'ServiceError';
    this.code = code;
    this.statusCode = statusCode;
    // Fix the prototype chain for instanceof
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class NotFoundError extends ServiceError {
  constructor(resource: string) {
    super('NOT_FOUND', `${resource} not found`, 404);
    this.name = 'NotFoundError';
  }
}

export class DuplicateItemKeyError extends ServiceError {
  constructor(itemKey: string) {
    super('ITEM_KEY_CONFLICT', `Item with key '${itemKey}' already exists`, 409);
    this.name = 'DuplicateItemKeyError';
  }
}

export class InvalidStatusTransitionError extends ServiceError {
  constructor(from: string, to: string) {
    super(
      'INVALID_STATUS_TRANSITION',
      `Cannot transition from '${from}' to '${to}'`,
      409,
    );
    this.name = 'InvalidStatusTransitionError';
  }
}
```

Use `instanceof` for branching in routes and error-handling middleware:

```typescript
// In route or middleware
if (err instanceof ServiceError) {
  return res.status(err.statusCode).json({
    error: err.code,
    message: err.message,
  });
}
```

### 4.2 Result pattern for service layer

For services where the caller needs to handle both success and error cases without try/catch, use a Result type (discriminated union):

```typescript
type Result<T, E = ServiceError> =
  | { ok: true; value: T }
  | { ok: false; error: E };

async function tryGetItem(id: string): Promise<Result<Item>> {
  const item = await repo.findById(id);
  if (!item) {
    return { ok: false, error: new NotFoundError(`item ${id}`) };
  }
  return { ok: true, value: item };
}

// Caller
const result = await tryGetItem('abc-123');
if (result.ok) {
  console.log(result.value.itemKey); // TypeScript knows value exists
} else {
  console.error(result.error.message); // TypeScript knows error exists
}
```

This is TypeScript's answer to Rust's `Result<T, E>`. Use it where the caller is expected to handle the error case as a normal control-flow path, not as an exception.

### 4.3 Don't swallow errors

Empty catch blocks are defects. Never catch and do nothing:

```typescript
// Bad — silently swallows
try {
  await doThing();
} catch {
  // nothing
}

// Bad — adds nothing
try {
  await doThing();
} catch (err) {
  throw err;
}

// Good — converts to a typed error
try {
  await doThing();
} catch (err) {
  if (err instanceof Prisma.PrismaClientKnownRequestError) {
    if (err.code === 'P2002') {
      throw new DuplicateItemKeyError(itemKey);
    }
  }
  // Re-throw if we don't recognize it
  throw err;
}

// Good — logs and re-throws
try {
  await doThing();
} catch (err) {
  logger.error({ err }, 'doThing failed');
  throw err;
}
```

### 4.4 Async error handling

Wrap async route handlers so unhandled rejections don't crash the process. Use a wrapper or a library like `express-async-errors`:

```typescript
// Wrapper pattern
function asyncHandler(
  fn: (req: Request, res: Response, next: NextFunction) => Promise<void>,
) {
  return (req: Request, res: Response, next: NextFunction) => {
    Promise.resolve(fn(req, res, next)).catch(next);
  };
}

router.post('/items', asyncHandler(async (req, res) => {
  // ... handler body
}));

// Or use express-async-errors:
import 'express-async-errors';
// Now async handlers automatically forward errors to the error middleware
```

Don't rely on `process.on('unhandledRejection')` as your primary error-handling strategy. Catch errors at the appropriate layer.

---

## 5. Persistence

### 5.1 Use an ORM or query builder

Don't write raw SQL strings and pass them to a connection. Use **Prisma** (recommended) or **Drizzle**. Both provide:

- Type-safe query building.
- Migration management.
- Connection pooling.

Prisma's schema language is the clearest for team readability. Drizzle is lighter and more SQL-faithful. Pick one per project; don't mix.

### 5.2 Parameterized queries always

Whether using Prisma, Drizzle, or raw SQL (in rare cases where the ORM can't express the query), always use parameterized queries. No string interpolation:

```typescript
// Bad — SQL injection vector
const rows = await db.$queryRawUnsafe(
  `SELECT * FROM items WHERE key = '${userInput}'`
);

// Good — Prisma parameterizes automatically
const row = await db.item.findUnique({ where: { itemKey: userInput } });

// Good — Drizzle parameterizes automatically
const [row] = await db
  .select()
  .from(itemsTable)
  .where(eq(itemsTable.itemKey, userInput));

// Good — raw query with parameters (when ORM can't express it)
const rows = await db.$queryRawUnsafe(
  'SELECT * FROM items WHERE key = $1',
  userInput
);
```

### 5.3 Migrations versioned and deterministic

Migrations live in a `prisma/migrations/` or `drizzle/` directory. They are versioned and applied in order:

```bash
# Prisma
npx prisma migrate dev          # develop
npx prisma migrate deploy       # production (no prompts)

# Drizzle
npx drizzle-kit generate
npx drizzle-kit migrate
```

Migration files are never edited after being applied to any environment. If a migration needs to change, create a new one. The migration history is append-only.

### 5.4 Connection pooling at startup

Create the database client once at application startup and pass it into services via dependency injection:

```typescript
// main.ts
import { PrismaClient } from '@prisma/client';
import { ItemRepository } from './data-access/item-repository.js';
import { ItemService } from './services/item-service.js';

const prisma = new PrismaClient();
const itemRepo = new ItemRepository(prisma);
const itemService = new ItemService(itemRepo);

// Pass itemService into route handlers
const app = createApp(itemService);
```

Prisma manages its own connection pool internally (default: connection limit based on the database). For Drizzle with `pg` or `mysql2`, use the driver's built-in pool:

```typescript
import { Pool } from 'pg';
import { drizzle } from 'drizzle-orm/node-postgres';

const pool = new Pool({
  connectionString: process.env.DATABASE_URL,
  max: 20, // tune for your workload
});

export const db = drizzle(pool);
```

---

## 6. Async patterns

The default in Node.js is **async**. Sync is the exception.

### 6.1 async/await always

Use `async/await` for all asynchronous operations in application code. Don't use `.then()/.catch()` chains:

```typescript
// Good
const item = await itemService.getItem(id);
const related = await itemService.getRelated(item.id);

// Bad — .then()/.catch() chains
itemService.getItem(id)
  .then(item => itemService.getRelated(item.id))
  .then(related => console.log(related))
  .catch(err => console.error(err));
```

The exception: in test assertions where you're explicitly testing promise rejection behavior, `.rejects` / `.resolves` is idiomatic:

```typescript
await expect(itemService.getItem('nonexistent')).rejects.toThrow(NotFoundError);
```

### 6.2 Promise.all for parallel independent operations

When operations are independent and can run in parallel, use `Promise.all`:

```typescript
// Good — runs in parallel
const [item, user, permissions] = await Promise.all([
  itemService.getItem(itemId),
  userService.getUser(userId),
  permissionService.check(userId, itemId),
]);

// Bad — sequential when it doesn't need to be
const item = await itemService.getItem(itemId);
const user = await userService.getUser(userId);
const permissions = await permissionService.check(userId, itemId);
```

Use `Promise.allSettled` when you want to collect all results even if some fail:

```typescript
const results = await Promise.allSettled([
  sendEmail(user),
  sendSlack(user),
  sendWebhook(user),
]);

const failures = results.filter((r): r is PromiseRejectedResult => r.status === 'rejected');
if (failures.length > 0) {
  logger.warn({ failures }, 'Some notifications failed');
}
```

### 6.3 AbortController for cancellable operations

Use `AbortController` for operations that should be cancellable (timeouts, request cancellation):

```typescript
const controller = new AbortController();
const timeoutId = setTimeout(() => controller.abort(), 5_000);

try {
  const response = await fetch(url, { signal: controller.signal });
  return response.json();
} finally {
  clearTimeout(timeoutId);
}
```

Propagate `AbortSignal` through your call stack:

```typescript
async function fetchWithRetry(url: string, signal?: AbortSignal): Promise<Response> {
  let lastError: Error | undefined;
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      return await fetch(url, { signal });
    } catch (err) {
      lastError = err as Error;
      if (err.name === 'AbortError') throw err; // Don't retry on abort
      await sleep(1000 * attempt, { signal });  // Backoff, respects abort
    }
  }
  throw lastError!;
}
```

### 6.4 Error boundaries for async

Catch errors at the appropriate layer. Don't let promises go unhandled:

```typescript
// Route level — catch and translate to HTTP
router.get('/items/:id', asyncHandler(async (req, res) => {
  const item = await itemService.getItem(req.params.id);
  res.json(item);
}));

// CLI level — catch and exit with message
async function run() {
  try {
    await executeCommand(args);
  } catch (err) {
    if (err instanceof ServiceError) {
      console.error(`Error: ${err.message}`);
      process.exit(1);
    }
    console.error('Unexpected error:', err);
    process.exit(1);
  }
}
```

---

## 7. Testing

### 7.1 Vitest as the test runner

Use **Vitest** — it's lighter, ESM-native, and faster than Jest for TypeScript projects. It shares a configuration shape with Vite and supports the same plugin ecosystem.

```bash
npm install -D vitest
```

`vitest.config.ts`:

```typescript
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    include: ['src/**/*.test.ts'],
  },
});
```

### 7.2 Test file naming

Test files use `*.test.ts` next to the source file:

```
src/
  services/
    item-service.ts
    item-service.test.ts
  data-access/
    item-repository.ts
    item-repository.test.ts
  routes/
    items.test.ts
```

This keeps tests co-located with the code they test. Alternative: a parallel `tests/` directory with the same structure. Pick one per project and be consistent.

### 7.3 Arrange-Act-Assert

Every test follows the Arrange-Act-Assert structure:

```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import { ItemService } from './item-service.js';
import { MockItemRepository } from './__mocks__/item-repository.js';

describe('ItemService', () => {
  let service: ItemService;
  let repo: MockItemRepository;

  beforeEach(() => {
    repo = new MockItemRepository();
    service = new ItemService(repo);
  });

  it('creates an item when key is unique', async () => {
    // Arrange
    repo.findByKey.mockResolvedValue(null);
    const input = { itemKey: 'new-key', payload: { foo: 'bar' } };

    // Act
    const result = await service.createItem(input);

    // Assert
    expect(result.itemKey).toBe('new-key');
    expect(repo.insert).toHaveBeenCalledWith(
      expect.any(String),
      input,
    );
  });

  it('throws DuplicateItemKeyError when key already exists', async () => {
    // Arrange
    repo.findByKey.mockResolvedValue({
      id: 'existing-id',
      itemKey: 'dup-key',
      payload: {},
      status: 'draft',
      createdAt: new Date(),
    });

    // Act & Assert
    await expect(
      service.createItem({ itemKey: 'dup-key', payload: {} }),
    ).rejects.toThrow(DuplicateItemKeyError);
  });
});
```

### 7.4 Mock external dependencies, not internal implementation details

Mock the data-access layer, HTTP clients, and file-system operations. Don't mock the service's own internal methods — test the real behavior:

```typescript
// Good — mock the repository (external dependency)
const mockRepo = {
  findById: vi.fn(),
  findByKey: vi.fn(),
  insert: vi.fn(),
};

// Bad — mocking the service's own method hides real behavior
vi.spyOn(service, 'getItem').mockResolvedValue(fakeItem);
```

Use `vi.fn()` for spies and `vi.mock()` for module-level mocks:

```typescript
vi.mock('@prisma/client', () => ({
  PrismaClient: vi.fn(() => mockPrisma),
}));
```

### 7.5 Integration tests for route → service → DB paths

Unit tests aren't enough. Test the full path from HTTP request through service to a real (or test-container) database:

```typescript
import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import request from 'supertest';
import { createTestApp } from '../test-utils.js';
import { TestDatabase } from '../test-utils/db.js';

describe('POST /items', () => {
  let app: ReturnType<typeof createTestApp>;
  let testDb: TestDatabase;

  beforeAll(async () => {
    testDb = await TestDatabase.create();
    app = createTestApp(testDb.prisma);
  });

  afterAll(async () => {
    await testDb.destroy();
  });

  it('returns 201 and the created item', async () => {
    const response = await request(app)
      .post('/items')
      .send({ itemKey: 'test-key', payload: { test: true } });

    expect(response.status).toBe(201);
    expect(response.body.itemKey).toBe('test-key');
    expect(response.body.status).toBe('draft');
  });

  it('returns 409 when key already exists', async () => {
    // First request creates the item
    await request(app)
      .post('/items')
      .send({ itemKey: 'conflict-key', payload: {} });

    // Second request with same key should conflict
    const response = await request(app)
      .post('/items')
      .send({ itemKey: 'conflict-key', payload: {} });

    expect(response.status).toBe(409);
    expect(response.body.error).toBe('ITEM_KEY_CONFLICT');
  });
});
```

Use Docker-based test containers (e.g., `testcontainers` package) for real database integration tests, or SQLite in-memory for quick iteration.

---

## 8. API conventions

### 8.1 JSON wire format is camelCase

Node.js/TypeScript uses camelCase natively — the JSON wire format from `API_CONVENTIONS.md` matches the language convention directly. No translation seam is needed (unlike Python's snake_case ↔ camelCase Pydantic alias or Rust's `#[serde(rename_all = "camelCase")]`).

```typescript
// The wire format and the TypeScript type match
interface ItemCreate {
  itemKey: string;       // sent as "itemKey" in JSON — matches wire format
  itemPayload: Record<string, unknown>;
}
```

### 8.2 Reference API_CONVENTIONS.md for the rest

For URL conventions (kebab-case paths, plural resource names), status code mapping, pagination shape, and error envelope format, see `API_CONVENTIONS.md`. This section exists to confirm that TypeScript consumers the wire format as-is — no additional renaming or transformation layer is required.

---

## 9. Documentation

### 9.1 Module-level JSDoc

Every module starts with a JSDoc comment describing its purpose and layer:

```typescript
/**
 * Item Service — Business Logic Layer.
 *
 * Handles item creation, lookup, status transitions, and lock management.
 *
 * Layer: services. Imports from data-access only. No HTTP knowledge.
 */
export class ItemService {
  // ...
}
```

### 9.2 Function/method JSDoc

Public functions and methods get JSDoc comments:

```typescript
/**
 * Attempt to acquire a creation lock.
 *
 * @param itemKey - The item key to lock.
 * @param lockedBy - The identifier of the lock holder.
 * @param ttlSeconds - Time-to-live for the lock in seconds. Defaults to 600.
 * @returns `true` if the lock was acquired, `false` if already held.
 * @throws {ServiceError} If the lock table cannot be reached.
 */
async acquireLock(
  itemKey: string,
  lockedBy: string,
  ttlSeconds: number = 600,
): Promise<boolean> {
  // ...
}
```

One-line summary. Blank line. Detail. `@returns` to document return semantics. `@throws` for typed exceptions. Don't document parameters individually unless their meaning isn't obvious from name and type.

### 9.3 Inline comments

Comments explain **why**, not **what**:

```typescript
// Bad — restates the code
index += 1; // increment index

// Good — explains the why
// Off-by-one: indices in this dataset are 1-based per the upstream spec.
index += 1;
```

If the code itself isn't self-explanatory, often the right fix is renaming a variable or extracting a function, not adding a comment.

---

## 10. Configuration and environment

### 10.1 Environment variables

Read environment variables through a validated config module, not scattered `process.env` calls:

```typescript
import { z } from 'zod';

const EnvSchema = z.object({
  NODE_ENV: z.enum(['development', 'production', 'test']).default('development'),
  PORT: z.coerce.number().default(3000),
  DATABASE_URL: z.string().url(),
  LOG_LEVEL: z.enum(['debug', 'info', 'warn', 'error']).default('info'),
});

export const config = EnvSchema.parse(process.env);
```

This gives you:
- Type-safe config (all values have the correct type).
- Validation at startup (fail fast if a required variable is missing).
- Defaults for optional variables.

### 10.2 Don't hardcode secrets

Never commit API keys, database passwords, or tokens to source control. Use `.env` files for local development (listed in `.gitignore`) and environment-specific secret managers for production.

---

## 11. Package.json and project configuration

### 11.1 package.json

```json
{
  "name": "myproject",
  "version": "0.1.0",
  "type": "module",
  "private": true,
  "engines": {
    "node": ">=20.0.0"
  },
  "scripts": {
    "build": "tsc",
    "dev": "tsx watch src/main.ts",
    "start": "node dist/main.js",
    "test": "vitest run",
    "test:watch": "vitest",
    "lint": "eslint src/",
    "format": "prettier --write src/",
    "format:check": "prettier --check src/"
  },
  "dependencies": {
    "express": "^4.18.0",
    "@prisma/client": "^5.0.0",
    "zod": "^3.22.0"
  },
  "devDependencies": {
    "@types/express": "^4.17.0",
    "@types/node": "^20.0.0",
    "eslint": "^8.0.0",
    "prettier": "^3.0.0",
    "prisma": "^5.0.0",
    "tsx": "^4.0.0",
    "typescript": "^5.0.0",
    "vitest": "^1.0.0",
    "supertest": "^6.0.0"
  }
}
```

Pin to major versions in `package.json` (`^4.18.0` not `4.18.2`) unless you have a specific reason. Use a lockfile (`package-lock.json` or `pnpm-lock.yaml`) for deterministic installs.

### 11.2 tsconfig.json

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "lib": ["ES2022"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "declaration": true,
    "sourceMap": true
  },
  "include": ["src/**/*.ts"],
  "exclude": ["node_modules", "dist", "src/**/*.test.ts"]
}
```

- `module: "NodeNext"` + `moduleResolution: "NodeNext"` enforces ESM with `.js` extensions in imports.
- `forceConsistentCasingInFileNames` catches cross-platform path casing bugs.
- `skipLibCheck: true` speeds up compilation; the types in `node_modules` are not your responsibility.

### 11.3 tsx for development

Use `tsx` (the modern successor to `ts-node`) for running TypeScript directly during development. It's fast, ESM-native, and doesn't require a build step:

```bash
npm install -D tsx
# In package.json scripts:
# "dev": "tsx watch src/main.ts"
```

---

## 12. Why conventions matter even in a single project

It's tempting to think that conventions only matter when multiple people or multiple systems collaborate. They matter inside a single project too, for two reasons:

- **AI assistants (Claude, Copilot, etc.) generate code that follows the conventions of the code they see.** Drifty conventions produce drifty generated code. Tight conventions produce tight generated code.
- **Future-you is a different collaborator.** The code you'll be reading in six months was written by someone with different context. Conventions are the shared assumptions that let future-you understand what you wrote.

The conventions documents (this one plus `API_CONVENTIONS.md` and `NAMING_CONVENTIONS.md`) are the contract between past, present, and future contributors — human and AI. Reference them in design docs rather than restating them. When they need to change, update the doc and any in-flight designs that depend on them.

---

## 13. Cross-references

- For naming of files and folders (including `.ts` modules, config files, and test files): `NAMING_CONVENTIONS.md`.
- For HTTP API and JSON wire format conventions (URLs, status codes, error envelopes, pagination): `API_CONVENTIONS.md`.
- For other languages: `CODE_CONVENTIONS-<language>.md`.
- For the design values that motivate code-level discipline: `../DESIGN_PHILOSOPHY.md` (especially "Conventions are constitutional" and "Bootstrap-residue is a smell").
- For the structural foundation that governs design docs and contracts: `../designs/design-000-meta.md`.
- For TypeScript's official handbook: <https://www.typescriptlang.org/docs/>.
- For the TypeScript ESLint ruleset: <https://typescript-eslint.io/>.
