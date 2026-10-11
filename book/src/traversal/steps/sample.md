# Step - sample

`sample(n)` keeps a random sample of `n` traversers of the whole stream, drawn without replacement
(TinkerPop `SampleGlobalStep`). `sample(Scope.local, n)` samples `n` elements of the list (or
entries of the map) each traverser holds (`SampleLocalStep`).

```rhai,graph=modern
g.with("random.seed", 42).V().values("name").sample(2).to_list()               // ["vadas", "josh"]
g.with("random.seed", 42).V().values("name").fold().sample(Scope.local, 3).to_list()
// [["josh", "peter", "marko"]]
g.V("1").values("age").sample(Scope.local, 5).to_list()                         // [29]
```

- A stream (or a list) of at most `n` elements passes unchanged.
- `sample(Scope.local, n)` keeps the drawn elements in the order they were drawn; a value that is
  not a list or map (a number, a vertex, a path, `null`) passes through.
- A negative `n` samples nothing.
- Merged traversers are sampled unit by unit, so the sample never stands for more than `n`
  traversers.
- In a `repeat()` body, `sample(n)` samples each iteration's frontier
  (`g.V().repeat(__.sample(2)).times(2)` yields 2 vertices); in `local()` it samples per
  traverser (`g.V().local(__.outE().sample(1))`).

## Weights

A `by()` after `sample(n)` weighs each traverser: it is drawn with a probability proportional to
its weight.

```rhai,graph=modern
g.E().sample(2).by("weight")                         // heavier edges are more likely
g.V().sample(1).by(__.outE().count())                // vertices with more out-edges are more likely
```

- The weight must be a number of at least 0; anything else fails with `ValueError::SampleWeight`,
  whose help shows how to convert it.
- A traverser whose `by()` is unproductive (a missing property) is dropped, as in `order()`.
- A weight of 0 is never drawn, so fewer than `n` traversers come back when fewer have a positive
  weight. TinkerPop's sampling loop does not end in that case; this is a
  [deliberate difference](../../for_developers/tinkerpop_deviations.md).
- Only one `by()` is allowed; a second one fails with `InvalidModulator`.

## Reproducible samples

`sample()`, `coin()` and `order().by(Order.shuffle)` draw from one random number generator per
execution, shared by child traversals. Without options it is seeded from the operating system's
entropy (also in the browser playground). With `g.with("random.seed", n)` (any integer) the same
query draws the same values on every run with the same Graphersal version:

```rhai,graph=modern
g.with("random.seed", 42).V().values("name").order().by(Order.shuffle).to_list()
// ["marko", "ripple", "peter", "lop", "vadas", "josh"]
```

The option is Graphersal's counterpart of TinkerPop's `SeedStrategy`. It does not reproduce the
draws of TinkerPop's Java `Random` for the same seed, and the `SeedStrategy` scenarios of the
TinkerPop suite stay out of scope with `withStrategies()`. See the
[execution options reference](../../users_guide/execution_options_reference.md).

## Order.shuffle

`order().by(Order.shuffle)` puts the stream (or, with `order(Scope.local)`, the list) into random
order. As in TinkerPop, a clause after the shuffle still sorts: the items are shuffled first and
then stably sorted by the clauses that follow the last `Order.shuffle`, so
`order().by(Order.shuffle).by("age")` is sorted by age with ties in random order.

Rust: `GraphTraversalSource::sample(n)`, `sample_scoped(Scope, n)`, `Order::Shuffle`;
`.with("random.seed", n)` on the source.
