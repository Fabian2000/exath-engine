# exath-engine

A complex-native mathematical expression DSL. Written in Rust, available as a native library (C/C++/C#), WebAssembly (JavaScript/TypeScript), and as a Rust crate.

---

## Design principles

- **Deterministic**: same expression always produces the same result; no randomness, no side effects
- **Safe**: no I/O, no code execution, no filesystem access; safe to embed in untrusted environments
- **Complex-native**: all expressions evaluate over ℂ; real results are a special case (imaginary part = 0)
- **Strict on domain errors**: operations like comparison (`>`, `<`, …) and rounding require real inputs and return errors for complex values; modulo likewise

---

## Workspace layout

```text
engine/
├── core/        exath-engine       Rust library (the evaluator itself)
├── ffi-c/       exath-engine-ffi   C-compatible shared library + header
└── ffi-wasm/    exath-engine-wasm  WebAssembly + JS bindings (wasm-pack)
```

---

## Features

- **Complex numbers**: every expression is evaluated over ℂ; real results are a special case
- **Session with variables**: stateful context, `a = 5` assigns, `a + 1` reads
- **User-defined functions**: `f(x) = x^2`, `g(x, y) = x*y + 1` stored in session, callable by name
- **Rich function set**: trig, inverse trig, hyperbolic, inverse hyperbolic, exp/log, rounding, complex parts, and more
- **Multi-argument functions**: `if(cond, a, b)`, `min(...)`, `max(...)`, `clamp(x, lo, hi)`, `gcd(a, b)`, `lcm(a, b)`
- **Comparison & logic operators**: `>`, `<`, `>=`, `<=`, `==`, `!=`, `&&`, `||`, `!`
- **One eval gateway**: every operation (numeric, symbolic, matrix, units) is
  invoked by evaluating a string: `evaluate(expr)` or `Session::eval` /
  `Session::eval_line`. The Rust crate, C-FFI and WASM expose exactly this
  gateway, so all three are identical
- **Computer algebra**: `diff`, `simplify`, `expand`, `factor`, `solve`,
  `integral`, `taylor`, `limit`, `laplace`, `dsolve`, … all as `eval_line` forms
- **AST access**: parse to an inspectable tree for tooling
- **Three targets**: Rust, C shared library (with auto-generated header), WebAssembly

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

## Which entry point?

Four ways to evaluate, chosen by *stateful?* and *numeric-only or also symbolic?*

| Function | State | Returns | Understands symbolic forms? |
| --- | --- | --- | --- |
| `evaluate(expr)` | stateless | real `f64` (errors if complex) | no |
| `evaluate_complex(expr)` | stateless | `CalcResult` (real or complex) | no |
| `Session::eval(line)` | stateful (vars + functions) | `CalcResult` | no |
| `Session::eval_line(line)` | stateful (vars + functions) | `LineResult` (value **or** expression) | yes |

- `evaluate` is just `evaluate_complex` that errors instead of returning a
  complex result; use it when you specifically want a real number.
- `eval_line` is a superset of `eval`: it runs the same lines and additionally
  understands `diff` / `factor` / `solve` / matrix / … forms, returning an
  expression string for symbolic results. Use it whenever you want CAS.

---

## Syntax reference

### Numbers

| Syntax | Example |
| --- | --- |
| Integer | `42` |
| Decimal (dot) | `3.14` |
| Scientific notation | `6.022e23` |

### Constants

| Name | Value |
| --- | --- |
| `pi` or `π` | π ≈ 3.14159… |
| `e` | e ≈ 2.71828… |
| `phi` or `φ` | φ ≈ 1.61803… (golden ratio) |
| `epsilon` or `ε` | Euler's number e (alias) |
| `i` | imaginary unit, i² = −1 |

### Operators

| Operator | Description |
| --- | --- |
| `+` `-` `*` `/` | Arithmetic |
| `^` or `**` | Power (right-associative) |
| `%` or `mod` | Modulo (real only) |
| `==` `!=` `<` `<=` `>` `>=` | Comparison → `1.0` or `0.0` (real only) |
| `&&` `\|\|` `!` | Logical AND / OR / NOT |
| `!` (postfix) | Factorial, e.g. `5!` = 120 (real only) |
| `\|x\|` | Absolute value / modulus, e.g. `\|-3\|` = 3 |
| `( … )` | Grouping |

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
| `if(cond, true_val, false_val)` | Conditional; only the chosen branch is evaluated |
| `piecewise(c1, v1, c2, v2, …, default)` | First true condition wins; e.g. `piecewise(x<0, -x, x)` = \|x\| |
| `min(a, b, ...)` | Minimum of any number of real arguments |
| `max(a, b, ...)` | Maximum of any number of real arguments |
| `clamp(x, lo, hi)` | Clamp x to the range [lo, hi] |
| `gcd(a, b)` | Greatest common divisor (integer arguments) |
| `lcm(a, b)` | Least common multiple (integer arguments) |

> `gcd` and `lcm` require arguments that are mathematically integral: `|x − round(x)| < 1e-9`.
> This tolerates typical floating-point rounding, e.g. `gcd(9.0, 6.0)` → 3.

#### Special functions

| Function | Description |
| --- | --- |
| `gamma(x)` | Gamma function Γ(x) (Lanczos) |
| `lgamma(x)` | Natural log of \|Γ(x)\| |
| `digamma(x)` | Digamma ψ(x) = Γ'(x)/Γ(x) |
| `beta(a, b)` | Beta B(a,b) = Γ(a)Γ(b)/Γ(a+b) |
| `erf(x)` `erfc(x)` | Error function and its complement |

#### Statistics & distributions

| Function | Description |
| --- | --- |
| `mean(a, b, …)` | Arithmetic mean |
| `median(a, b, …)` | Median |
| `variance(a, b, …)` | Population variance |
| `stddev(a, b, …)` | Population standard deviation |
| `npdf(x, mu, sigma)` | Normal probability density |
| `ncdf(x, mu, sigma)` | Normal cumulative distribution |
| `binom(n, k)` | Binomial coefficient (n choose k) |

#### Number theory

Integer arguments (within i64/i128 range).

| Function | Description |
| --- | --- |
| `isprime(n)` | 1 if n is prime, else 0 |
| `nextprime(n)` | Smallest prime greater than n |
| `totient(n)` | Euler's totient φ(n) |
| `powmod(a, b, m)` | Modular exponentiation aᵇ mod m |
| `factorint(n)` | Prime factorisation, e.g. `factorint(360)` → `2^3 * 3^2 * 5` |

---

## Session & variables

A `Session` holds a variable table and a function table that persist across `eval` calls.

**Variable assignment**: line of the form `identifier = expression`:

```text
x = 5
y = x^2 + 1    → 26
```

**Conditional assignment**:

```text
x = -3
result = if(x >= 0, sqrt(x), abs(x))   → 3
```

**Numeric vs symbolic**: `eval` returns a number; `eval_line` additionally
understands symbolic forms (`diff`, `factor`, `solve`, …) and returns an
expression string for them. A symbolic result can be bound to a name and reused:

```text
g = diff(x^2, x)   → 2 * x        (symbolic variable)
g + 1              → 2 * x + 1
```

**Introspection**: `is_valid(expr)` returns whether an expression parses;
`supported_functions()` lists every built-in name.

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

**Rust API**: function definitions go through the same `eval` call:

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

---

## Numeric & data forms

`eval` / `eval_line` forms that return a number:

| Form | Description |
| --- | --- |
| `sum(expr, var, a, b)` | Σ expr for var = a…b (integer steps) |
| `product(expr, var, a, b)` | Π expr for var = a…b |
| `deriv(expr, var, x0)` | Numeric derivative at x0 (central difference) |
| `integral(expr, var, a, b)` | Definite integral (exact if possible, else adaptive Simpson) |
| `convert(value, from, to)` | Unit conversion, units as names |

```text
sum(k^2, k, 1, 5)           → 55
product(k, k, 1, 5)         → 120
deriv(x^3, x, 2)            → 12
integral(sin(x), x, 0, pi)  → 2
convert(5, km, m)           → 5000
```

Range limit for `sum`/`product`: 10,000,000 terms.

---

## Linear algebra

Matrix literals are `[[…], […]]` (rows); commas separate elements.

| Form | Description |
| --- | --- |
| `det(M)` | Determinant |
| `inv(M)` | Inverse |
| `transpose(M)` | Transpose |
| `trace(M)` | Trace |
| `rank(M)` | Rank |
| `norm(M)` | Frobenius norm |
| `identity(n)` | n×n identity matrix |
| `linsolve(A, b)` | Solve A·x = b |
| `eigenvalues(M)` | Eigenvalues (symmetric → Jacobi) |
| `eigenvectors(M)` | Eigenvectors (symmetric → orthonormal) |
| `svdvals(M)` | Singular values (descending) |
| `charpoly(M, x)` | Characteristic polynomial in `x` |

```text
det([[1,2],[3,4]])              → -2
inv([[2,0],[0,2]])              → [[0.5, 0], [0, 0.5]]
linsolve([[2,0],[0,2]], [4,6])  → [[2], [3]]
eigenvalues([[2,1],[1,2]])      → 1, 3
```

---

## Computer algebra

Symbolic operations are `eval_line` forms returning an expression string.

| Form | Description |
| --- | --- |
| `diff(expr, var)` | Derivative |
| `simplify(expr)` | Canonical simplification + identities |
| `expand(expr)` | Expand products and powers |
| `factor(expr, var)` | Factor a polynomial |
| `polygcd(p, q, var)` | Polynomial GCD |
| `solve(eq, var)` | Solve (exact for polynomials, verified numeric for transcendental) |
| `nsolve(expr, var, x0)` | Newton's method from initial guess x0 |
| `integral(expr, var)` | Indefinite integral (rules + verified u-substitution + partial fractions) |
| `taylor(expr, var, x0, order)` | Taylor polynomial about x0 |
| `limit(expr, var, x0)` | Limit (L'Hôpital, including ±∞) |
| `sumc(expr, k, n)` | Closed-form Σ (Faulhaber) as a polynomial in n |
| `laplace(expr, t, s)` | Laplace transform |
| `dsolve([a_n, …, a_0], var)` | Linear constant-coefficient ODE |
| `assume(x > 0)` | Sign assumption consulted by `simplify` |

```text
diff(sin(x^2), x)          → 2 * x * cos(x^2)
factor(x^2 - 1, x)         → (x + 1) * (x - 1)
solve(x^2 - 4, x)          → x = 2, x = -2
integral(2*x*cos(x^2), x)  → sin(x^2)
limit(sin(x)/x, x, 0)      → 1
dsolve([1,3,2], x)         → C1 * exp(-2*x) + C2 * exp(-x)
assume(x > 0); simplify(sqrt(x^2))  → x
```

The simplifier produces a canonical polynomial normal form (collects like terms, expands products, merges powers, folds numeric constants like `sin(0)`, `cos(pi)`, `4!`) and applies identities: Pythagorean `sin²+cos²=1`, hyperbolic `cosh²−sinh²=1`, reciprocal-trig canonicalisation, inverse pairs `exp(ln x)=x`, exp laws, and surds. Every integration / solving result is verified internally, so a returned answer is always correct.

---

## Multivariable calculus

| Form | Description |
| --- | --- |
| `grad(expr, [x, y, …])` | Gradient (column vector of partials) |
| `jacobian([f, g, …], [x, y, …])` | Jacobian matrix |
| `hessian(expr, [x, y, …])` | Hessian matrix |
| `odesolve(f, x, y, x0, y0, x1)` | Numeric ODE y' = f(x,y) via RK4, returns y(x1) |
| `minimize(f, x, a, b)` / `maximize(f, x, a, b)` | 1-D optimisation on [a, b] (golden section) |

```text
grad(x^2 + y^2, [x, y])      → [[2 * x], [2 * y]]
odesolve(y, x, y, 0, 1, 1)   → 2.718…   (y' = y, y(0) = 1)
minimize((x - 3)^2, x, 0, 10) → 3
```

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
- **Numeric range forms** are O(n) in interval/range size: `deriv` uses 2 evaluations, definite `integral` uses an adaptive Simpson rule, `sum`/`product` evaluate once per integer step
- **No global state**: each `Session` is an independent value; safe to use concurrently from multiple threads as long as each thread owns its own `Session`

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

## Numerical accuracy & exactness

exath is an embeddable, multi-language engine, so its external number type is
`f64`/`double` (identical across Rust/C/WASM/...). What that means in practice:

- **Exact where it counts, f64 elsewhere.** Symbolic coefficients use exact
  rationals (`i128`-based); `1/3 + 1/3 = 2/3`, `∫x² dx = x³/3`. If a rational
  numerator/denominator would overflow `i128`, it degrades gracefully to `f64`
  (loses exactness, never panics). There is no arbitrary precision (BigInt): a
  deliberate trade-off for the universal cross-language `f64` contract.
- **Symbolic decisions use tolerances.** Constant folding, zero-tests and root
  detection compare with small tolerances (~1e-9..1e-12). The differential test
  suite (`tests/verification.rs`) checks that `simplify`/`factor` preserve value
  at sample points; still, treat results near singularities with care.
- **Numerical linear algebra.** Symmetric eigenproblems use the **Jacobi**
  algorithm (accurate eigenvalues + orthonormal eigenvectors). Singular values
  use **one-sided Jacobi SVD** (does not form `MᵀM`, so conditioning is
  preserved). Non-symmetric eigenvalues fall back to the characteristic
  polynomial + numeric roots (fine for small matrices; less robust for large or
  ill-conditioned ones).
- **Commas are separators:** the comma always separates arguments/elements; decimals use `.` only (`max(1, 2)`, `[1, 2, 3]`, `3.14`).
- **Panic-free:** every operation returns `Result`; fuzzing 30k random inputs
  never panics.

---

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
