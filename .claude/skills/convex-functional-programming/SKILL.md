---
name: convex-functional-programming
description: Use when writing Convex queries, mutations, or actions — separates pure business logic from database operations, composes enrichments, and returns Result<TData, TError> instead of throwing
---

# Convex Functional Programming

Functional programming patterns adapted for Convex backend development. The Result pattern is the project default — domain code returns `Result<TData, TError>` instead of throwing.

## Convex-Specific FP Concepts

In Convex, "pure" has a modified meaning due to database operations within transactions:

| Standard FP              | Convex Adaptation                                            |
| ------------------------ | ------------------------------------------------------------ |
| No side effects          | Database reads acceptable (deterministic within transaction) |
| Same input = same output | Same input + same db state = same output                     |
| No external state        | `ctx.db` is the controlled external state                    |

## Rules

| Rule                          | Description                                                                                 |
| ----------------------------- | ------------------------------------------------------------------------------------------- |
| Separate pure from impure     | Extract business logic to pure helper functions; handlers coordinate db and compose helpers |
| Explicit mutations            | Database writes (`patch`, `insert`, `delete`) only in handlers, never hidden in helpers     |
| Immutable updates             | Use `patch` with spread operators; never mutate retrieved documents                         |
| Declarative transforms        | Use `map`, `filter`, `reduce` instead of imperative loops                                   |
| Result pattern for validation | Return `Result<TData, TError>` directly — never use helper functions                        |
| Composable enrichment         | Build reusable async enrichment functions for cross-document data                           |

## The Result Pattern

All domain operations return `Result<TData, TError>` from `@/lib/result`. Practically eliminates try/catch in domain code (acceptable only at external API boundaries inside provider implementations or webhook handlers — see `convex-workpool` once adopted).

```typescript
import type { Result } from "@/lib/result";

// Define named types for each function's success and error shapes
type CreateContractSuccessResult = { contractId: ContractId; status: ContractStatus };
type CreateContractErrorResult = { code: "INVALID_INPUT" | "DUPLICATE_CONTRACT" };

function createContract(
  args: CreateContractArgs,
): Result<CreateContractSuccessResult, CreateContractErrorResult> {
  if (!args.tenant.cpf) {
    return {
      success: false,
      error: { code: "INVALID_INPUT" },
      message: "Tenant CPF is required",
    };
  }
  return {
    success: true,
    data: { contractId, status: "pendente" },
    message: "Contract created",
  };
}
```

**Rules:**

- Always return `Result<T>` objects directly as plain object literals
- Declare explicit return types as `Result<{FunctionName}SuccessResult>` or `Result<{FunctionName}SuccessResult, {FunctionName}ErrorResult>` to guarantee type inference on `result.data` and `result.error`
- Never wrap returns in helpers like `createSuccessResult()` / `createErrorResult()`
- Discriminate on `result.success` at every consumer

## Do

### Pure Data Transformations

```typescript
// Pure helper - no ctx, no db, easily testable
function calculateTotalAvailableGuarantee(contracts: Contract[]): number {
  return contracts.reduce((sum, c) => sum + c.availableGuaranteeBRL, 0);
}

function filterActiveContracts(contracts: Contract[]): Contract[] {
  return contracts.filter((c) => c.status === "ativo");
}

// Handler coordinates db + pure helpers
export const getContractSummary = query({
  args: {},
  handler: async (ctx) => {
    const all = await ctx.db.query("contracts").collect();
    const active = filterActiveContracts(all);
    return {
      total: calculateTotalAvailableGuarantee(active),
      activeCount: active.length,
    };
  },
});
```

### Composable Query Enrichment

```typescript
import type { QueryCtx } from "./_generated/server";

async function enrichWithHistory(ctx: QueryCtx, contracts: Contract[]) {
  return Promise.all(
    contracts.map(async (contract) => {
      const history = await ctx.db
        .query("contractHistory")
        .withIndex("by_contract", (q) => q.eq("contractPublicId", contract.publicId))
        .collect();
      return { ...contract, history };
    }),
  );
}

export const listContractsWithHistory = query({
  args: {},
  handler: async (ctx) => {
    const contracts = await ctx.db.query("contracts").collect();
    return enrichWithHistory(ctx, contracts);
  },
});
```

> **Note:** When `convex-helpers` is installed, use `asyncMap` from `convex-helpers` instead of `Promise.all(items.map(...))` for sequential async iteration. See deferred-conventions note.

### Immutable Update Builders

```typescript
// For tables with discriminated metadata, build the full patch
function buildEncerradoPatch() {
  return {
    status: "encerrado" as const,
  };
}

function buildCanceladoPatch(reason: string) {
  return {
    status: "cancelado" as const,
    metadata: { status: "cancelado" as const, reason, canceledAt: new Date().toISOString() },
  };
}

// In handler — patches all related fields atomically
await ctx.db.patch(contractId, buildEncerradoPatch());
```

> **Note**: Never use conditional spreads (`...(status === "encerrado" ? { x } : {})`) to set variant-specific fields. Use discriminated metadata bags in the schema and build the full patch per variant. See `convex-document-types` for schema patterns.

> **Note**: `as const` is permitted for literal type narrowing (it's a narrowing assertion, not a type cast). This is distinct from `as Type` which is prohibited.

> **Note**: In production code, use type aliases (e.g., `ContractId`, `ContractHistoryId`) instead of raw `Id<'tableName'>`. Raw `Id<>` appears in these examples for brevity.

## Don't

| Anti-Pattern                                        | Fix                                                   |
| --------------------------------------------------- | ----------------------------------------------------- |
| Imperative loops for transformations                | Use `map`/`filter`/`reduce`                           |
| Mutating retrieved documents (`doc.items.push(x)`)  | Use `patch` with new object                           |
| Hidden mutations in helpers                         | Keep `ctx.db.patch/insert/delete` visible in handlers |
| Business logic mixed with db operations             | Extract to pure functions                             |
| Throwing for expected validation failures           | Return `Result<TData, TError>` directly               |
| `createSuccessResult()` / `createErrorResult()`     | Plain object literal returns                          |

## Quality Checks

1. **Pure separation**: All business logic in pure helper functions (no `ctx` parameter)
2. **Explicit mutations**: All `ctx.db.patch/insert/delete` calls visible in the handler body
3. **No document mutation**: Retrieved documents never modified directly
4. **Declarative style**: No `for` loops for transformations. Exception: `for...of` with early return is acceptable for short-circuit validation chains
5. **Testable helpers**: Pure functions can be unit tested without Convex setup
6. **Result returns**: Domain functions declare `Result<...SuccessResult, ...ErrorResult>` and return plain object literals

See also: `convex-document-types`, official `convex-quickstart`, `convex-create-component`
