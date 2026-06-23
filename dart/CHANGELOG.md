# Changelog

## 1.0.2

- Fix: the WASM assets were missing from the published package (wasm-pack ships
  a `.gitignore` with `*` that pub honoured per-directory, dropping assets/wasm).
  Web and any consumer build now resolve the bundled WASM correctly.

## 1.0.1

- First working release (1.0.0 was retracted before use). `exath` is now a
  Flutter plugin: prebuilt native libraries bundled for Android, iOS, macOS,
  Windows, Linux, and the WASM module for web. No Rust toolchain, no manual
  setup. Call `await ensureInitialized()` once at startup (loads WASM on web,
  no-op elsewhere).

## 1.0.0

- Initial release: Dart/Flutter bindings for the exath-engine.
- One eval gateway: `evaluate`, `isValid`, `supportedFunctions`, and
  `ExathSession` with `eval` / `evalLine` (symbolic and matrix forms),
  variable and user-function management.
- Native backend via `dart:ffi` (C ABI); web backend via `js_interop` (WASM).
