/// Angle mode applied to trigonometric functions.
enum AngleMode {
  rad,
  deg,
  grad;

  /// The engine's integer encoding (`Deg = 0, Rad = 1, Grad = 2`).
  int get code => switch (this) {
        AngleMode.deg => 0,
        AngleMode.rad => 1,
        AngleMode.grad => 2,
      };

  /// The engine's string encoding used by the WASM build.
  String get name_ => switch (this) {
        AngleMode.deg => 'deg',
        AngleMode.rad => 'rad',
        AngleMode.grad => 'grad',
      };
}

/// Result of a numeric evaluation. A real result has [im] == 0.
class ExathResult {
  final double re;
  final double im;

  /// Error message, or `null` on success.
  final String? error;

  const ExathResult(this.re, this.im, {this.error});

  bool get isComplex => im != 0;
  bool get isError => error != null;

  @override
  String toString() {
    if (isError) return 'Error: $error';
    if (isComplex) return '$re ${im < 0 ? '-' : '+'} ${im.abs()}i';
    return '$re';
  }
}

/// Result of [ExathSession.evalLine]: either a numeric value or a symbolic
/// expression string (for `diff`, `factor`, `solve`, ... forms).
sealed class LineResult {
  const LineResult();
}

/// A numeric line result (`2 + 3`, `det([[1,2],[3,4]])`, ...).
class NumberResult extends LineResult {
  final double re;
  final double im;
  const NumberResult(this.re, this.im);
  bool get isComplex => im != 0;

  @override
  String toString() => isComplex ? '$re ${im < 0 ? '-' : '+'} ${im.abs()}i' : '$re';
}

/// A symbolic line result (`2 * x`, `(x + 1) * (x - 1)`, ...).
class ExpressionResult extends LineResult {
  final String expression;
  const ExpressionResult(this.expression);

  @override
  String toString() => expression;
}

/// Thrown when the engine reports an error for a line or expression.
class ExathException implements Exception {
  final String message;
  const ExathException(this.message);
  @override
  String toString() => 'ExathException: $message';
}
