# Math Expressions

`math("<equation>")` evaluates an arithmetic equation for every traverser and emits the result,
always as a double. Graphersal follows TinkerPop's `math()` step, which uses the
[exp4j](https://www.objecthunter.net/exp4j/) expression language: the same operators, precedence,
functions and constants, the same variable resolution and the same errors. The equation is parsed
once, when the traversal is built, by Graphersal's own expression engine; it does not need the
`script` feature.

Every example on this page was run with `graphersal` on the default modern graph; the line after `//`
is its real output.

## Quick reference

```rhai,graph=modern
g.V("1").values("age").math("_ / 2").toList()                                  // 14.5
g.V().math("_ + 1").by("age").toList()                                         // 30.0, 28.0, 33.0, 36.0
g.V().as("a").out("knows").as("b").math("a + b").by("age").toList()            // 61.0, 56.0
g.V("1").project("a", "b").by("age").by(__.out().count()).math("a / b").toList()  // 9.666666666666666
g.withSideEffect("x", 100).V("1").values("age").math("_ + x").toList()          // 129.0
g.inject(1).math("2^3").toList()                                                // 8.0
```

In Rust the step is `math(expr)` on `GraphTraversalSource`, `AnonymousTraversal` and `__`, with
the same `by()` modulators as in the DSL.

## Syntax

An equation is built from:

- **numbers**: `12`, `1.5`, `.5`, `5.`, `1e3`, `1.2E-3`, `2.5e+2` (no sign, hex or `inf`
  literal; a sign is the unary operator);
- **the current value** `_` and **variables** (`a`, `price_2`): a name `[A-Za-z_][A-Za-z0-9_]*`
  that is not a function;
- **operators**, **functions** and **constants** from the tables below;
- **parentheses**: `(`, `[` and `{` all group and call (`sqrt[_]`), but a closer must match its
  opener.

There is no implicit multiplication, as in TinkerPop (which turns it off): `2 pi`, `2(3)` and
`2 _` are errors; write `2 * pi`.

A function name without parentheses applies to everything to its right up to the end of the
enclosing group, as in exp4j: `sin _` is `sin(_)`, `sin _ + 1` is `sin(_ + 1)`, and
`(sin _ + 1) * 2` is `sin(_ + 1) * 2`.

```rhai,graph=modern
g.inject(2).math("sin _ + 1").next()           // 0.1411200080598672  (= sin(3))
g.inject(100).math("log10(_) + log(e)").next() // 3.0
```

### Operators

Higher precedence binds tighter. Unary operators never take a pending binary operator, so
`-2^2` is `-(2^2)` and `2^-2^2` is `2^(-(2^2))`.

| Operator | Precedence | Associativity | Meaning |
|---|---:|---|---|
| `a + b` | 500 | left | addition |
| `a - b` | 500 | left | subtraction |
| `a * b` | 1000 | left | multiplication |
| `a / b` | 1000 | left | division; a zero divisor is an error |
| `a % b` | 1000 | left | remainder with the sign of the dividend (Java `%`); a zero divisor is an error |
| `a ^ b` | 10000 | right | power (right-associative) |
| `-a` | 5000 | right | negation |
| `+a` | 5000 | right | identity |

```rhai,graph=modern
g.inject(1).math("-2^2").next()   // -4.0
g.inject(1).math("2^3^2").next()  // 512.0
g.inject(-7).math("_ % 3").next() // -1.0
```

`+` and `-` are unary at the start, after an opening bracket, after `,` and after another
operator; everywhere else they are binary (`2--2` is `4`).

### Functions

The exp4j 0.4.8 built-in set. All take one argument except `pow`. Domain errors are not errors:
`sqrt(-1)` and `log(-1)` are NaN and `log(0)` is negative infinity, as in Java.

| Function | Meaning |
|---|---|
| `abs(x)` | absolute value |
| `acos(x)` | arc cosine (radians) |
| `asin(x)` | arc sine (radians) |
| `atan(x)` | arc tangent (radians) |
| `cbrt(x)` | cube root |
| `ceil(x)` | round up to an integer |
| `cos(x)` | cosine (radians) |
| `cosh(x)` | hyperbolic cosine |
| `cot(x)` | cotangent; an error where tan(x) = 0 |
| `exp(x)` | e raised to x |
| `expm1(x)` | exp(x) - 1 |
| `floor(x)` | round down to an integer |
| `log(x)` | natural logarithm |
| `log10(x)` | base-10 logarithm |
| `log1p(x)` | log(1 + x) |
| `log2(x)` | base-2 logarithm |
| `pow(x, y)` | x raised to y |
| `signum(x)` | sign: -1, 0 or 1 |
| `sin(x)` | sine (radians) |
| `sinh(x)` | hyperbolic sine |
| `sqrt(x)` | square root |
| `tan(x)` | tangent (radians) |
| `tanh(x)` | hyperbolic tangent |

There is no `round`; use `floor(_ + 0.5)`. `cot`, `expm1`, `log1p` and `pow` are exp4j built-ins
that TinkerPop's `math()` cannot use (it declares their names as variables, which exp4j rejects);
Graphersal accepts them.

### Constants

| Constant | Value | Meaning |
|---|---|---|
| `pi` | 3.141592653589793 | π |
| `π` | 3.141592653589793 | π |
| `e` | 2.718281828459045 | Euler's number |
| `φ` | 1.61803398874 | golden ratio (exp4j's 12-digit value) |

`pi` and `e` are variables with a fallback, as in exp4j: a map key, side effect or step label named
`e` wins, and the constant is used only when the name resolves to nothing else (TinkerPop fails in
that case). `π` and `φ` are always constants.

```rhai,graph=modern
g.withSideEffect("e", 2).inject(1).math("e + _").next()  // 3.0
```

## Variables and `by()`

`_` is the current traverser's value. Any other variable is resolved as in TinkerPop's `Scoping`,
in this order:

1. a key of the current value when it is a map (`project()`, `select()`, `valueMap()`,
   `elementMap()`, `group()` results, a map-valued property);
2. a side effect (`withSideEffect()`, `aggregate()`, `store()`);
3. a step label (`as()`); the last object with that label on the path.

The `by()` modulators form a ring over the distinct variables in order of first appearance: the
first `by()` applies to the first variable, the second to the second, and so on, cycling when there
are fewer `by()`s than variables. A variable used twice takes one slot. `_`'s `by()` runs on the
current traverser; any other variable's `by()` runs on the resolved object.

```rhai,graph=modern
// b appears first: b takes in("created").count(), a takes age
g.V().as("a").out("created").as("b").math("b + a").by(__.in("created").count()).by("age").toList()
// 32.0, 33.0, 35.0, 38.0
```

A traverser for which a `by()` produces nothing (`by("age")` on a vertex without `age`) is
filtered out, as in TinkerPop 3.6+.

Every variable must resolve to a number. A string, a list or a map is an error; convert a numeric
string with `as_number(GType.DOUBLE)` first.

## Nested access (Graphersal extension)

Graphersal extends the language with nested access after any variable, with no whitespace before
`.` or `[`. It is not part of exp4j or TinkerPop: exp4j rejects every such equation, so no
TinkerPop equation changes its meaning.

| Form | Reads |
|---|---|
| `_.key` | a key of a map (or a property of a vertex or edge) |
| `_["unit price"]`, `_['k']` | a key that is not an identifier (escapes `\\`, `\"`, `\'`) |
| `_[0]`, `_[-1]` | an element of a list; a negative index counts from the end |

The path is applied after the variable's `by()` projection and reads the value in place, without
materializing the element.

```rhai,graph=modern
g.V("1").elementMap().math("_.age * 2").toList()        // 58.0
g.V("1").valueMap().math("_.age[0] + 1").toList()       // 30.0  (valueMap() values are lists)
g.inject(#{price: #{total: 10, items: [1, 2.5, 4]}}).math("_.price.total * 2 + _.price.items[-1]").toList()
// 24.0
g.inject(#{"unit price": 3}).math("_[\"unit price\"] * 4").toList()  // 12.0
```

## Errors

An equation that does not parse fails before the traversal runs, even when no traverser reaches
the step. Every other problem fails the traversal for the traverser that hits it. The diagnostic
underlines the offending token:

```rhai
g.inject(1).math("2 pi")
// Error: math("2 pi") failed at column 3: missing operator before name 'pi'; implicit multiplication is not supported, write '*' explicitly
//   equation: math("2 pi")
//                     ^
```

| Problem | Example | Message |
|---|---|---|
| syntax | `math("2 +* 3")` | `unexpected '*'; expected a number, a variable, a function or '('` |
| unknown function | `math("sqr(_)")` | `unknown function 'sqr'; ...` (the help suggests `sqrt`) |
| unknown variable | `math("_ + x")` with no `x` anywhere | `variable 'x' is not '_', a key of the current map, a side effect or a step label` |
| bad path | `elementMap().math("_.agee")` | `_.agee leads to no value: no key 'agee'; the available keys are: id, label, name, age` |
| not a number | `values("name").math("_ + 1")` | `the variable _ for math() must resolve to a number, but it is a string` |
| null | a `null` value | `the variable _.a for math() must resolve to a number, but it is null` |
| division by zero | `math("_ / 0")` | `Division by zero!` (exp4j's message; also `%` and `cot`) |

## The current value is `_`

The current value is `_`, as in TinkerPop; there is no other name for it. `math("value + 1")` fails
with an unknown-variable error whose help shows the rewritten query. Results are doubles
(`math("2 + 2")` is `4.0`), and `^` is the power operator.
