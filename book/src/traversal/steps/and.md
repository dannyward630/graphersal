# Step - and

`and(a, b, ...)` keeps a traverser when every child traversal produces a result for it.

```rhai,graph=modern
g.V().and(__.outE("knows"), __.values("age").is(P.lt(30))).values("name")   // ["marko"]
```

## The infix form `a.and().b`

`and()` with no argument is TinkerPop's infix notation, rewritten as TinkerPop's
`ConnectiveStrategy` does before the traversal runs:

- the **left operand** is every step before `and()`, back to the start of the traversal; the start
  step of the root traversal (`V()`, `E()`, `inject()`, ...) and the step labels it carries stay
  outside;
- the **right operand** is every step after `and()` up to the end of the traversal; a further
  `and()` starts another operand (`a.and().b.and().c` is `and(a, b, c)`);
- `and()` binds tighter than [`or()`](or.md): `a.and().b.or().c` is `or(and(a, b), c)`.

```rhai,graph=modern
g.V().where(__.out("created").and().out("knows").or().in("knows")).values("name")
// ["marko", "vadas", "josh"]
g.V().has("name", "marko").and().has("age", 29)              // v[1]
```

Because the right operand runs to the end, a step written after the infix form is part of it:
`g.V().has("name", "marko").and().has("age", 29).values("name")` still yields the vertex, with
`values("name")` inside the `and()`. Wrap the connective in a child traversal
(`filter(__.has(..).and().has(..))`, `where(..)`) to continue after it.

Rust: the prefix form `and(vec![a, b])`; the infix form is a DSL notation.
