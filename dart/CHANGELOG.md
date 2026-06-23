# Changelog

## 0.1.0

- Initial release: Dart/Flutter bindings for the exath-engine.
- One eval gateway: `evaluate`, `isValid`, `supportedFunctions`, and
  `ExathSession` with `eval` / `evalLine` (symbolic and matrix forms),
  variable and user-function management.
- Native backend via `dart:ffi` (C ABI); web backend via `js_interop` (WASM).
