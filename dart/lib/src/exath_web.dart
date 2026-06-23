import 'dart:async';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';

import 'package:web/web.dart' as web;

import 'types.dart';

// The wasm-bindgen module is loaded by [ensureInitialized] and exposed on
// `globalThis.exath`; these bindings then call into it.

@JS('exath.evaluate')
external _JsResult _jsEvaluate(JSString expr, JSString angleMode);

@JS('exath.isValid')
external bool _jsIsValid(JSString expr);

@JS('exath.supportedFunctions')
external JSArray<JSString> _jsSupportedFunctions();

extension type _JsResult._(JSObject _) implements JSObject {
  external double get re;
  external double get im;
  external bool get isError;
  external String? get errorMessage;
}

extension type _JsLine._(JSObject _) implements JSObject {
  external bool get isExpression;
  external String get expression;
  external double get re;
  external double get im;
  external bool get isError;
  external String? get errorMessage;
}

@JS('exath.ExathSession')
extension type _JsSession._(JSObject _) implements JSObject {
  external factory _JsSession(JSString angleMode);
  external _JsResult eval(JSString line);
  external _JsLine evalLine(JSString line);
  external void setVar(JSString name, double re, double im);
  external void removeVar(JSString name);
  external void clearVars();
  external void removeFn(JSString name);
  external JSArray<JSString> varNames();
  external JSArray<JSString> fnNames();
}

bool _ready = false;

/// Loads and initializes the bundled WASM module. Call once before using the
/// API on web; subsequent calls are no-ops. (No-op on native platforms.)
Future<void> ensureInitialized() async {
  if (_ready) return;
  const base = 'assets/packages/exath_engine/assets/wasm';
  final completer = Completer<void>();
  late final JSFunction listener;
  listener = ((web.Event _) {
    if (completer.isCompleted) return;
    final err = globalContext.getProperty('__exathError'.toJS);
    if (err.isDefinedAndNotNull) {
      completer.completeError(
          ExathException('exath wasm init failed: ${err.dartify()}'));
    } else {
      completer.complete();
    }
  }).toJS;
  web.window.addEventListener('exath:ready', listener);
  final script = web.HTMLScriptElement()
    ..type = 'module'
    ..text = '''
import init, * as m from '$base/exath_engine_wasm.js';
init('$base/exath_engine_wasm_bg.wasm')
  .then(() => { globalThis.exath = m; })
  .catch((e) => { globalThis.__exathError = String(e); })
  .finally(() => window.dispatchEvent(new Event('exath:ready')));
''';
  web.document.head!.appendChild(script);
  await completer.future;
  web.window.removeEventListener('exath:ready', listener);
  _ready = true;
}

ExathResult evaluate(String expr, {AngleMode angleMode = AngleMode.rad}) {
  final r = _jsEvaluate(expr.toJS, angleMode.name_.toJS);
  if (r.isError) return ExathResult(0, 0, error: r.errorMessage);
  return ExathResult(r.re, r.im);
}

bool isValid(String expr) => _jsIsValid(expr.toJS);

List<String> supportedFunctions() =>
    _jsSupportedFunctions().toDart.map((e) => e.toDart).toList();

class ExathSession {
  final _JsSession _s;

  ExathSession({AngleMode angleMode = AngleMode.rad})
      : _s = _JsSession(angleMode.name_.toJS);

  ExathResult eval(String line) {
    final r = _s.eval(line.toJS);
    if (r.isError) return ExathResult(0, 0, error: r.errorMessage);
    return ExathResult(r.re, r.im);
  }

  LineResult evalLine(String line) {
    final r = _s.evalLine(line.toJS);
    if (r.isError) throw ExathException(r.errorMessage ?? 'error');
    if (r.isExpression) return ExpressionResult(r.expression);
    return NumberResult(r.re, r.im);
  }

  void setVar(String name, double re, [double im = 0]) =>
      _s.setVar(name.toJS, re, im);
  void removeVar(String name) => _s.removeVar(name.toJS);
  void clearVars() => _s.clearVars();
  void removeFn(String name) => _s.removeFn(name.toJS);
  List<String> varNames() =>
      _s.varNames().toDart.map((e) => e.toDart).toList();
  List<String> fnNames() => _s.fnNames().toDart.map((e) => e.toDart).toList();

  void dispose() {}
}
