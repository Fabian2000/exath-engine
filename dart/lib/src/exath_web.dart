import 'dart:js_interop';

import 'types.dart';

// Bindings to the wasm-bindgen module, expected to be exposed on the JS global
// as `exath` (see the package README for the one-line setup).

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

ExathResult evaluate(String expr, {AngleMode angleMode = AngleMode.rad}) {
  final r = _jsEvaluate(expr.toJS, angleMode.name_.toJS);
  if (r.isError) return ExathResult(0, 0, error: r.errorMessage);
  return ExathResult(r.re, r.im);
}

bool isValid(String expr) => _jsIsValid(expr.toJS);

List<String> supportedFunctions() =>
    _jsSupportedFunctions().toDart.map((e) => e.toDart).toList();

/// A stateful session backed by the WASM `ExathSession`.
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

  /// No-op on web; the JS garbage collector reclaims the session.
  void dispose() {}
}
