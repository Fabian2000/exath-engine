# exath

Dart and Flutter bindings for the [exath-engine](https://github.com/Fabian2000/exath-engine):
a complex-native math expression evaluator with computer algebra (differentiation,
factoring, solving, integration, limits, Laplace, ODEs, linear algebra, ...).

Everything goes through one eval gateway, identical to the Rust crate and the
C / WASM builds: `evaluate` for one-shot numeric results, and `ExathSession`
for stateful evaluation including the symbolic / matrix DSL via `evalLine`.

## Usage

```dart
import 'package:exath/exath.dart';

void main() {
  // One-shot numeric
  print(evaluate('2^10 + sqrt(9)'));   // 1027.0
  print(evaluate('sqrt(-4)'));         // 0.0 + 2.0i

  final s = ExathSession();
  s.eval('r = 3');
  print(s.eval('pi * r^2').re);        // 28.27...

  // Symbolic (computer algebra) via evalLine
  print(s.evalLine('diff(sin(x^2), x)'));      // 2 * x * cos(x^2)
  print(s.evalLine('factor(x^2 - 1, x)'));     // (x + 1) * (x - 1)
  print(s.evalLine('solve(x^2 - 4, x)'));      // x = 2, x = -2
  print(s.evalLine('integral(x^2, x)'));       // x^3 / 3

  s.dispose(); // free the native session (no-op on web)
}
```

`evalLine` returns a sealed `LineResult`: a `NumberResult` (numeric) or an
`ExpressionResult` (symbolic string). `eval` always returns a numeric
`ExathResult`. See `example/` for more.

## Platform setup

### Native (Dart VM, Flutter desktop): no Rust needed

Download the prebuilt library for your platform (from the engine's GitHub
release) once:

```bash
dart run exath:download
```

That fetches `libexath_engine_ffi.{so,dylib,dll}` into a per-user cache, where
the package finds it automatically. **No Rust toolchain required.**

The loader resolves the library in this order:
1. the `EXATH_LIB` environment variable (explicit path),
2. the cache populated by `dart run exath:download`,
3. the platform-default name on the system library path / next to the executable.

If you prefer to build it yourself: `cargo build --release -p exath-engine-ffi`.

(Mobile, i.e. Android/iOS, bundles the prebuilt library via the Flutter plugin
layer rather than downloading at runtime.)

### Web (Flutter web)

The package binds the WASM build via `js_interop`, expecting the wasm-bindgen
module exposed on the JS global as `exath`. Build it and wire it up once:

```bash
cd ffi-wasm && wasm-pack build --target web
```

```html
<script type="module">
  import init, * as exath from './pkg/exath_engine_wasm.js';
  await init();
  globalThis.exath = exath;   // the Dart bindings look here
</script>
```

## API

| Dart | Description |
| --- | --- |
| `evaluate(expr, {angleMode})` | One-shot numeric, returns `ExathResult` |
| `isValid(expr)` | Whether an expression parses |
| `supportedFunctions()` | Names of all built-ins |
| `ExathSession().eval(line)` | Stateful numeric, returns `ExathResult` |
| `ExathSession().evalLine(line)` | Numeric **or** symbolic, returns `LineResult` |
| `setVar` / `removeVar` / `clearVars` / `varNames` | Variable management |
| `removeFn` / `fnNames` | User-defined function management |
| `dispose()` | Free the native session |

For the full DSL form reference (every `diff` / `factor` / `solve` / matrix /
... form), see the [engine README](https://github.com/Fabian2000/exath-engine).

## License

Licensed under either of Apache License 2.0 (`LICENSE-APACHE`) or MIT
(`LICENSE-MIT`) at your option.
