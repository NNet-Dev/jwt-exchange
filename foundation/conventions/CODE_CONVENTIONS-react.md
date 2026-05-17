
# Code Conventions — React / TypeScript

**Status:** Living document.
**Audience:** Anyone writing React or TypeScript code in this project — humans and AI assistants generating code.
**Scope:** React component patterns, hooks, state management, project layout, TypeScript usage, testing, performance. Defers to `CODE_CONVENTIONS-nodejs.md` for base TypeScript style, ESM, and toolchain rules not covered here.
**Companion docs:** For base TypeScript/Node.js conventions (imports, toolchain, strict mode, error handling, async patterns), see `CODE_CONVENTIONS-nodejs.md`. For naming of files and folders, see `NAMING_CONVENTIONS.md`. For HTTP API and JSON wire format conventions, see `API_CONVENTIONS.md`.

When this document and `CODE_CONVENTIONS-nodejs.md` conflict, this document wins for React-specific concerns (this is rare). When this document is silent, follow `CODE_CONVENTIONS-nodejs.md`, then Prettier defaults, `typescript-eslint/recommended`, and the TypeScript compiler with `strict: true`.

---

## 1. React style basics

### 1.1 Naming

- **Component files:** `PascalCase.tsx` (`UserProfile.tsx`, `NavItem.tsx`).
- **Component functions:** `PascalCase` (`function UserProfile() { ... }`).
- **Custom hooks:** `camelCase` prefixed with `use` (`useAuth`, `useDebounceValue`).
- **Hook files:** `camelCase.ts` (`useAuth.ts`, `useDebounceValue.ts`).
- **Utility/helpers:** `camelCase.ts` (`format-date.ts`, `validate-form.ts`).
- **Context modules:** `camelCase.ts` or `PascalCase.ts` (`auth-context.ts`, `AuthContext.ts`). Pick one per project and be consistent.
- **Type-only files:** `camelCase.types.ts` (`item.types.ts`, `user.types.ts`).

### 1.2 Toolchain

- **React:** 19.x preferred. 18.x minimum.
- **TypeScript:** 5.x with `strict: true`. All rules from `CODE_CONVENTIONS-nodejs.md` §2 apply.
- **JSX vs TSX:** use `.tsx` for files containing JSX, `.ts` for files without JSX. Never use `.jsx`/`.js` in a TypeScript project.
- **Bundler:** Vite is the default for new projects. Next.js is the default for SSR/SSG. Do not mix bundlers in a single project.
- **Prettier + ESLint:** required. Use `eslint-plugin-react-hooks` and `eslint-plugin-react` (with `jsx-runtime` setting) for linting.
- **Node.js:** 20 LTS minimum. See `CODE_CONVENTIONS-nodejs.md` §1.2.

### 1.3 Indentation and line length

- 2 spaces (Prettier default).
- 100-character line limit (Prettier default).
- JSX indentation: align closing tags with the opening tag's statement, not the props:

```tsx
// Good
return (
  <div className="card">
    <h2>{title}</h2>
    <p>{description}</p>
  </div>
);

// Bad — closing tag misaligned
return (
    <div className="card">
      <h2>{title}</h2>
      <p>{description}</p>
    </div>
);
```

### 1.4 Imports

Follow `CODE_CONVENTIONS-nodejs.md` §1.4 for ordering and grouping. React-specific additions:

- React imports come first in the standard-library group:

```typescript
import { useState, useMemo } from 'react';
import type { ReactNode } from 'react';

import { Router } from 'express';
```

- Import React hooks from `'react'`, not from third-party re-export barrels.
- Import component types from the component's own module or a `.types.ts` sibling — not from `index.ts` re-exports.
- Avoid `import * as React from 'react'` — use named imports. The JSX transform (since React 17) does not require `React` to be in scope.

### 1.5 String formatting and JSX

- Template literals for all string interpolation outside JSX (per `CODE_CONVENTIONS-nodejs.md` §1.5).
- Inside JSX, use `{}` for interpolation:

```tsx
// Good
<p>Welcome, {user.name}</p>
<div className={`avatar ${isOnline ? 'online' : 'offline'}`} />

// Bad — string concatenation in JSX
<p>{'Welcome, ' + user.name}</p>
```

- Use template literals for computed class names when conditional logic is involved, or use a utility like `clsx`/`classnames` for complex cases:

```tsx
// Simple — template literal
<button className={`btn ${variant} ${disabled ? 'disabled' : ''}`} />

// Complex — clsx
import clsx from 'clsx';
<button className={clsx('btn', variant, { disabled, loading })} />
```

---

## 2. TypeScript with React

### 2.1 Component typing

**Function components:** type props with an interface, use `React.FC` only when you need `children` with a default type or static properties (`displayName`, `defaultProps`). Most components don't need `React.FC`:

```typescript
// Good — explicit props type, no React.FC
interface UserProfileProps {
  userId: string;
  showAvatar?: boolean;
}

function UserProfile({ userId, showAvatar = true }: UserProfileProps) {
  // ...
}

// Also good — React.FC when you want the implicit children type
interface CardProps {
  title: string;
}

const Card: React.FC<CardProps> = ({ title, children }) => (
  <div className="card">
    <h2>{title}</h2>
    {children}
  </div>
);
```

When in doubt, skip `React.FC` and type props explicitly. `React.FC` adds an implicit `children?: ReactNode` which can be misleading — a component that doesn't render `children` shouldn't accept it.

**Event handlers:** type with React's event types, not raw DOM types:

```typescript
// Good
function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
  event.preventDefault();
}

function handleClick(event: React.MouseEvent<HTMLButtonElement>) {
  // ...
}

// Bad — using raw DOM types loses React-specific info
function handleClick(event: MouseEvent) { ... }
```

**Refs:** use `useRef` with the correct element type:

```typescript
// Good — typed ref
const inputRef = useRef<HTMLInputElement>(null);

// Good — ref to a value (mutable container)
const renderCount = useRef(0);
```

### 2.2 Props: interfaces vs type aliases

Follow `CODE_CONVENTIONS-nodejs.md` §2.3: use `interface` for props (they may be extended), `type` for unions and derived types:

```typescript
// Props — interface (extensible)
interface ButtonProps {
  label: string;
  onClick: () => void;
  variant?: 'primary' | 'secondary' | 'ghost';
  disabled?: boolean;
}

// Derived — type alias
type ButtonVariant = ButtonProps['variant'];
type ButtonSize = 'sm' | 'md' | 'lg';
```

### 2.3 Discriminated unions for component variants

When a component has distinct visual or behavioral variants, use a discriminated union in props:

```typescript
type AlertProps = {
  variant: 'info' | 'warning' | 'error' | 'success';
  title: string;
  children: ReactNode;
  onDismiss?: () => void;
};

function Alert({ variant, title, children, onDismiss }: AlertProps) {
  const icon = ALERT_ICONS[variant]; // variant narrows if needed
  // ...
}
```

For components whose props change shape based on a prop (polymorphic components), use discriminated unions:

```typescript
type InputFieldProps =
  | { type: 'text'; value: string; onChange: (v: string) => void }
  | { type: 'checkbox'; checked: boolean; onChange: (v: boolean) => void };

function InputField(props: InputFieldProps) {
  if (props.type === 'text') {
    // props.value and props.onChange(string) are available
    return <input type="text" value={props.value} onChange={e => props.onChange(e.target.value)} />;
  }
  // props.checked and props.onChange(boolean) are available
  return <input type="checkbox" checked={props.checked} onChange={e => props.onChange(e.target.checked)} />;
}
```

### 2.4 Generic components

When a component needs to work with a generic type (e.g., a table or select), use `React.FC` with a generic or a function with a generic call signature:

```typescript
interface TableProps<T> {
  data: T[];
  columns: ColumnDef<T>[];
  rowKey: (item: T) => string;
}

function Table<T>({ data, columns, rowKey }: TableProps<T>) {
  // ...
}

// Usage
<Table<User> data={users} columns={userColumns} rowKey={u => u.id} />;
```

---

## 3. Component architecture

### 3.1 Project shape

```
src/
  main.tsx              ← Entry point. Renders <App /> into root.
  app.tsx               ← Top-level app component. Router, providers, layout.
  components/           ← Reusable UI components (dumb/presentational).
    ui/                 ← Primitive components (Button, Input, Card).
    layout/             ← Layout components (Header, Sidebar, Container).
  features/             ← Feature modules (smart/container components).
    auth/
      auth-form.tsx
      auth-context.tsx
      use-auth.ts
      types.ts
    dashboard/
      dashboard.tsx
      widgets/
  hooks/                ← Shared custom hooks.
  lib/                  ← Utilities (formatters, validators, API client).
  stores/               ← State management (Zustand stores, Redux slices, etc.).
  routes/               ← Route definitions and page components (if using file-based routing, this is the pages/ directory).
  types/                ← Shared type definitions.
  styles/               ← Global styles, theme, CSS variables.
```

**Feature-based organization** is preferred over type-based for anything beyond a trivial app. Group by feature (`features/auth/`, `features/dashboard/`) so related components, hooks, types, and tests live together. The `components/ui/` folder holds cross-cutting primitives.

### 3.2 Component design rules

- **Single responsibility:** one component, one concern. If a component handles form state, validation, API submission, and display, it's doing too much.
- **Props are the API:** a component's props interface is its public contract. Document non-obvious props with JSDoc.
- **Dumb vs smart:** separate presentational components (receive data via props, emit events via callbacks) from container components (fetch data, manage state, wire things together). Presentational components are easier to test and reuse.
- **Composition over configuration:** prefer `children` and render props over long lists of boolean flags:

```tsx
// Bad — boolean proliferation
<Card showHeader showFooter collapsible compact />

// Good — composition
<Card>
  <Card.Header>Title</Card.Header>
  <Card.Body>Content</Card.Body>
  <Card.Footer>Actions</Card.Footer>
</Card>
```

### 3.3 Compound components

For components that work together as a group (Menu, Tabs, Accordion), use the compound component pattern:

```tsx
interface TabsContextValue {
  activeTab: string;
  setActiveTab: (id: string) => void;
}

const TabsContext = createContext<TabsContextValue | null>(null);

function Tabs({ defaultValue, children }: TabsProps) {
  const [activeTab, setActiveTab] = useState(defaultValue);
  return (
    <TabsContext.Provider value={{ activeTab, setActiveTab }}>
      <div className="tabs">{children}</div>
    </TabsContext.Provider>
  );
}

function TabList({ children }: { children: ReactNode }) {
  return <div className="tab-list" role="tablist">{children}</div>;
}

function Tab({ value, children }: { value: string; children: ReactNode }) {
  const ctx = useContext(TabsContext);
  if (!ctx) throw new Error('Tab must be used within Tabs');
  const isActive = ctx.activeTab === value;
  return (
    <button role="tab" aria-selected={isActive} onClick={() => ctx.setActiveTab(value)}>
      {children}
    </button>
  );
}

function TabPanel({ value, children }: { value: string; children: ReactNode }) {
  const ctx = useContext(TabsContext);
  if (!ctx) throw new Error('TabPanel must be used within Tabs');
  if (ctx.activeTab !== value) return null;
  return <div role="tabpanel">{children}</div>;
}

Tabs.List = TabList;
Tabs.Tab = Tab;
Tabs.Panel = TabPanel;
```

### 3.4 Custom hooks

Custom hooks encapsulate reusable stateful logic. Rules:

- **Name:** always start with `use` (`useAuth`, `useDebounce`, `useLocalStorage`).
- **Single concern:** one hook, one piece of logic. `useAuth` handles authentication; `useDebounce` handles debouncing. Don't combine them into `useAuthAndDebounce`.
- **Return a stable shape:** the return value's shape should not change between renders. Use a tuple or an object, consistently:

```typescript
// Object return — self-documenting
function useAuth() {
  return { user, login, logout, isLoading };
}

// Tuple return — concise for simple hooks (like useState)
function useToggle(initial = false): [boolean, () => void] {
  const [value, setValue] = useState(initial);
  const toggle = useCallback(() => setValue(v => !v), []);
  return [value, toggle];
}
```

- **No side effects in render:** hooks are the only place for side effects. Never call `fetch`, `localStorage`, or DOM APIs directly in the component body.
- **Dependencies:** `useEffect` and `useMemo`/`useCallback` dependencies must be exhaustive. Use the `react-hooks/exhaustive-deps` lint rule. If a dependency is intentionally omitted, explain why with a comment and suppress the lint rule:

```typescript
// eslint-disable-next-line react-hooks/exhaustive-deps — subscription is stable
useEffect(() => {
  const sub = stableEmitter.subscribe(handler);
  return () => sub.unsubscribe();
}, []);
```

### 3.5 Context usage

Context is for values that many components need at different depths (theme, auth, locale). It is **not** a substitute for props or a general state management solution:

- **Don't put frequently-changing values in context** without memoizing the provider value. Every context update re-renders all consumers:

```tsx
// Bad — new object every render
<AuthContext.Provider value={{ user, login, logout }}>
  {children}
</AuthContext.Provider>

// Good — stable reference
const value = useMemo(() => ({ user, login, logout }), [user, login, logout]);
<AuthContext.Provider value={value}>{children}</AuthContext.Provider>
```

- **Split contexts by update frequency:** put stable config (feature flags, theme) in one context and frequently-changing state (form values, cursor position) in another. This prevents unnecessary re-renders of consumers that only need the stable data.
- **Always provide a custom hook for context access:**

```tsx
function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}
```

### 3.6 Rendering lists

- Always provide a stable `key` — not the array index unless the list is static and never reordered:

```tsx
// Good — stable identifier
items.map(item => <ItemRow key={item.id} item={item} />)

// Bad — index as key (breaks on reorder, insert, delete)
items.map((item, i) => <ItemRow key={i} item={item} />)
```

- For long lists (>100 items), use virtualization (`@tanstack/react-virtual`, `react-window`). Rendering hundreds of DOM nodes that are off-screen wastes memory and causes jank.

### 3.7 Form handling

- For simple forms (1-3 fields, no complex validation), use controlled components with `useState`:

```tsx
function LoginForm() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    auth.login(email, password);
  };

  return (
    <form onSubmit={handleSubmit}>
      <input value={email} onChange={e => setEmail(e.target.value)} />
      <input type="password" value={password} onChange={e => setPassword(e.target.value)} />
      <button type="submit">Login</button>
    </form>
  );
}
```

- For complex forms (many fields, conditional validation, multi-step), use **React Hook Form** — it minimizes re-renders by using uncontrolled inputs with refs:

```tsx
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';

const schema = z.object({
  email: z.string().email(),
  password: z.string().min(8),
});

function ComplexForm() {
  const { register, handleSubmit, formState: { errors } } = useForm({
    resolver: zodResolver(schema),
  });

  return (
    <form onSubmit={handleSubmit(onSubmit)}>
      <input {...register('email')} />
      {errors.email && <span>{errors.email.message}</span>}
      <input type="password" {...register('password')} />
      {errors.password && <span>{errors.password.message}</span>}
      <button type="submit">Submit</button>
    </form>
  );
}
```

- Validate with **Zod** at the boundary (form submission, API response). Zod schemas double as TypeScript type inference:

```typescript
const UserSchema = z.object({
  name: z.string().min(1),
  email: z.string().email(),
  role: z.enum(['admin', 'editor', 'viewer']),
});

type User = z.infer<typeof UserSchema>;
```

---

## 4. State management

### 4.1 Hierarchy of needs

Choose the simplest state mechanism that fits:

1. **Local component state** (`useState`, `useReducer`) — data owned by one component.
2. **Lifted state** — data shared by a few sibling components; lift to the nearest common parent.
3. **Context** — data needed by many components at different depths, changes infrequently (theme, auth, locale).
4. **Server state** (`@tanstack/react-query`, `swr`) — data fetched from APIs, needs caching, deduplication, invalidation.
5. **Global client state** (Zustand, Redux) — data shared across unrelated features, changes frequently (cart, notifications, undo history).

**Default:** start with local state. Promote only when the need is clear. Premature global state is a common source of unnecessary re-renders and coupling.

### 4.2 Server state

Use **TanStack Query** (React Query) for all server-state management. Don't put API responses in `useState` or global stores — React Query handles caching, background refetching, deduplication, and invalidation:

```typescript
// Query
function useUser(userId: string) {
  return useQuery({
    queryKey: ['users', userId],
    queryFn: () => api.getUser(userId),
  });
}

// Mutation
function useUpdateUser() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: UserUpdate) => api.updateUser(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] });
    },
  });
}
```

Query keys are arrays — use a consistent hierarchy:

```typescript
['users']                    // all users
['users', userId]            // specific user
['users', userId, 'posts']   // user's posts
['users', userId, 'posts', postId] // specific post
```

### 4.3 Global client state

When global client state is needed, use **Zustand** — it's lightweight, has no boilerplate, and supports selective subscriptions to avoid unnecessary re-renders:

```typescript
import { create } from 'zustand';

interface NotificationStore {
  notifications: Notification[];
  add: (n: Notification) => void;
  dismiss: (id: string) => void;
}

export const useNotifications = create<NotificationStore>((set) => ({
  notifications: [],
  add: (n) => set((state) => ({ notifications: [...state.notifications, n] })),
  dismiss: (id) => set((state) => ({
    notifications: state.notifications.filter(n => n.id !== id),
  })),
}));

// Selective subscription — only re-renders when notifications array changes
function NotificationBadge() {
  const count = useNotifications((s) => s.notifications.length);
  return <span>{count}</span>;
}
```

### 4.4 Derived state

Compute derived values during render or with `useMemo`. Don't store them in `useState`:

```tsx
// Bad — redundant state
const [total, setTotal] = useState(0);
useEffect(() => {
  setTotal(items.reduce((sum, i) => sum + i.price, 0));
}, [items]);

// Good — derived during render
const total = useMemo(
  () => items.reduce((sum, i) => sum + i.price, 0),
  [items]
);
```

---

## 5. Effects and lifecycle

### 5.1 useEffect rules

- **Effects are for synchronization, not events.** If something happens because a user clicked, put it in the event handler, not in an effect:

```tsx
// Bad — effect for an event
useEffect(() => {
  if (searchQuery) {
    fetchResults(searchQuery);
  }
}, [searchQuery]);

// Good — event handler
function handleSearch(query: string) {
  fetchResults(query);
}
```

- **Effects for setup/cleanup:** subscriptions, timers, DOM measurements, third-party library initialization:

```tsx
useEffect(() => {
  const controller = new AbortController();
  fetch('/api/data', { signal: controller.signal })
    .then(res => res.json())
    .then(setData);
  return () => controller.abort(); // cleanup
}, []);
```

- **No effects for deriving state** — compute during render instead (§4.4).
- **Strict mode double-invocation:** effects run twice in development (React 18+). Design effects to be idempotent — cleanup must reverse setup completely.

### 5.2 useEffect dependency array

- **Never lie to the dependency array.** Omitting dependencies causes stale closures and subtle bugs.
- **Empty array (`[]`)** means "run once on mount, cleanup on unmount." Only use it when the effect truly has no dependencies.
- **Functions in dependencies:** if a function is called inside an effect, wrap it in `useCallback` or move it inside the effect:

```tsx
// Good — function inside effect
useEffect(() => {
  function fetchData() {
    // ... uses props.userId
  }
  fetchData();
}, [props.userId]);

// Also good — useCallback
const fetchData = useCallback(() => {
  // ... uses props.userId
}, [props.userId]);

useEffect(() => {
  fetchData();
}, [fetchData]);
```

---

## 6. Error handling

### 6.1 Error boundaries

Use error boundaries for class-component-level error catching. In function components, use `react-error-boundary`:

```tsx
import { ErrorBoundary } from 'react-error-boundary';

function App() {
  return (
    <ErrorBoundary
      FallbackComponent={ErrorFallback}
      onReset={() => window.location.reload()}
    >
      <MainContent />
    </ErrorBoundary>
  );
}

function ErrorFallback({ error, resetErrorBoundary }: { error: Error; resetErrorBoundary: () => void }) {
  return (
    <div role="alert">
      <h2>Something went wrong</h2>
      <pre>{error.message}</pre>
      <button onClick={resetErrorBoundary}>Try again</button>
    </div>
  );
}
```

### 6.2 Route-level error boundaries

Place error boundaries at route boundaries so one failing component doesn't take down the entire app:

```tsx
<Route path="/dashboard" element={
  <ErrorBoundary FallbackComponent={DashboardError}>
    <Dashboard />
  </ErrorBoundary>
} />
```

### 6.3 Async error handling in components

For async operations not managed by React Query, use state to track errors:

```tsx
function UserProfile({ userId }: { userId: string }) {
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    api.getUser(userId, { signal: controller.signal })
      .then(setUser)
      .catch((err) => {
        if (err.name !== 'AbortError') setError(err);
      });
    return () => controller.abort();
  }, [userId]);

  if (error) return <ErrorFallback error={error} />;
  // ...
}
```

---

## 7. Testing

### 7.1 Test runner and libraries

- **Vitest** — test runner (per `CODE_CONVENTIONS-nodejs.md` §7).
- **Testing Library** (`@testing-library/react`, `@testing-library/jest-dom`) — query and interact with components the way users do.
- **MSW** (Mock Service Worker) — mock API responses at the network layer, not by mocking fetch or modules.

### 7.2 Test file naming

```
src/
  components/
    user-profile.tsx
    user-profile.test.tsx     ← co-located
  features/
    auth/
      auth-form.tsx
      auth-form.test.tsx
```

Co-locate tests with the component. Alternatively, use a parallel `tests/` directory — pick one and be consistent.

### 7.3 Testing philosophy

**Test behavior, not implementation.** Query elements by role, label, or text — not by class name, data attribute, or component internals:

```tsx
// Good — user-centric queries
const button = screen.getByRole('button', { name: /submit/i });
const heading = screen.getByRole('heading', { level: 1 });
const input = screen.getByLabelText('Email');

// Bad — implementation details
const button = container.querySelector('.btn-primary');
const heading = container.querySelector('h1');
const input = container.querySelector('[data-testid="email-input"]');
```

Use `data-testid` only as a last resort when no semantic query works.

### 7.4 Test patterns

```tsx
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect } from 'vitest';
import { LoginForm } from './auth-form';

describe('LoginForm', () => {
  it('submits with valid credentials', async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(<LoginForm onSubmit={onSubmit} />);

    await user.type(screen.getByLabelText('Email'), 'user@example.com');
    await user.type(screen.getByLabelText('Password'), 'secret123');
    await user.click(screen.getByRole('button', { name: /login/i }));

    expect(onSubmit).toHaveBeenCalledWith({
      email: 'user@example.com',
      password: 'secret123',
    });
  });

  it('shows validation error for empty email', async () => {
    const user = userEvent.setup();

    render(<LoginForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole('button', { name: /login/i }));

    expect(screen.getByText('Email is required')).toBeInTheDocument();
  });
});
```

### 7.5 Mocking API responses

Use MSW to mock at the network level:

```tsx
import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';
import { afterAll, afterEach, beforeAll } from 'vitest';

const server = setupServer(
  http.get('/api/users/:id', ({ params }) => {
    return HttpResponse.json({ id: params.id, name: 'Test User' });
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());
```

This tests the real fetch path — including error handling for network failures — without hitting a real server.

---

## 8. Performance

### 8.1 When to optimize

Don't optimize prematurely. Measure first (React DevTools Profiler). The most common performance issues in React are:

1. Rendering lists without virtualization.
2. Unnecessary re-renders from unstable references (inline functions/objects in props).
3. Fetching data in `useEffect` instead of using a caching layer.
4. Large bundles from importing entire libraries.

### 8.2 Memoization

- **`React.memo`:** wrap components that receive the same props frequently and are expensive to render. Don't memoize everything — the comparison has a cost:

```tsx
const ExpensiveList = React.memo(function ExpensiveList({ items }: { items: Item[] }) {
  return <ul>{items.map(item => <li key={item.id}>{item.name}</li>)}</ul>;
});
```

- **`useMemo`:** for expensive computations that don't need to re-run every render:

```tsx
const sortedItems = useMemo(
  () => [...items].sort((a, b) => a.name.localeCompare(b.name)),
  [items]
);
```

- **`useCallback`:** when passing functions to memoized children or as effect dependencies:

```tsx
const handleClick = useCallback(
  (id: string) => setSelectedId(id),
  [] // stable reference
);
```

### 8.3 Code splitting

Split routes and heavy components:

```tsx
// Route-level splitting
const Dashboard = lazy(() => import('./features/dashboard/dashboard'));

function App() {
  return (
    <Suspense fallback={<LoadingSpinner />}>
      <Routes>
        <Route path="/dashboard" element={<Dashboard />} />
      </Routes>
    </Suspense>
  );
}
```

### 8.4 Stable references

Inline functions and objects in JSX create new references on every render, breaking memoization:

```tsx
// Bad — new function reference every render
<Item onClick={() => handleSelect(item.id)} />

// Good — stable callback
const handleClick = useCallback(() => handleSelect(item.id), [item.id, handleSelect]);
<Item onClick={handleClick} />
```

For simple components (not memoized), inline functions are fine. Only stabilize references when the child is memoized or when the function is an effect dependency.

---

## 9. Accessibility (a11y)

### 9.1 Baseline requirements

- **Semantic HTML:** use the right element. `<button>` for actions, `<a>` for navigation, `<nav>` for navigation regions. Don't use `<div onClick={...}>` when a `<button>` works.
- **ARIA attributes:** only when semantic HTML isn't enough. ARIA is a supplement, not a replacement, for proper HTML semantics.
- **Keyboard navigation:** all interactive elements must be reachable and operable via keyboard (Tab, Enter, Space, Escape).
- **Focus management:** after navigation, modal opening, or dynamic content changes, move focus to the appropriate element.
- **Alt text:** all meaningful images need `alt` text. Decorative images use `alt=""`.

### 9.2 Common patterns

```tsx
// Good — semantic button with proper role
<button type="button" aria-label="Close dialog" onClick={onClose}>
  <CloseIcon />
</button>

// Good — form with labels
<label htmlFor="email-input">Email</label>
<input id="email-input" type="email" required />

// Bad — div as button, no keyboard support
<div className="close-btn" onClick={onClose}>✕</div>
```

### 9.3 Testing accessibility

Use `@testing-library/dom` queries (`getByRole`, `getByLabelText`) which enforce accessibility semantics. Run `axe-core` in CI for automated a11y audits:

```tsx
import { axe, toHaveNoViolations } from 'jest-axe';
expect.extend(toHaveNoViolations);

it('has no accessibility violations', async () => {
  const { container } = render(<UserProfile user={testUser} />);
  const results = await axe(container);
  expect(results).toHaveNoViolations();
});
```

---

## 10. Cross-references

- For base TypeScript/Node.js conventions (ESM, strict mode, imports, error handling, async patterns, testing toolchain): `CODE_CONVENTIONS-nodejs.md`.
- For file and folder naming: `NAMING_CONVENTIONS.md`.
- For HTTP API and JSON wire format conventions: `API_CONVENTIONS.md`.
- For the design philosophy that motivates these conventions: `DESIGN_PHILOSOPHY.md`.
