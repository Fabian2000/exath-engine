# Changelog

## 1.0.1

- Fix (web): the WASM module failed to load with "Failed to resolve module
  specifier" because the import used a bare specifier. Now loaded via a dynamic
  import() against an absolute URL (document.baseURI), so it works under any
  base href.

## 1.0.0

- First release of `exath_engine`: a Flutter plugin for the
  [exath-engine](https://github.com/Fabian2000/exath-engine), a complex-native
  math evaluator with a computer-algebra core. Prebuilt native libraries are
  bundled for Android, iOS, macOS, Windows, and Linux, and the WASM module is
  bundled for web. Call `await ensureInitialized()` once at startup (loads the
  WASM module on web, no-op elsewhere).
