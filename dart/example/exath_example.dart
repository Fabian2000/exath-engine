import 'package:exath/exath.dart';

void main() {
  // One-shot numeric evaluation.
  print(evaluate('2^10 + sqrt(9)')); // 1027.0
  print(evaluate('sqrt(-4)')); // 0.0 + 2.0i

  final s = ExathSession();

  // Stateful numeric evaluation.
  s.eval('r = 3');
  s.eval('h = 4');
  print(s.eval('pi * r^2 * h').re); // 113.097...

  // Symbolic forms via evalLine (computer algebra, linear algebra, ...).
  print(s.evalLine('diff(sin(x^2), x)')); // 2 * x * cos(x^2)
  print(s.evalLine('factor(x^2 - 5*x + 6, x)')); // (x - 2) * (x - 3)
  print(s.evalLine('solve(x^2 - 4, x)')); // x = 2, x = -2
  print(s.evalLine('integral(x^2, x)')); // x^3 / 3
  print(s.evalLine('det([[1,2],[3,4]])')); // -2.0

  // A symbolic result can be bound and reused (resolve it with evalLine).
  s.evalLine('g = diff(x^3, x)'); // 3 * x^2
  s.evalLine('x = 2');
  print(s.evalLine('g')); // 12.0

  print(supportedFunctions().take(5).toList());

  s.dispose();
}
