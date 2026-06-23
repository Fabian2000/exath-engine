import 'types.dart';

const _msg = 'exath: no native (dart:ffi) or web (js_interop) backend available '
    'on this platform';

Future<void> ensureInitialized() async {}

ExathResult evaluate(String expr, {AngleMode angleMode = AngleMode.rad}) =>
    throw UnsupportedError(_msg);

bool isValid(String expr) => throw UnsupportedError(_msg);

List<String> supportedFunctions() => throw UnsupportedError(_msg);

class ExathSession {
  ExathSession({AngleMode angleMode = AngleMode.rad}) {
    throw UnsupportedError(_msg);
  }

  ExathResult eval(String line) => throw UnsupportedError(_msg);
  LineResult evalLine(String line) => throw UnsupportedError(_msg);
  void setVar(String name, double re, [double im = 0]) =>
      throw UnsupportedError(_msg);
  void removeVar(String name) => throw UnsupportedError(_msg);
  void clearVars() => throw UnsupportedError(_msg);
  void removeFn(String name) => throw UnsupportedError(_msg);
  List<String> varNames() => throw UnsupportedError(_msg);
  List<String> fnNames() => throw UnsupportedError(_msg);
  void dispose() {}
}
