# Step - concat

`concat(string...)` appends strings to the string a traverser holds; `concat(traversal...)` appends
the **first** result of each child traversal, run on the incoming traverser (its path included).
The arguments are all strings or all traversals. Without an argument the string passes unchanged.

```rhai
graphersal> g.V().hasLabel("software").as("s").values("name").concat(" uses ").concat(__.select("s").values("lang")).to_list()
"lop uses java"
"ripple uses java"

graphersal> g.V("1").outE("created").as("e").V("1").values("name").concat(__.select("e").label(), __.select("e").inV().values("name")).to_list()
"markocreatedlop"

graphersal> g.inject("a").concat(__.inject("c")).to_list()
"aa"
```

The last example shows the first-result rule: `__.inject("c")` yields the incoming `"a"` first.

## null

A `null` is skipped wherever it occurs, as the incoming value, as a constant (`()` in Rhai) or as a
child result. The result is `null` only when everything was `null`:

```rhai
graphersal> g.inject((), "a").concat((), "b").to_list()
"b"
"ab"

graphersal> g.inject(()).concat().to_list()
()
```

## Errors

- An incoming value that is neither a string nor `null` is a cast error. Convert it first, for
  example `values("age").as_string().concat(" years old")`.
- A constant argument that is neither a string nor `null` (`concat(1)`) is rejected before the
  traversal runs.
- A child traversal that yields nothing, or a value that is not a string, is a
  `ValueError::StringFunctionInput`. Make the child always produce a string, for example with
  `coalesce(..., constant(""))` or `as_string()`.

Rust API: `concat(vec!["a", "b"])` (a `null` argument is `ElementProperty::Null`) and
`concat_traversal(vec![__::select("a").values("lang")])`.
