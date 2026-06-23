import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

import 'types.dart';

// ── C struct mappings ────────────────────────────────────────────────────────

final class _CResult extends Struct {
  @Double()
  external double re;
  @Double()
  external double im;
  @Int32()
  external int isError;
  external Pointer<Utf8> errorMsg;
}

final class _CLineResult extends Struct {
  @Int32()
  external int isExpression;
  external Pointer<Utf8> expression;
  @Double()
  external double re;
  @Double()
  external double im;
  @Int32()
  external int isError;
  external Pointer<Utf8> errorMsg;
}

final class _CSession extends Opaque {}

// ── Library loading ──────────────────────────────────────────────────────────

DynamicLibrary _open() {
  final override = Platform.environment['EXATH_LIB'];
  if (override != null && override.isNotEmpty) {
    return DynamicLibrary.open(override);
  }
  if (Platform.isWindows) return DynamicLibrary.open('exath_engine_ffi.dll');
  if (Platform.isMacOS) return DynamicLibrary.open('libexath_engine_ffi.dylib');
  return DynamicLibrary.open('libexath_engine_ffi.so');
}

final DynamicLibrary _lib = _open();

// ── Function bindings ────────────────────────────────────────────────────────

final _evaluate = _lib.lookupFunction<_CResult Function(Pointer<Utf8>, Int32),
    _CResult Function(Pointer<Utf8>, int)>('exath_evaluate');

final _isValid = _lib.lookupFunction<Int32 Function(Pointer<Utf8>),
    int Function(Pointer<Utf8>)>('exath_is_valid');

final _supportedFunctions = _lib.lookupFunction<Pointer<Utf8> Function(),
    Pointer<Utf8> Function()>('exath_supported_functions');

final _sessionNew = _lib.lookupFunction<Pointer<_CSession> Function(Int32),
    Pointer<_CSession> Function(int)>('exath_session_new');

final _sessionFree = _lib.lookupFunction<Void Function(Pointer<_CSession>),
    void Function(Pointer<_CSession>)>('exath_session_free');

final _sessionEval = _lib.lookupFunction<
    _CResult Function(Pointer<_CSession>, Pointer<Utf8>),
    _CResult Function(Pointer<_CSession>, Pointer<Utf8>)>('exath_session_eval');

final _sessionEvalLine = _lib.lookupFunction<
    _CLineResult Function(Pointer<_CSession>, Pointer<Utf8>),
    _CLineResult Function(
        Pointer<_CSession>, Pointer<Utf8>)>('exath_session_eval_line');

final _sessionSetVar = _lib.lookupFunction<
    Void Function(Pointer<_CSession>, Pointer<Utf8>, Double, Double),
    void Function(Pointer<_CSession>, Pointer<Utf8>, double,
        double)>('exath_session_set_var');

final _sessionRemoveVar = _lib.lookupFunction<
    Void Function(Pointer<_CSession>, Pointer<Utf8>),
    void Function(
        Pointer<_CSession>, Pointer<Utf8>)>('exath_session_remove_var');

final _sessionClearVars = _lib.lookupFunction<Void Function(Pointer<_CSession>),
    void Function(Pointer<_CSession>)>('exath_session_clear_vars');

final _sessionRemoveFn = _lib.lookupFunction<
    Void Function(Pointer<_CSession>, Pointer<Utf8>),
    void Function(
        Pointer<_CSession>, Pointer<Utf8>)>('exath_session_remove_fn');

final _sessionFnNames = _lib.lookupFunction<
    Pointer<Utf8> Function(Pointer<_CSession>),
    Pointer<Utf8> Function(Pointer<_CSession>)>('exath_session_fn_names');

final _sessionVarNames = _lib.lookupFunction<
    Pointer<Utf8> Function(Pointer<_CSession>),
    Pointer<Utf8> Function(Pointer<_CSession>)>('exath_session_var_names');

final _freeString = _lib.lookupFunction<Void Function(Pointer<Utf8>),
    void Function(Pointer<Utf8>)>('exath_free_string');

// ── Helpers ──────────────────────────────────────────────────────────────────

T _withCString<T>(String s, T Function(Pointer<Utf8>) body) {
  final ptr = s.toNativeUtf8();
  try {
    return body(ptr);
  } finally {
    calloc.free(ptr);
  }
}

/// Read an owned C string (allocated by the engine) and free it.
String _takeString(Pointer<Utf8> ptr) {
  if (ptr == nullptr) return '';
  final s = ptr.toDartString();
  _freeString(ptr);
  return s;
}

// ── Public API ───────────────────────────────────────────────────────────────

ExathResult evaluate(String expr, {AngleMode angleMode = AngleMode.rad}) {
  return _withCString(expr, (p) {
    final r = _evaluate(p, angleMode.code);
    if (r.isError != 0) {
      return ExathResult(0, 0, error: _takeString(r.errorMsg));
    }
    return ExathResult(r.re, r.im);
  });
}

bool isValid(String expr) => _withCString(expr, (p) => _isValid(p) != 0);

List<String> supportedFunctions() {
  final s = _takeString(_supportedFunctions());
  return s.isEmpty ? const [] : s.split(',');
}

/// A stateful session: variables and user-defined functions persist across
/// calls. Call [dispose] when done to free the native session.
class ExathSession {
  Pointer<_CSession> _handle;
  bool _disposed = false;

  ExathSession({AngleMode angleMode = AngleMode.rad})
      : _handle = _sessionNew(angleMode.code);

  void _check() {
    if (_disposed) throw StateError('ExathSession used after dispose()');
  }

  /// Evaluate one numeric line (`var = expr`, `f(x) = ...`, or an expression).
  ExathResult eval(String line) {
    _check();
    return _withCString(line, (p) {
      final r = _sessionEval(_handle, p);
      if (r.isError != 0) {
        return ExathResult(0, 0, error: _takeString(r.errorMsg));
      }
      return ExathResult(r.re, r.im);
    });
  }

  /// Evaluate one line including symbolic / matrix forms. Returns a
  /// [NumberResult] for numeric results or an [ExpressionResult] for symbolic
  /// ones. Throws [ExathException] on error.
  LineResult evalLine(String line) {
    _check();
    return _withCString(line, (p) {
      final r = _sessionEvalLine(_handle, p);
      if (r.isError != 0) {
        throw ExathException(_takeString(r.errorMsg));
      }
      if (r.isExpression != 0) {
        return ExpressionResult(_takeString(r.expression));
      }
      return NumberResult(r.re, r.im);
    });
  }

  void setVar(String name, double re, [double im = 0]) {
    _check();
    _withCString(name, (p) => _sessionSetVar(_handle, p, re, im));
  }

  void removeVar(String name) {
    _check();
    _withCString(name, (p) => _sessionRemoveVar(_handle, p));
  }

  void clearVars() {
    _check();
    _sessionClearVars(_handle);
  }

  void removeFn(String name) {
    _check();
    _withCString(name, (p) => _sessionRemoveFn(_handle, p));
  }

  List<String> varNames() {
    _check();
    final s = _takeString(_sessionVarNames(_handle));
    return s.isEmpty ? const [] : s.split(',');
  }

  List<String> fnNames() {
    _check();
    final s = _takeString(_sessionFnNames(_handle));
    return s.isEmpty ? const [] : s.split(',');
  }

  /// Free the native session. The instance must not be used afterwards.
  void dispose() {
    if (_disposed) return;
    _sessionFree(_handle);
    _handle = nullptr;
    _disposed = true;
  }
}
