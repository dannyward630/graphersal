# Step - tail

`tail(n)` keeps the last `n` traversers of the stream; `tail()` is `tail(1)`. `tail(Scope.local, n)`
keeps the last `n` elements of the list each traverser holds.

```rhai,graph=modern
g.V().values("name").order().tail()           // ["vadas"]
g.V().values("name").order().tail(2)          // ["ripple", "vadas"]
```

Rust: `tail(n)`, `tail_scoped(Scope::Local, n)`.
