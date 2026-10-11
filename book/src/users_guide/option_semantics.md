# Option Keys of choose() and branch()

This page lists where Graphersal's `option()` keys and unmatched-traverser rules differ from, or
deliberately extend, Apache TinkerPop 3.8.2. Everything not listed follows the TinkerPop feature
files (`Choose.feature`, `Branch.feature`).

## Rules in one table

| | `choose(traversal)` | `branch(traversal)` |
|---|---|---|
| option keys | literal, predicate (`P.between(26, 30)`), `Pick.none`, `Pick.unproductive` | the same, plus a traversal key and `Pick.any` |
| match | the first matching option, in declaration order | every matching option |
| selector produced a value, nothing matches | the `Pick.none` option (the first one); without one the traverser **passes through unchanged** | every `Pick.none` option; without one the traverser is dropped |
| selector produced nothing | the `Pick.unproductive` option (the first one); without one the traverser **passes through unchanged**, even when a `Pick.none` option exists | every `Pick.unproductive` option; without one the traverser is dropped |
| traversal key, `Pick.any` | error when the traversal executes | allowed; `Pick.any` is additive |

A literal key is compared like `P.eq(literal)`: numbers match by value (`option(2, ..)` matches the
integer 2 and the float 2.0). A literal or predicate operand is always data: a bare string never
names a step label or a vertex id. A traversal key matches when it produces at least one result with
the selector's value as its start.

## Deviations

- **`Pick.any` and an unproductive selector.** `Pick.any` is documented as "always", so it also runs
  for a traverser whose selector produced nothing. The TinkerPop feature files do not cover this
  case.
- **Traversal key and paths.** A traversal key starts at the selector's value but keeps the path of
  the choosing traverser, so `path()` inside a key sees that path.
- **Profile attribution.** `.profile()` lists the steps of a traversal key as nested steps of the
  option, before the steps of its branch.
- **Order of passed-through traversers.** When a `choose()` option holds global state
  ([Branch Children](branch_batches.md)), traversers that pass through unchanged are emitted after
  the option buckets. TinkerPop does not promise an order.
- **Errors.** `choose()` with a traversal key or `Pick.any` raises `InvalidOption` when the
  traversal executes, not when the step is built. The error text differs from TinkerPop's; only the
  error type is compared by the conformance suite.
- **`Pick.unproductive`** is implemented for both steps. `discard()` and `fail()` in the TinkerPop
  option scenarios are separate steps that are not implemented yet.
