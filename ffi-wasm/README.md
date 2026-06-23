# exath-engine — JavaScript / TypeScript

WebAssembly bindings for the exath-engine expression evaluator.

## Build

```bash
cd ffi-wasm

# For browser (ESM)
wasm-pack build --target web

# For Node.js (CommonJS)
wasm-pack build --target nodejs

# For bundlers (webpack, vite, etc.)
wasm-pack build --target bundler
```

Output goes to `ffi-wasm/pkg/` with `.js`, `.wasm`, `.d.ts`, and `package.json`.

## Quick start (Node.js)

```js
import { evaluate, isValid, ExathSession } from "./pkg/exath_engine_wasm.js";

// One-shot evaluation
const r = evaluate("2^10 + sqrt(9)", "rad");
console.log(r.re);  // 1027

// Complex result
const c = evaluate("sqrt(-4)", "rad");
console.log(c.re, c.im, c.isComplex);  // 0, 2, true

// Error handling
const e = evaluate("ln(0)", "rad");
if (e.isError) console.log(e.errorMessage);  // "ln undefined for 0"

// Validation
isValid("2+3");   // true
isValid("2++");   // false
```

## Quick start (Browser)

```html
<script type="module">
import init, { evaluate, ExathSession } from "./pkg/exath_engine_wasm.js";

await init();

const r = evaluate("pi * 5^2", "rad");
console.log(r.re);  // 78.5398...
</script>
```

Browser builds require calling `init()` before any other function. Node.js builds do not.

## Session

```js
const s = new ExathSession("deg");

// Variables
s.eval("r = 5");
s.eval("h = 10");
const vol = s.eval("pi * r^2 * h");
console.log(vol.re);  // 785.398...

// User-defined functions
s.eval("hyp(a, b) = sqrt(a^2 + b^2)");
s.eval("circle(r) = pi * r^2");
console.log(s.eval("hyp(3, 4)").re);    // 5
console.log(s.eval("circle(10)").re);    // 314.159...
console.log(s.fnNames());               // ["hyp", "circle"]

// Manage state
s.setVar("x", 42.0, 0.0);   // real value
s.setVar("z", 3.0, 4.0);    // complex value (3+4i)
console.log(s.varNames());   // ["r", "h", "x", "z"]
s.removeVar("x");
s.removeFn("hyp");
s.clearVars();
```

## Numeric forms (via evalLine)

Numeric range forms and unit conversion are `evalLine` DSL forms — read `.re`:

```js
const s = new ExathSession("rad");
s.evalLine("deriv(x^3, x, 2)").re;          // 12   (numeric derivative)
s.evalLine("integral(sin(x), x, 0, pi)").re;// 2    (definite integral)
s.evalLine("sum(k^2, k, 1, 5)").re;         // 55
s.evalLine("product(k, k, 1, 5)").re;       // 120
s.evalLine("convert(5, km, m)").re;         // 5000
```

## API reference

### ExathResult

Every function returns an `ExathResult` object:

| Property | Type | Description |
| --- | --- | --- |
| `.re` | `number` | Real part |
| `.im` | `number` | Imaginary part |
| `.isComplex` | `boolean` | True if result has nonzero imaginary part |
| `.isError` | `boolean` | True if evaluation failed |
| `.errorMessage` | `string?` | Error description (undefined if no error) |

### Functions

| Function | Description |
| --- | --- |
| `evaluate(expr, angleMode)` | Evaluate an expression (numeric, returns `ExathResult`) |
| `isValid(expr)` | Check if expression parses |
| `supportedFunctions()` | Array of built-in function names |

Everything else — symbolic, numeric range forms, matrix, units — goes through
`ExathSession.evalLine` (see below). There are no per-operation functions.

### ExathSession

| Method | Description |
| --- | --- |
| `new ExathSession(angleMode)` | Create session (`"rad"`, `"deg"`, `"grad"`) |
| `.eval(line)` | Evaluate a line (numeric, returns `ExathResult`) |
| `.evalLine(line)` | Evaluate a line incl. symbolic/matrix forms (returns `ExathLine`) |
| `.setVar(name, re, im)` | Set variable (im=0 for real) |
| `.removeVar(name)` | Remove a variable |
| `.clearVars()` | Clear all variables |
| `.varNames()` | Array of variable names |
| `.fnNames()` | Array of user-defined function names |
| `.removeFn(name)` | Remove a user-defined function |

### Angle mode

Pass as string: `"rad"` (default), `"deg"`, or `"grad"`. Case-insensitive.

## The eval gateway — symbolic, matrix, numeric

All symbolic and matrix operations go through `evalLine`. Symbolic results have
`.isExpression === true` (read `.expression`); numeric results give `.re`/`.im`.

```js
const s = new ExathSession("rad");

// Computer algebra
s.evalLine("diff(sin(x^2), x)").expression;   // "2 * x * cos(x^2)"
s.evalLine("simplify(x + 0 + 1*x)").expression; // "2 * x"
s.evalLine("factor(x^2 - 1, x)").expression;  // "(x + 1) * (x - 1)"
s.evalLine("solve(x^2 - 4, x)").expression;   // "x = 2, x = -2"
s.evalLine("integral(x^2, x)").expression;    // "x^3 / 3"
s.evalLine("limit(sin(x)/x, x, 0)").expression; // "1"
s.evalLine("dsolve([1,3,2], x)").expression;  // "C1 * exp(-2*x) + C2 * exp(-x)"

// Linear algebra
s.evalLine("det([[1,2],[3,4]])").re;          // -2
s.evalLine("eigenvalues([[2,1],[1,2]])").expression; // "1, 3"

// Symbolic variables: bind a result, then substitute numerically
s.evalLine("g = diff(x^3, x)");               // g = 3 * x^2
s.evalLine("x = 2");
s.evalLine("g").re;                            // 12
```

`evalLine(line)` returns an `ExathLine`: read `.expression` when
`.isExpression` is true, otherwise `.re`/`.im` (or `.isError`/`.errorMessage`).
This surface is identical to the Rust crate and the C-FFI. See the main
[README](../README.md) for the full DSL reference.
