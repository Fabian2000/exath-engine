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

## Numerical methods

```js
import { deriv_at, integrate_range, sum_range, prod_range } from "./pkg/exath_engine_wasm.js";

// Derivative: f'(x) at x=2 for f(x) = x^3
deriv_at("x^3", "x", 2.0, "rad").re;  // 12

// Integral: ∫₀^π sin(x) dx
integrate_range("sin(x)", "x", 0, Math.PI, "rad").re;  // 2

// Sum: Σ k² for k=1..5
sum_range("k^2", "k", 1, 5, "rad").re;  // 55

// Product: Π k for k=1..5
prod_range("k", "k", 1, 5, "rad").re;  // 120
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
| `evaluate(expr, angleMode)` | Evaluate an expression |
| `isValid(expr)` | Check if expression parses |
| `supportedFunctions()` | Array of built-in function names |
| `deriv_at(expr, var, x, mode)` | Numerical derivative |
| `integrate_range(expr, var, a, b, mode)` | Definite integral |
| `sum_range(expr, var, from, to, mode)` | Summation |
| `prod_range(expr, var, from, to, mode)` | Product |

### ExathSession

| Method | Description |
| --- | --- |
| `new ExathSession(angleMode)` | Create session (`"rad"`, `"deg"`, `"grad"`) |
| `.eval(line)` | Evaluate expression or assignment |
| `.setVar(name, re, im)` | Set variable (im=0 for real) |
| `.removeVar(name)` | Remove a variable |
| `.clearVars()` | Clear all variables |
| `.varNames()` | Array of variable names |
| `.fnNames()` | Array of user-defined function names |
| `.removeFn(name)` | Remove a user-defined function |

### Angle mode

Pass as string: `"rad"` (default), `"deg"`, or `"grad"`. Case-insensitive.
