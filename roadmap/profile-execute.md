# Structured `profile()` and the `execute()` terminal: design

Status: IMPLEMENTED in 0.1.0 (see "Implementation notes" at the end). It came before the transactions
work (`roadmap/transactions.md`). User documentation:
`book/src/users_guide/profiling_and_execute.md`. When this design and the code disagree, the
code wins.

## 1. Problem

- In Rhai, `profile()` returns a STRING (the rendered text table); scripts, the playground, the future wasm
  engine and the Python binding get no data. On failure it returns the error text as a normal string result
  (`crate::error::format_diagnostic`), so a script cannot tell a profile from a failure.
- There is no way to get the results AND the profile of one run. TinkerPop has none; Neo4j has it
  (`PROFILE` returns rows plus the plan with metrics).

## 2. Structured metrics (DECIDED)

Based on TinkerPop's `TraversalMetrics`, extended with our data. Keys are snake_case everywhere (Rust, Rhai,
JSON; the mapping to TinkerPop names is documented, not mirrored):

```
TraversalMetrics { duration, metrics: [Metrics], optimizer_rules_applied: [String] }
Metrics {
  id, name, duration,                                  // TinkerPop
  counts { traverser_count, element_count },           // TinkerPop traverserCount / elementCount
                                                       //   (= our count_out / bulk_out)
  percent_duration,                                    // TinkerPop percentDur annotation
  nested: [Metrics],                                   // TinkerPop nested metrics
  calls, count_in,                                     // ours, always present
  memory: Option<MemoryMetrics>,                       // ours, only with ProfileType::Memory
}
MemoryMetrics { bytes_materialized, bytes_retained, alloc_count, source: estimated | tracked }
```
In JSON/Rhai durations are `duration_ns` (integer nanoseconds).

**Extensibility rule (a future `ProfileType` must not force library users to adapt):**
- a new `ProfileType` adds only a NEW optional section per `Metrics` (like `memory`): `Option<..>` in Rust,
  a new key in Rhai/JSON, absent unless that type was requested;
- the meaning and type of an existing field never change;
- Rust structs are `#[non_exhaustive]` with getters (rule 15); consumers of maps/JSON ignore unknown keys.
  State this rule in the `///` of the types and in the book page; test that a profile without Memory has
  no `memory` key and that adding a type does not change existing keys.
- Replaces the former `StepMetricNode` / `ProfileContext` public structs (pre-release: free to reshape).

**Rhai:** `profile()` returns a dedicated `TraversalMetrics` type (field access `p.duration`,
`p.metrics[0].name`, `to_map()`, `to_json()`), so the session visualizer recognises it and renders today's
text table: the CLI looks the same, scripts get data. A failure inside `profile()` is a real script error.
TinkerPop compatibility: `profile()` still returns only metrics.

## 3. `execute()` terminal (DECIDED, name `execute`)

One terminal returning an execution object instead of N x M `to_list_with_profile()`-style terminals:

```rust
let exec = g.v().has_label("person").out("knows")
    .execute_with(ExecOptions::profile(ProfileType::Memory));
exec.results();   // what to_list() would return
exec.error();     // Option<&TraverserError>: a runtime error is PART of the object
exec.profile();   // Option<&TraversalMetrics>, present even on failure (partial, up to the failing step)
exec.into_result()?; // convenience for `?`
```
```js
let r = g.v().has_label("person").out("knows").execute(#{profile: true, profile_types: [ProfileType.Memory]});
r.results; r.error; r.profile
```
- `Execution` is `#[non_exhaustive]` and is the home for later per-run data: the `ChangeSet` of the run
  (transactions design), `mutated` (graph actually changed = non-empty change set), warnings, truncation.
- Errors (DECIDED): EVERY error is returned inside `Execution`, both runtime errors (with a
  partial profile if profiling was on) and errors that prevent execution from starting (plan/optimizer
  errors, invalid options, permission denial, resource-limit configuration). Callers handle ONE shape;
  `execute()` itself never returns `Err` / never throws in Rhai. `exec.into_result()` converts to `Result`
  for `?`.
- Error location (DECIDED): the error inside `Execution` carries WHERE it happened, as a
  property of the error (not of the metrics; it is needed without profiling too, e.g. to highlight the
  step in an editor):
  ```json
  "error": { "message": "...", "help": "...",
             "step_id": "3.1.0",
             "step_path": [ { "id": "3",     "name": "union(...)" },
                            { "id": "3.1.0", "name": "has(\"age\", gt(30))" } ] }
  ```
  - `step_id` uses the SAME numbering as `Metrics.id`, so a partial profile and the error join by id; no
    `failed_at` field in the metrics.
  - Nested steps (`union`, `where`, `repeat` bodies, `by()` / `local()` / `coalesce()` / `choose()` child
    traversals, ...): the id is a path alternating step index and child-traversal index:
    `<step>.<child>.<step>...`; `3.1.0` = top-level step 3, its child traversal 1, step 0 inside it.
    `step_path` lists the chain from the top level down to the failing step for readable messages.
  - Ids refer to positions in the OPTIMIZED plan (what `.profile()` shows, fused steps under their real
    names); they are static plan positions, not per-traverser occurrences. Mapping to a source span of the
    written query is a later, additive field.
  - Order of a step's child traversals (as the playground shows them): the step's own traversal arguments
    first, in argument order, then modulator children (`by()`, `times()`/`until()`/`emit()` bodies) in
    attachment order. Ids are assigned from the plan, not from execution order, so a branch that never runs
    leaves no gap.
  - Define the numbering once (shared by the profiler and the error path) and test it for every
    child-holding step kind.
- No rendered text inside the data (DECIDED): `TraversalMetrics` and `Execution` are pure data; turning
  them into text is the visualizer's job (Rhai session visualizer, `to_text()` / `to_table()`).
- Profiling instruments steps, so a profiled run is slightly slower; results are identical. Document it.
- DSL parity (rule 2): `execute()` / `execute(opts)` in Rhai; Python binding exposes the same object.

## 4. Front ends

- Playground: runs a query without an explicit terminal through `execute(...)`, updates the Profile panel
  whenever a run returns a profile, "Profile" and "Memory" toggles.
- Memory on wasm: `MemorySource::Estimated` is always available; the wasm build installs the
  `TrackingAllocator` (single-threaded `GlobalAlloc` wrapper), so the playground shows `Tracked` figures.
- CLI: unchanged look; `execute()` results rendered by the visualizer (results, then the profile table).

## 5. Implementation outline (DONE)

1. Metrics types + conversion from the profiling internals; Rust API; tests incl. the extensibility rule.
2. Rhai `TraversalMetrics` type, visualizer rendering, `profile()` errors become errors; CLI tests.
3. `execute()` / `execute_with()` + `Execution`; Rhai `execute(opts)`; partial profile on failure; tests.
4. Book page (profiling + execute), TinkerPop mapping table, help texts that mention `profile()` (rule 6).
5. Python binding parity.

## 6. Implementation notes

- `Metrics` has two optional sections beyond the decided list, added under the extensibility
  rule: `timing` (`min_duration`/`max_duration`, present once the step completed a call) and
  `loops` (`total_loops`/`max_depth`/`avg_loops_per_traverser`, `repeat()` only). They keep the
  table's `[min/avg/max]` and loop lines. A third optional key, `path_recording` (`"full"`,
  `"labels(a,b)"`), carries the step's path recording mode; `name` stays plain step text and only
  the table renders ` [path: ..]`.
- `MemorySource::NotRequested` is removed: memory not requested means the `memory` section is
  absent (`None` / no key), never a sentinel value.
- The error location lives on the error itself: `TraverserError::step_location()` returns
  `StepLocation { step_id, step_path }`, built from the `StepFailed` chain, which gained
  `child_index` next to `step_index`. Every terminal's errors have it, not only `execute()`'s.
  The Rhai/JSON error map is `{message, help?, step_id?, step_path?, diagnostic}`: `message` is
  plain text, `diagnostic` the full rendered text.
- The child numbering is defined once by `GraphStep::child_traversals_mut` (which also drives the
  optimizer's nested pass) and stored per plan in `AnonymousTraversal::child_index`.
- `TraversalMetrics.duration` covers the run and the bulk expansion, not the materialization of
  the results; `profile()` does not materialize results at all.
- The table lost the `TOTAL: prepare result:` and `TOTAL: collect stats:` rows (those figures no
  longer exist); `TOTAL: execute:` stays.
- Step paths in diagnostics use the same ids: `Step #1.1` became `Step #1.0.1` (child index
  included).
- `mutated` and `changes` came with the transactions work (`Execution::mutated()`/`changes()`):
  whether a run *may* mutate is known from the plan, whether it actually changed the graph only from
  a change set.

