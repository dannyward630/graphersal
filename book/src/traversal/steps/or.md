# Step - or

`or(a, b, ...)` keeps a traverser when at least one child traversal produces a result for it.

```rhai,graph=modern
g.V().or(__.has("age", P.gt(30)), __.hasLabel("software")).values("name")
```

`or()` with no argument is the infix form `a.or().b`, with the operand rules of the infix
[`and()`](and.md#the-infix-form-aandb); `and()` binds tighter, so `a.and().b.or().c` is
`or(and(a, b), c)`.

```rhai,graph=modern
g.V().emit(__.has("name", "marko").or().loops().is(2)).repeat(__.out()).values("name")
// ["marko", "ripple", "lop"]
```

Rust: the prefix form `or(vec![a, b])`; the infix form is a DSL notation.
