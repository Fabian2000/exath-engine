# Changelog

## 1.0.0

- First release of `exath_engine`: a Flutter plugin for the
  [exath-engine](https://github.com/Fabian2000/exath-engine), a complex-native
  math evaluator with a computer-algebra core. Prebuilt native libraries are
  bundled for Android, iOS, macOS, Windows, and Linux, and the WASM module is
  bundled for web. Call `await ensureInitialized()` once at startup (loads the
  WASM module on web, no-op elsewhere).
