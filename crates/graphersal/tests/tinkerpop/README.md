# Vendored TinkerPop conformance suite

This directory vendors part of the Apache TinkerPop project so that Graphersal's Gremlin
conformance can be tested offline, reproducibly, against a pinned upstream version. See
"Runner, baseline and report" below for how to use it.

## Pinned version

- **Tag:** `3.8.2`
- **Commit:** `952d8d8e4c3cd37ed8e192ecb97348e6e82594a9`
- **Tag date:** 2026-09-01
- **Source:** https://github.com/apache/tinkerpop

## Contents

- `features/` — the full Gherkin feature tree, copied verbatim from
  `gremlin-test/src/main/resources/org/apache/tinkerpop/gremlin/test/features/` (159 `.feature`
  files plus the `data/` subfolder they reference for parametrized scenarios). Each file carries
  its own Apache-2.0 header.
- `data/` — the stock scenario graphs:
  - `tinkerpop-modern.xml`, `grateful-dead.xml` — GraphML, readable directly by Graphersal's `io`
    importer.
  - `tinkerpop-crew.json`, `tinkerpop-sink.json` — **GraphSON** 3.0 (upstream ships no GraphML
    export of these two datasets at this tag), read by Graphersal's GraphSON importer. `sink` is
    loaded and runs. `crew` is the multi-/meta-property dataset: it imports (several entries become
    one array, meta-properties go into `_meta`), but its scenarios need list cardinality and
    properties on properties, which Graphersal does not have by design, so they are deliberate
    incompatibilities (group `multi-meta-properties`, see below) and are not run.
- `api-3.8.2.txt` — the TinkerPop 3.8.2 traversal API inventory (source methods, steps, `__`
  methods, terminal methods, token classes with their members), derived from the upstream grammar
  `gremlin-language/src/main/antlr4/Gremlin.g4` and `__.java`. Generated, never edited by hand; see
  "API inventory" below. `gen_api_inventory.py` next to it is the generator (ours, not vendored).
- `harness/`, `baseline.toml` — the runner's harness modules and the ratchet baseline (ours, not
  vendored); see "Runner, baseline and report" below.
- `LICENSE` — the upstream Apache-2.0 license, copied verbatim.
- `NOTICE` — attribution for the vendored files, scoped to this directory (the repository root
  `NOTICE` repeats it for the project as a whole).

## License

Apache-2.0, same as this project. This directory contains files copied from the Apache TinkerPop
project (https://tinkerpop.apache.org), Copyright The Apache Software Foundation, licensed under
the Apache License, Version 2.0. See `LICENSE` and `NOTICE` in this directory.

## Upgrade procedure

1. Pick the new tag (e.g. `3.9.0`) and shallow-clone it:
   `git clone --depth 1 --branch <tag> https://github.com/apache/tinkerpop.git /tmp/tinkerpop-<tag>`
2. Re-copy `features/` and the datasets under `data/` (GraphML, plus the GraphSON-only ones), replacing this directory's contents
   (`LICENSE` too, in case it changed).
3. Record the new tag, commit SHA and tag date in this file.
4. Regenerate the API inventory for the new tag (see "API inventory" below) and delete the old
   `api-<old tag>.txt`; update `INVENTORY_FILE`/`INVENTORY_VERSION` in `harness/inventory.rs`.
5. Run the conformance suite (`cargo test --package graphersal --features script,io,display,serde
   --test tinkerpop_features`) and review the diff against `baseline.toml` like code: scenarios that
   newly fail are regressions to investigate before merging the upgrade; scenarios that newly pass
   are removed from the baseline in the same change. Update `SUITE_VERSION` in
   `harness/report.rs`, and refresh the headline numbers in
   `book/src/for_developers/tinkerpop_compliance.md`.

## API inventory

`harness/inventory.rs` compares `api-<tag>.txt` with what the Rhai engine registers and fills the
last section of `target/tinkerpop-report.md` ("Not registered at all"), plus one summary line on
stderr. It is informational: it never fails the test, except when the inventory file itself cannot
be read or parsed.

How the file was generated (and how to regenerate it on upgrade; Python 3, standard library only):

```bash
git clone --depth 1 --branch 3.8.2 https://github.com/apache/tinkerpop.git /tmp/tinkerpop-3.8.2
git -C /tmp/tinkerpop-3.8.2 rev-parse HEAD   # must print 952d8d8e4c3cd37ed8e192ecb97348e6e82594a9
cd crates/graphersal/tests/tinkerpop
python3 gen_api_inventory.py /tmp/tinkerpop-3.8.2 3.8.2 952d8d8e4c3cd37ed8e192ecb97348e6e82594a9 > api-3.8.2.txt
```

What the generator extracts:

- `[GraphTraversalSource]`: the first keyword of each `traversalSourceSelfMethod_*` and
  `traversalSourceSpawnMethod_*` rule, plus `tx` (`transactionPart`).
- `[GraphTraversal]`: the first keyword of each `traversalMethod_*` rule that `traversalMethod`
  references (overloads collapse into one name).
- `[__]`: the public static methods of `__.java` that are also grammar steps (the grammar accepts
  `__.` before any step; this drops the modulators and computer steps `__` does not declare, and
  the Java-only factories `__.start()` / `__.__()`).
- `[terminal]`: the `traversalTerminalMethod_*` keywords (`toList`, `next`, `iterate`, ...).
- `[tokens]`: every `Class.member` pair the grammar spells as `K_<CLASS> DOT K_<MEMBER>` (or via a
  `*StringConstant` rule, e.g. `WithOptions`, `IO`, `ShortestPath`). Predicate combinators
  (`P.and`/`P.or`/`negate`) are instance methods, not token members, and are not listed.
  Traversal strategies (`SubgraphStrategy`, ...) are not listed either; the grammar takes any class
  name there.

Cross-check done for 3.8.2: `GraphTraversal.java`'s methods returning `GraphTraversal` match the
grammar steps except `program` (lambda-only, not in the canonical language) and `iterate` (listed
under `[terminal]`).

## Runner, baseline and report

The runner is the integration test target `crates/graphersal/tests/tinkerpop_features.rs`; the
harness it includes lives in `harness/` (`gherkin.rs` parser, `notation.rs` expected values,
`translate.rs`, `dataset.rs`/`execute.rs`, `compare.rs`/`classify.rs`, `baseline.rs`/`report.rs`,
`inventory.rs`, `scope.rs`). Run it with

```bash
cargo test --package graphersal --features script,io,display,serde --test tinkerpop_features
```

It is also part of the plain `cargo test --package graphersal --features script,io,display,serde`.

- **Execution.** Every scenario runs through the Rhai script engine, the path users take, on a pool
  of worker lanes (one per core; `TINKERPOP_LANES=<n>` overrides) with a per-scenario timeout and
  `catch_unwind`. Read-only scenarios share one graph per dataset; mutating ones get a fresh copy.
- **Translator principle.** `translate.rs` rewrites only the *lexical* differences between canonical
  Gremlin (Groovy) and Rhai: numeric suffixes, list/map literals, string quoting, parameter and
  side-effect names, and Groovy's static imports (a bare `out()` becomes `__.out()`, a bare `desc`
  becomes `Order.desc`). It never papers over a semantic gap: a missing step, overload or token
  reaches the engine and fails there. A literal type Graphersal lacks (datetime, set, a
  BigInteger outside int64, a BigDecimal outside float64) is classified `translate`.
- **Number-width literals (test-surface deviation).** Graphersal has `int64` and `float64` only. The
  literals `1b`, `1s`, `1n` (and the expected values `d[1].b`, `.s`, `.n`) map to an integer, `1m`
  (`d[1].m`) to a float when the decimal fits a finite `f64`. The width cannot be told apart, so
  the 8 `semantics/Equality` scenarios check value equality across int/float only (expected: five
  ints `1` and three floats `1.0`); a result is never rewritten. 16 more scenarios pass by value equality only and are
  not evidence of byte/short/BigInteger support: 3 `AsNumber` (`5b/5s/5n`), 12 `Sum`, and
  `Inject::g_injectXbigintBoundaryValuesX`; the list is in `book/src/for_developers/tinkerpop_deviations.md`. See
  `book/src/users_guide/predicates.md`, section "Deviations". Token map keys (`T.label:`, `(T.id):`, `(label):`,
  `Direction.OUT:`, `(from):`, and the notation keys `t[..]`/`D[..]` of parameter maps) become the
  DSL's reserved string keys `"T.label"`/`"T.id"`/`"Direction.OUT"`/`"Direction.IN"`; a bare
  `label:` stays the string key `"label"` (grammar `nakedKey`), and an `M[..]` key stays
  `translate`.
- **Classes.** Every scenario gets exactly one of `pass`, `skipped` (`@GraphComputerOnly`, a dataset
  that cannot be loaded, upstream "unsupported test"), `translate`, `missing` (Rhai "Function/
  Variable not found"; the name is extracted), `runtime`, `wrong_result`, `wrong_error`, `timeout`,
  `panic`. The headline groups them: **Unsupported** (`missing` + `translate`: the step, overload,
  token or literal does not exist in Graphersal) and **Wrong behaviour of supported steps** (`runtime`,
  `wrong_result`, `wrong_error`, `timeout`, `panic`: it executes but differs or fails; the number to keep at
  0), each with its share, plus the same two columns in the per-category and per-file tables. An expected error (`the traversal will raise an error`) matches by type: any
  non-`missing` runtime error passes, whatever its message text (our own messages are kept). This is a deliberate
  deviation of the harness: TinkerPop's scenarios assert the exact error text, Graphersal's errors have
  their own wording (with `help()`), so only the fact and the kind of failure are compared.
- **Out of scope.** Scenarios that configure TinkerPop strategies are excluded from the
  compatibility scope (see below): class `excluded`, never executed, in no total or percentage and
  never in the baseline (an entry for one is stale, like an entry for a nonexistent id).
- **Comparison normalizations.** Ids compare tolerantly (`d[1].i` matches the string id `"1"`);
  numbers compare by class only (integer vs. floating point), and two `float64` result values are equal within 1 ulp (`compare::floats_within_one_ulp`: `NaN` equals `NaN`, infinities exact, `+0` equals `-0`; absorbs libm vs. Java last-digit differences such as `sin(4.0)`, never applied to integers, ids, orderings or errors); an `ordered` expectation is relaxed
  to a multiset match when the translated traversal has no explicit ordering step (reported as an
  "order-relaxed pass").
- **Tree assertions.** `the result should be a tree with a structure of` is supported
  (`compare::parse_tree` / `compare_tree`): the single result is a map of key to nested map, a leaf
  is an empty map, keys are compared by value (vertices by id, numbers by class, strings), and the
  siblings of a node match as a set because TinkerPop trees are unordered. `subgraph` assertions are
  still not evaluated (class `translate`).
- **Ratchet baseline.** `baseline.toml` lists every non-passing scenario with its class and a
  one-line reason. The test fails when a scenario outside the baseline fails (a regression) and when
  a baseline entry passes now (stale entry: remove it in the same change). A class change of a
  still-failing entry is reported, not failed. Regenerate the file and review the diff like code:

  ```bash
  TINKERPOP_UPDATE_BASELINE=1 cargo test --package graphersal --features script,io,display,serde --test tinkerpop_features
  ```
- **Report.** Every run prints a summary to stderr and writes the full report to
  `target/tinkerpop-report.md` (`$CARGO_TARGET_DIR/tinkerpop-report.md` when set): headline,
  per-category and per-file tables, failures ranked by missing name / exercised step, class
  changes, order-relaxed passes and the API inventory. `TINKERPOP_DETAILS=1` also prints one line
  per non-passing scenario.
- **Book page sync.** The machine-derivable numbers of `book/src/for_developers/tinkerpop_compliance.md`
  (blocks `scope`, `headline`, `categories`, `inventory`, `excluded`, `incompat`, each between
  `<!-- tinkerpop:begin NAME -->` and `<!-- tinkerpop:end NAME -->`) are generated by `harness/page.rs`
  from the same renderers as the report (`report.rs`; the only difference is the `../` prefix of the doc
  links in `incompat`). A normal run compares the blocks with the page and FAILS `tinkerpop_conformance`
  on any difference (only trailing whitespace is ignored) with the differing lines and the update
  command; it never writes the page. `TINKERPOP_UPDATE_BASELINE=1` rewrites `baseline.toml` and the text
  between the markers, nothing else (the prose, step tables and the `Taken on` line are hand-written).
  A missing, duplicated, unclosed or unknown marker is an error. The page path resolves through
  `CARGO_MANIFEST_DIR`. To add a block: render it in `page::render_blocks` and put the marker pair on
  the page. Commit the regenerated page with the change that moved the numbers.

## Out of scope

Principle: Graphersal provides subgraph views, partitions, read-only mode and row-level security
through its own access-control mechanism, and execution settings through its own unified,
hierarchical execution-metadata mechanism (replacing `with()`/`withStrategies()`/parameters), not
through TinkerPop `TraversalStrategy`s; GraphComputer/OLAP strategies do not apply to an in-memory
OLTP engine. So every scenario whose traversal, initializer or `GraphCount` traversal calls
`withStrategies(...)`/`withoutStrategies(...)` (bare class or `new X(...)`) is excluded:
class `excluded`, not executed, no baseline entry (an entry for it is stale), in no total or
percentage, and a feature file whose scenarios are all excluded is left out of the per-file table.
`withSideEffect`, `withSack` and `with(key, value)` stay in scope. The match is on the strategy
call, never on the file name. The table lives in `harness/scope.rs`:

- access control (views/partitions/read-only/row-level security): `SubgraphStrategy`,
  `PartitionStrategy`, `ReadOnlyStrategy` — 93 scenarios;
- GraphComputer/OLAP (all `process.computer.traversal.strategy` classes in 3.8.2):
  `VertexProgramStrategy`, `VertexProgramRestrictionStrategy`, `ComputerVerificationStrategy`,
  `ComputerFinalizationStrategy`, `HaltedTraverserStrategy`, `MessagePassingReductionStrategy`,
  `GraphFilterStrategy` — 16 scenarios;
- strategy configuration not supported: every other strategy the suite names (`CountStrategy`,
  `RepeatUnrollStrategy`, `ProductiveByStrategy`, `SeedStrategy`, ...) plus a catch-all for any
  other `*Strategy` — 106 scenarios.

When a scenario names several, the strategy listed first in the table decides the reason.

## Deliberate incompatibilities (second exclusion family)

Scenarios where Graphersal differs from TinkerPop on purpose (a user decision) or does not plan the
feature are excluded too, from an explicit table of scenario ids: `INCOMPATIBILITIES` in
`harness/scope.rs`, 183 ids in 10 codes, each with a title, a why and a book page: `select-undeclared-label` (10, `select("a")` and `select(Pop.x, ..)`),
`property-element-order` (5), `multi-meta-properties` (41, including the 16 `crew` scenarios), `null-property-removal` (3), `map-to-string` (2), `path-retraction-artefact` (1), `match-step` (36), `extra-number-types` (42), `io-step` (12), `graph-algorithms` (31). `call()` is not excluded: it is unsupported (no service model yet). Ids only,
never tags or content, so a scenario added by a suite upgrade is never excluded silently. They behave like
the strategy exclusions: class `excluded` (outcome detail `incompat:<code>`), not run, in no total or
percentage, no baseline entry (an entry is stale), and a strategy exclusion wins. The report has a
"Deliberate incompatibilities" section with the table and the id lists.

Two guard tests in `tinkerpop_features.rs` protect the table: `deliberate_table_is_consistent` (every
id exists in the vendored features, ids and codes are unique, no id is also strategy-excluded, every doc
page exists) and `deliberate_scenarios_do_not_pass_silently` (the listed scenarios are executed anyway and the test
fails if one passes: the deviation is gone). To move a scenario back into scope: remove its id from
`INCOMPATIBILITIES`, run the suite with `TINKERPOP_UPDATE_BASELINE=1`, review that the baseline gains
its entry (or the scenario passes), and update the compliance and deviations pages.

Headline (2026-10-04): 280 of 2072 scenarios excluded (215 strategies, 65 deliberate
incompatibilities); **1247 / 1792 in-scope scenarios pass (69.6 %)**. Current numbers:
`book/src/for_developers/tinkerpop_compliance.md`.
