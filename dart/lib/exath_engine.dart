/// Dart and Flutter bindings for the exath-engine.
///
/// One eval gateway, identical to the Rust crate and the C / WASM builds:
/// [evaluate] for one-shot numeric results, and [ExathSession] for stateful
/// evaluation including the symbolic / matrix DSL via [ExathSession.evalLine].
///
/// ```dart
/// final s = ExathSession();
/// print(s.eval('2 + 3 * 4').re);            // 14.0
/// print(s.evalLine('diff(x^2, x)'));        // 2 * x
/// print(s.evalLine('factor(x^2 - 1, x)'));  // (x + 1) * (x - 1)
/// s.dispose();
/// ```
///
/// On native platforms this binds the C library (`libexath_engine_ffi`) via
/// `dart:ffi`; on the web it binds the WASM build via `js_interop`. See the
/// package README for how to provide each.
library;

export 'src/types.dart';
export 'src/exath_stub.dart'
    if (dart.library.ffi) 'src/exath_native.dart'
    if (dart.library.js_interop) 'src/exath_web.dart';
