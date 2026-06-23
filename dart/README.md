# exath_engine

Flutter plugin for the [exath-engine](https://github.com/Fabian2000/exath-engine):
a complex-native math expression evaluator with computer algebra (differentiation,
factoring, solving, integration, limits, Laplace, ODEs, linear algebra, ...).
Prebuilt native libraries are bundled for every platform: no Rust toolchain, no
manual setup.

One eval gateway, identical to the Rust crate and the C / WASM builds:
`evaluate` for one-shot numeric results, and `ExathSession` for stateful
evaluation including the symbolic / matrix DSL via `evalLine`.

## Install

```bash
flutter pub add exath_engine
```

Supported platforms: **Android, iOS, macOS, Windows, Linux, and Web.** The
native library is bundled per platform (jniLibs / xcframework / dylib / dll /
so); the web build ships the WASM module. Nothing to configure.

## Usage

Call `await ensureInitialized()` once before using the API. It loads the WASM
module on web and is a harmless no-op everywhere else, so the same code runs on
all platforms:

```dart
import 'package:exath_engine/exath_engine.dart';

Future<void> main() async {
  await ensureInitialized();           // loads WASM on web; no-op elsewhere

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

  s.dispose();
}
```

`evalLine` returns a sealed `LineResult`: a `NumberResult` (numeric) or an
`ExpressionResult` (symbolic string). `eval` always returns a numeric
`ExathResult`. See `example/` for a full Flutter app.

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
