import 'package:flutter_web_plugins/flutter_web_plugins.dart';

/// Web platform registration for the exath plugin.
///
/// The API itself is exposed through direct JS interop (see `exath_web.dart`);
/// this class exists so Flutter recognises the web implementation. The
/// wasm-bindgen module is loaded from the bundled web assets.
class ExathWebPlugin {
  static void registerWith(Registrar registrar) {
    // No method channels: exath talks to the WASM module via js_interop.
  }
}
