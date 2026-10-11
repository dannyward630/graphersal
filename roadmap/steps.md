# Gremlin steps: candidates beyond 0.1.0

The live list of missing steps and tokens, ranked by the TinkerPop scenarios they hold back, is the
report of the conformance suite (`target/tinkerpop-report.md`, see
`book/src/for_developers/tinkerpop_compliance.md`). This page records design notes for steps that
need more than an implementation.

---

## `match()`

The TinkerPop 3 `match()` is excluded from the compatibility scope for good
(`book/src/for_developers/tinkerpop_deviations.md`, "Excluded for good"): declarative pattern
matching comes through a Cypher/GQL front end on the same engine. That fits the form TinkerPop itself
moved to: since 4.0.0-beta.3 TinkerPop replaced
the old `match()` with a new form: it takes a declarative query string, GQL by default (TinkerPop
calls it TinkerGQL, a subset of ISO GQL):

```
g.match("MATCH (a:person)-[:knows]->(b:person) WHERE a.age > $age", ["age": 30]).select("a","b").by("name")
```

The variables bound by `MATCH` enter the path history, and ordinary Gremlin steps (`select()` and
so on) work on them afterwards.

`match` is a reserved word in Rhai, so the DSL needs the keyword-token rewriting of
`script/engine.rs` (as for `in`, `as`, `is`, `where`, ...) before the step can be called by that
name. A GQL front end would also be the natural user of the vertex label set (`(n:A:B)`).
