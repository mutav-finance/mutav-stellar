---
name: react-hook-composition
description: Use when creating data fetching hooks — enforces single-purpose hooks with no side effects; toasts, analytics, and error handling belong in view model hooks, not data hooks
---

# React Hook Composition

## Overview

Data fetching hooks have **one job**: fetch and return data with status. Side effects (toasts, analytics, navigation) belong in **view model hooks**, not in data fetching hooks.

For the pattern where side effects are handled, see `react-component-view-model-pattern`.

## Core Principle

**Data fetching hooks return data and status. Nothing else.**

They must NOT:

- Show toasts
- Handle errors with `useEffect`
- Log analytics
- Trigger navigation
- Manage refs for previous values

## Current state of `useQuery` in this project

SGR currently uses raw `useQuery` from `convex/react`. That hook returns `undefined` for both loading and error states, requiring manual derivation. This is a known temporary state — a shared `useQuery` wrapper that exposes `{ data, isPending, isError, error }` is on the deferred conventions list (install `convex-helpers`, build wrapper using `makeUseQueryWithStatus`).

Until the wrapper exists, data hooks should still **return a normalized shape** so consumers depend on the shape, not on the underlying Convex behavior:

```tsx
"use client";

import { useQuery } from "convex/react";
import { api } from "@/convex/_generated/api";
import type { ContractId } from "@/convex/contracts/domain";

export function useContract(contractId: ContractId) {
  const data = useQuery(api.contracts.get, { contractId });
  return {
    data,
    isPending: data === undefined,
    isError: false,
    error: null,
  };
}
```

When the shared wrapper lands, swap the import — consumers don't change.

## Correct Pattern (post-wrapper)

```tsx
import { useQuery } from "@/hooks/useQuery"; // shared wrapper, deferred
import { api } from "@/convex/_generated/api";
import type { ContractId } from "@/convex/contracts/domain";

export function useContract(contractId: ContractId) {
  const { data, isPending, isError, error } = useQuery(
    api.contracts.get,
    { contractId },
  );
  return { contract: data, isPending, isError, error };
}
```

## Entity-by-id hooks

Require the domain Id type, not `string | null` or `undefined`. Callers handle "no id" before calling. Assert from `useParams()` only at the boundary, with a comment.

```tsx
// In a page boundary:
const params = useParams<{ id: string }>();
// route param validated by route shape
const contractId = params.id as ContractId;
```

## Do

| Pattern                                      | Description                                          |
| -------------------------------------------- | ---------------------------------------------------- |
| Return `{ data, isPending, isError, error }` | Standard return shape for all data hooks             |
| Keep hooks pure                              | No side effects, just data transformation            |
| Compose in view models                       | View model hooks combine data hooks and add behavior |
| Name by data fetched                         | `useContract`, `useContractHistory`                  |
| Require entity Id types                      | `ContractId`, not `string`                           |

## Don't

| Anti-pattern                                   | Problem                                                                                           |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Optional id (`string \| null`) on entity hooks | Bypasses type safety; hook is meaningless without id. Callers must handle "no id" before calling. |
| `useEffect` with error handler                 | Hidden side effect, can't be disabled                                                             |
| `useToast()` inside data hook                  | Forces toast on every consumer                                                                    |
| `useAnalytics()` inside data hook              | Hidden tracking, testing nightmare                                                                |
| `useRouter().push()` inside data hook          | Unexpected navigation side effect                                                                 |
| `useRef` for error deduplication               | Tracking complexity for effects that shouldn't exist                                              |
| Type assertions inside hooks to bypass errors  | Hides bugs; only assert at boundaries (e.g. route params) with a comment                          |

## Where Side Effects Belong

| Layer           | Responsibility                                            |
| --------------- | --------------------------------------------------------- |
| Data hook       | Fetch data, return `{ data, isPending, isError, error }`  |
| View model hook | Compose data hooks, handle side effects, provide UI state |

## Placeholder hooks

When a Convex query isn't ready yet, expose a stub hook with the same shape as the real one. Keeps view models and UI stable, and avoids leaking "no implementation" states.

```tsx
import type { ContractId, Contract } from "@/convex/contracts/domain";

// @placeholder — replace with useQuery(api.contracts.list, ...) when ready
export function useContractsList(_orgId: string): {
  data: Contract[];
  isPending: boolean;
  isError: boolean;
  error: Error | null;
} {
  return { data: [], isPending: false, isError: false, error: null };
}
```

Use `@placeholder` or `// TODO: replace with useQuery(...)` so it's grep-able.

## Quality Checks

1. **Entity hooks require Id type**: Hooks that query by entity id take required `ContractId`, not `string | null`
2. **No side effect imports**: No `useToast`, `useAnalytics`, `useRouter` inside data hooks
3. **No `useEffect` for errors**: Never `useEffect(() => { if (error) ... }, [error])`
4. **No refs for tracking**: No `useRef` to track previous values for deduplication
5. **Standard return shape**: Returns `{ data, isPending, isError, error }` (or domain-named alias like `{ contract, isPending, isError, error }`)
6. **Single purpose**: Hook name describes what data it fetches, nothing more
7. **No raw Convex types**: Use type aliases from entity/domain modules (e.g. `ContractId`), never raw `Id<'contracts'>` outside entity files

> **Note:** These are data fetching hooks, not view model hooks. View model hooks (`use{ComponentName}`) compose data hooks with UI logic. See `react-component-view-model-pattern`.

See also: `react-component-view-model-pattern`, `convex-document-types`
