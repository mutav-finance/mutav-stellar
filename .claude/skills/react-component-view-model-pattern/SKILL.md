---
name: react-component-view-model-pattern
description: Use when building page or feature components — components are pure rendering, all logic and state lives in a view model hook named use{ComponentName}. Applies to client components only in Next.js App Router.
---

# React Component View Model Pattern

## Overview

Components are **pure rendering functions**. All logic, state management, and side effects belong in a **view model hook**. The component simply renders what the view model provides.

## Next.js App Router scope

This pattern applies to **client components only** (files with `"use client"` at the top, or any component imported by one).

- **Server components** — render-only by nature. They do their own data fetching with `await` directly in the component body. No view model hook needed.
- **Client components** — apply the view model pattern.

When a route page is a server component that loads data and passes it to a client component, the client component still uses the view model pattern for its own interactive state.

## Architecture

- **Component** — Pure rendering only. No `useState`, `useEffect`, `useCallback`, `useMemo`. Single hook call: `const vm = use{ComponentName}()`. Returns JSX based on vm properties.
- **View Model Hook** — All state, side effects, derived state, callbacks. Composes data fetching hooks. Returns everything the component needs.
- **Data Fetching Hooks** — Pure, single-purpose (see `react-hook-composition`).

## Naming Convention

| Component               | View Model Hook            |
| ----------------------- | -------------------------- |
| `ContractDetailPage`    | `useContractDetailPage`    |
| `ContractsTable`        | `useContractsTable`        |
| `TenantApprovalCard`    | `useTenantApprovalCard`    |
| `DocumentsList`         | `useDocumentsList`         |

**Rule:** If component is `{Name}`, view model hook is `use{Name}`.

## Component Template

```tsx
"use client";

function ContractDetailPage({ contractId }: { contractId: ContractId }) {
  const vm = useContractDetailPage({ contractId });

  if (vm.isPending) return <Skeleton />;
  if (vm.isError) return <ErrorDisplay />;

  return (
    <div>
      <h1>{vm.contract.publicId}</h1>
      <Button onClick={vm.handleArchive}>Archive</Button>
    </div>
  );
}
```

## View Model Template

```tsx
function useContractDetailPage({ contractId }: { contractId: ContractId }) {
  // 1. Compose data hooks
  const { contract, isPending, isError } = useContract(contractId);

  // 2. Local state
  const [isEditing, setIsEditing] = useState(false);

  // 3. Side effects
  useEffect(() => {
    /* analytics, etc. */
  }, []);

  // 4. Callbacks
  const handleArchive = useCallback(() => {
    /* archive logic */
  }, []);

  // 5. Derived state (only when computing objects/arrays — not primitives)
  const tenantDisplayName = useMemo(
    () => contract && formatTenantName(contract.tenant),
    [contract],
  );

  // 6. Return everything component needs
  return { contract, isPending, isError, isEditing, handleArchive, tenantDisplayName };
}
```

> **Memoization:** Primitive derived values (booleans, numbers, strings) don't need `useMemo`. Computed objects, arrays, or expensive transformations must use `useMemo` with precise deps — never bundled with `isPending`/`isError` in one broad `useMemo` (recalculates on every status change).

> **`useEffectEvent` (React 19):** When an effect calls a function but should only re-run on data changes, isolate the function reference with `useEffectEvent`. The effect declares only reactive values (e.g., `contractId`), not function references.

## File Organization

Co-locate the component and its view model:

```
src/components/contracts/
  ContractDetailPage.tsx      # Component (pure rendering)
  useContractDetailPage.ts    # View model hook
```

For pages under `src/app/`, the route file may be a server component that loads data and renders a client component holding the view model:

```
src/app/contracts/[id]/
  page.tsx                    # Server component — loads data, renders <ContractDetailPage />
src/components/contracts/
  ContractDetailPage.tsx      # Client component
  useContractDetailPage.ts    # View model hook
```

## Do

| Pattern                    | Description                               |
| -------------------------- | ----------------------------------------- |
| One hook call in component | `const vm = useComponentName()`           |
| All logic in view model    | State, effects, callbacks, derived values |
| Component only renders     | JSX based on view model output            |
| Memoize callbacks          | `useCallback` for all handlers            |
| Memoize computed objects   | `useMemo` only for objects/arrays/expensive — not primitives |

## Don't

| Anti-pattern                          | Problem                               |
| ------------------------------------- | ------------------------------------- |
| `useState` in component               | Logic leaking into rendering layer    |
| `useEffect` in component              | Side effects in rendering layer       |
| Inline handlers `onClick={() => ...}` | Creates new function each render      |
| Multiple hook calls in component      | Component should only call view model |
| Business logic in component           | All logic belongs in view model       |
| View model in a server component      | Hooks don't run there — restructure   |

## Quality Checks

1. **Single hook call**: Component has exactly one hook call: `const vm = use{ComponentName}()`
2. **No React hooks in component**: No `useState`, `useEffect`, `useCallback`, `useMemo`
3. **No inline handlers**: No `onClick={() => ...}` in JSX
4. **Pure rendering**: Component body is just conditional JSX based on vm
5. **Naming match**: View model hook name matches component name
6. **Client-only**: View model hooks live in client components (`"use client"`)

See also: `react-hook-composition`, official `next-best-practices`
