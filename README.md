# exath-engine

A complex-native mathematical expression DSL. Written in Rust, available as a native library (C/C++/C#), WebAssembly (JavaScript/TypeScript), and as a Rust crate.

---

## Design principles

- **Deterministic** — same expression always produces the same result; no randomness, no side effects
- **Safe** — no I/O, no code execution, no filesystem access; safe to embed in untrusted environments
- **Complex-native** — all expressions evaluate over ℂ; real results are a special case (imaginary part = 0)
- **Strict on domain errors** — operations like comparison (`>`, `<`, …) and rounding require real inputs and return errors for complex values; modulo likewise

---

## Workspace layout

```text
engine/
├── core/        exath-engine       — Rust library (the evaluator itself)
├── ffi-c/       exath-engine-ffi   — C-compatible shared library + header
└── ffi-wasm/    exath-engine-wasm  — WebAssembly + JS bindings (wasm-pack)
```

---

## Features

- **Complex numbers** — every expression is evaluated over ℂ; real results are a special case
- **Session with variables** — stateful context, `a = 5` assigns, `a + 1` reads
- **User-defined functions** — `f(x) = x^2`, `g(x, y) = x*y + 1` stored in session, callable by name
- **Rich function set** — trig, inverse trig, hyperbolic, inverse hyperbolic, exp/log, rounding, complex parts, and more
- **Multi-argument functions** — `if(cond, a, b)`, `min(...)`, `max(...)`, `clamp(x, lo, hi)`, `gcd(a, b)`, `lcm(a, b)`
- **Comparison & logic operators** — `>`, `<`, `>=`, `<=`, `==`, `!=`, `&&`, `||`, `!`
- **Numerical methods** — derivative, integral, sum, product
- **AST access** — parse to an inspectable tree for tooling
- **Three targets** — Rust, C shared library (with auto-generated header), WebAssembly

---

## Quick start

### Rust

```toml
[dependencies]
exath-engine = { path = "engine/core" }
```

```rust
use exath_engine::{evaluate_complex, Session, AngleMode};

// One-shot evaluation
let result = evaluate_complex("sqrt(-4) + 2*pi", AngleMode::Rad)?;

// Stateful session with variables
let mut s = Session::new(AngleMode::Rad);
s.eval("r = 3")?;
s.eval("h = 4")?;
let vol = s.eval("pi * r^2 * h")?;   // CalcResult::Real(113.097...)

// User-defined functions
s.eval("circle_area(r) = pi * r^2")?;
s.eval("hyp(a, b) = sqrt(a^2 + b^2)")?;
let area = s.eval("circle_area(5)")?;  // CalcResult::Real(78.539...)
let c    = s.eval("hyp(3, 4)")?;       // CalcResult::Real(5.0)
```

### C / C++ / C#

```c
#include "exath_engine.h"

ExathSession *s = exath_session_new(Rad);
exath_session_eval(s, "f(x) = x^2 + 1");
ExathResult r = exath_session_eval(s, "f(5)");
printf("%.6f\n", r.re);               // 26.000000
exath_session_free(s);
```

Build the shared library:

```bash
cargo build --release -p exath-engine-ffi
# → target/release/libexath_engine_ffi.so  (Linux)
# → target/release/exath_engine_ffi.dll    (Windows)
```

The C header is at `ffi-c/include/exath_engine.h`.

### JavaScript / TypeScript (WebAssembly)

```bash
cd ffi-wasm
wasm-pack build --target web
```

```js
import init, { ExathSession, evaluate, isValid } from "./pkg/exath_engine_wasm.js";
await init();

// One-shot
const r = evaluate("sqrt(-4)", "rad");
console.log(r.re, r.im);              // 0, 2

// Session with user-defined functions
const s = new ExathSession("rad");
s.eval("f(x) = x^2 + 1");
s.eval("g(x, y) = x * y + 3");
console.log(s.eval("f(5)").re);       // 26
console.log(s.eval("g(2, 4)").re);    // 11
console.log(s.fnNames());             // ["f", "g"]
```

---

## Syntax reference

### Numbers

| Syntax | Example |
| --- | --- |
| Integer | `42` |
| Decimal (dot or comma) | `3.14` or `3,14` |
| Scientific notation | `6.022e23` |

### Constants

| Name | Value |
| --- | --- |
| `pi` or `π` | π ≈ 3.14159… |
| `e` | e ≈ 2.71828… |
| `phi` or `φ` | φ ≈ 1.61803… |

### Operators

| Operator | Description |
| --- | --- |
| `+` `-` `*` `/` | Arithmetic |
| `^` or `**` | Power (right-associative) |
| `%` or `mod` | Modulo (real only) |
| `==` `!=` `<` `<=` `>` `>=` | Comparison → `1.0` or `0.0` (real only) |
| `&&` `\|\|` `!` | Logical AND / OR / NOT |

> **Note on complex numbers and comparisons:** Ordering operators (`<`, `<=`, `>`, `>=`) are only defined for real numbers and return an error for complex values. `==` / `!=` compare both real and imaginary parts within a tolerance of 1e-12.

Implicit multiplication is supported: `2pi`, `3(x+1)`, `2sqrt(x)`.

### Functions

#### Trigonometric

| Function | Description |
| --- | --- |
| `sin(x)` `cos(x)` `tan(x)` `cot(x)` | Basic trig (angle mode applies) |
| `sec(x)` `csc(x)` | Secant, cosecant |
| `asin(x)` `acos(x)` `atan(x)` `acot(x)` | Inverse trig |
| `asec(x)` `acsc(x)` | Inverse secant, cosecant |

#### Hyperbolic

| Function | Description |
| --- | --- |
| `sinh(x)` `cosh(x)` `tanh(x)` `coth(x)` | Hyperbolic functions |
| `sech(x)` `csch(x)` | Hyperbolic secant, cosecant |
| `asinh(x)` `acosh(x)` `atanh(x)` `acoth(x)` | Inverse hyperbolic |
| `asech(x)` `acsch(x)` | Inverse hyperbolic secant, cosecant |

#### Exponential & Logarithmic

| Function | Description |
| --- | --- |
| `exp(x)` | eˣ |
| `ln(x)` | Natural logarithm |
| `lg(x)` / `log(x)` | Base-10 logarithm |
| `log:b(x)` | Logarithm with base b, e.g. `log:2(8)` = 3 |

#### Roots

| Function | Description |
| --- | --- |
| `sqrt(x)` or `√x` | Square root (complex for negative reals) |
| `cbrt(x)` | Cube root |

#### Complex number functions

| Function | Description |
| --- | --- |
| `abs(x)` or `\|x\|` | Absolute value / modulus |
| `arg(z)` | Phase angle (argument) of a complex number |
| `conj(z)` | Complex conjugate |
| `real(z)` | Real part |
| `imag(z)` | Imaginary part |

#### Rounding

| Function | Description |
| --- | --- |
| `floor(x)` | Round down (real only) |
| `ceil(x)` | Round up (real only) |
| `round(x)` | Round to nearest, 0.5 → 1 (real only) |
| `trunc(x)` | Truncate toward zero (real only) |
| `frac(x)` | Fractional part (real only) |

#### Other

| Function | Description |
| --- | --- |
| `sign(x)` / `sgn(x)` | Signum: -1, 0, or 1 (real only) |
| `deg(x)` | Convert radians to degrees |
| `rad(x)` | Convert degrees to radians |

#### Multi-argument functions

| Function | Description |
| --- | --- |
| `if(cond, true_val, false_val)` | Conditional — only the chosen branch is evaluated |
| `min(a, b, ...)` | Minimum of any number of real arguments |
| `max(a, b, ...)` | Maximum of any number of real arguments |
| `clamp(x, lo, hi)` | Clamp x to the range [lo, hi] |
| `gcd(a, b)` | Greatest common divisor (integer arguments) |
| `lcm(a, b)` | Least common multiple (integer arguments) |

> `gcd` and `lcm` require arguments that are mathematically integral: `|x − round(x)| < 1e-9`.
> This tolerates typical floating-point rounding, e.g. `gcd(9.0, 6.0)` → 3.

---

## Session & variables

A `Session` holds a variable table and a function table that persist across `eval` calls.

**Variable assignment** — line of the form `identifier = expression`:

```text
x = 5
y = x^2 + 1    → 26
```

**Conditional assignment**:

```text
x = -3
result = if(x >= 0, sqrt(x), abs(x))   → 3
```

**C API**:

```c
ExathSession *s = exath_session_new(Rad);
exath_session_set_var(s, "x", 5.0, 0.0);     // set programmatically
ExathResult r = exath_session_eval(s, "x^2");
exath_session_free(s);
```

**JS API**:

```js
const s = new ExathSession("rad");
s.setVar("x", 5.0, 0.0);
const r = s.eval("x^2");
console.log(r.re);    // 25
console.log(s.varNames());
```

---

## User-defined functions

A session line of the form `name(params) = body` defines a reusable function. Parameters shadow outer variables for the duration of the call.

```text
f(x) = x^2 + 1
f(5)              → 26
f(sqrt(9))        → 10

g(x, y) = x * y + 3
g(2, 4)           → 11

norm(a, b, c) = sqrt(a^2 + b^2 + c^2)
norm(1, 2, 2)     → 3

fib_approx(n) = round(phi^n / sqrt(5))
fib_approx(10)    → 55
```

Functions can reference session variables and call built-in functions. Recursion is **not** supported (the body is evaluated at call time with no call stack).

**Rust API** — function definitions go through the same `eval` call:

```rust
let mut s = Session::new(AngleMode::Rad);
s.eval("f(x) = x^2 + 1")?;
let r = s.eval("f(4)")?;   // CalcResult::Real(17.0)
println!("{:?}", s.fn_names()); // ["f"]
s.remove_fn("f");
```

**JS API**:

```js
s.eval("f(x) = x^2 + 1");
console.log(s.eval("f(4)").re);  // 17
console.log(s.fnNames());         // ["f"]
s.removeFn("f");
```

---

## Numerical methods

These operate on single-variable real-valued expressions.

### Derivative

```rust
// f'(x) at x=1.0 using central finite difference
// Step size h = max(|x| * 1e-7, 1e-10) for relative scaling
let d = deriv("x^3 + 2*x", "x", 1.0, AngleMode::Rad)?;  // → 5.0
```

### Integral

```rust
// ∫₀^π sin(x) dx  (composite Simpson's rule, n=1000 fixed intervals)
let i = integrate("sin(x)", "x", 0.0, std::f64::consts::PI, AngleMode::Rad)?;  // → 2.0
```

The step count (n=1000) is fixed; for high-accuracy work on rapidly oscillating functions consider subdividing the interval manually.

### Sum / Product

```rust
// Σ k² for k = 1..5
let s = sum("k^2", "k", 1, 5, AngleMode::Rad)?;   // → 55.0

// Π k for k = 1..5  (= 5!)
let p = prod("k", "k", 1, 5, AngleMode::Rad)?;    // → 120.0
```

Maximum range: 10,000,000 terms.

---

## AST access

Parse an expression into an inspectable tree:

```rust
use exath_engine::parse_str;

let ast = parse_str("x^2 + 1")?;
println!("{:#?}", ast);
// BinOp(
//     Add,
//     BinOp(
//         Pow,
//         Var("x"),
//         Number(2.0),
//     ),
//     Number(1.0),
// )
```

The `Ast` and `BinOp` enums are fully public and can be traversed for analysis, pretty-printing, or symbolic manipulation.

---

## Building

```bash
# All crates
cargo build --release

# C shared library only
cargo build --release -p exath-engine-ffi

# WebAssembly (requires wasm-pack)
cd ffi-wasm
wasm-pack build --target web
```

---

## Angle modes

All trigonometric functions and their inverses respect the angle mode:

| Mode | `AngleMode::Rad` | `AngleMode::Deg` | `AngleMode::Grad` |
| --- | --- | --- | --- |
| `sin(90)` | −0.8011… | 1.0 | 0.9877… |
| `asin(1)` | 1.5707… (π/2) | 90.0 | 100.0 |

---

## Floating-point semantics

The engine uses `f64` (IEEE 754 double precision) internally. Two tolerance constants apply:

| Context | Tolerance | Used for |
| --- | --- | --- |
| Equality `==` / `!=` | 1e-12 | Comparing complex values |
| Integer check | 1e-9 | `gcd`, `lcm` argument validation |
| Real check | 1e-12 | Deciding whether a result is real or complex |

These values are not configurable per-call; they are conservative defaults suitable for hand-entered expressions.

---

## Performance

Designed for deterministic evaluation, not symbolic algebra or high-throughput batching.

- **Parsing** is O(n) in expression length (single-pass tokenizer + recursive descent parser)
- **Evaluation** is O(depth of AST) for expression evaluation
- **Numerical methods** are O(n) in interval/range size: `deriv` uses 2 evaluations, `integrate` uses 1002 evaluations (fixed n=1000 Simpson), `sum`/`prod` evaluate once per integer step
- **No global state** — each `Session` is an independent value; safe to use concurrently from multiple threads as long as each thread owns its own `Session`

---

## Error handling

All functions return `Result<_, ExathError>`. `ExathError` carries a `kind: ErrorKind` for programmatic branching and a `message: String` for display:

```rust
use exath_engine::{ExathError, ErrorKind};

match s.eval("ln(0)") {
    Err(e) if e.kind == ErrorKind::DomainError => println!("math error: {}", e),
    Err(e) => println!("other error: {}", e),
    Ok(r)  => println!("{:?}", r),
}
```

Error categories:

| `ErrorKind` | When |
| --- | --- |
| `ParseError` | Invalid syntax, unexpected token |
| `UndefinedName` | Unknown variable or function name |
| `ArgumentCount` | Wrong number of arguments |
| `ArgumentType` | Complex value where real is required |
| `DomainError` | `ln(0)`, division by zero, `0^x` for x≤0 |
| `Overflow` | `gcd`/`lcm` arguments too large for i64 |
| `ComplexResult` | Numerical method produced a complex intermediate |
| `RangeTooLarge` | `sum`/`prod` range exceeds 10,000,000 terms |

`ExathError` implements `std::error::Error` and `Display`.

At the C and JavaScript boundaries, errors are stringified: check `result.is_error == 1` / `result.isError` and read `result.error_msg` / `result.errorMessage`.
