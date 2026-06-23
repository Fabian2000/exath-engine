//! Verification layer: fuzzing (panic-safety) and differential/property checks.

use exath_engine::symbolic::{factor, simplify_expr};
use exath_engine::{evaluate, is_valid, AngleMode, CalcResult, Session};

/// Tiny deterministic PRNG (no external deps, reproducible).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next() as usize) % xs.len()]
    }
}

/// Feeding random token soup must never panic — only ever Ok/Err.
#[test]
fn fuzz_parser_and_evaluator_never_panic() {
    let toks = [
        "1", "2.5", "0", "x", "y", "+", "-", "*", "/", "^", "(", ")", "sin", "cos", "tan",
        "sqrt", "ln", "exp", "!", ",", "[", "]", "pi", "e", "i", " ", "gamma", "abs", "<", "==",
        "&&", "1,5", "diff", "solve", "x^2", ";",
    ];
    let mut rng = Rng(0x9E3779B97F4A7C15);
    for _ in 0..30_000 {
        let len = (rng.next() % 14) as usize;
        let mut s = String::new();
        for _ in 0..len {
            s.push_str(rng.pick(&toks));
        }
        // None of these may panic; results are irrelevant.
        let _ = is_valid(&s);
        let _ = evaluate(&s, AngleMode::Rad);
        let mut sess = Session::new(AngleMode::Rad);
        let _ = sess.eval_line(&s);
    }
}

fn eval_xy(src: &str, x: f64, y: f64) -> Option<f64> {
    let mut s = Session::new(AngleMode::Rad);
    s.set_var("x", x, 0.0);
    s.set_var("y", y, 0.0);
    match s.eval(src) {
        Ok(CalcResult::Real(v)) => Some(v),
        Ok(CalcResult::Complex(re, _)) => Some(re),
        Err(_) => None,
    }
}

/// simplify(e) must agree with e at sample points (no value-changing rewrites).
#[test]
fn differential_simplify_preserves_value() {
    let corpus = [
        "x + x + x",
        "(x + 1)^3",
        "(x + 2)*(x - 2)",
        "x*y + y*x",
        "sin(x)^2 + cos(x)^2",
        "x/2 + x/3",
        "2*x^2 - 8",
        "ln(x) + ln(y)",
        "exp(x)*exp(y)",
        "sqrt(8) + sqrt(2)",
        "(x^2 - 1)/(x - 1)",
        "x^3 - 3*x^2 + 3*x - 1",
        "tan(x)*cos(x)",
    ];
    let points = [(0.7, 1.3), (2.4, -0.6), (3.1, 2.2), (-1.5, 0.8), (5.0, 4.0)];
    for e in corpus {
        let s = simplify_expr(e).unwrap_or_else(|_| e.to_string());
        for (x, y) in points {
            if let (Some(a), Some(b)) = (eval_xy(e, x, y), eval_xy(&s, x, y)) {
                if a.is_finite() && b.is_finite() {
                    assert!(
                        (a - b).abs() <= 1e-6 * (1.0 + a.abs()),
                        "simplify changed value: {} -> {} at ({},{}): {} vs {}",
                        e, s, x, y, a, b
                    );
                }
            }
        }
    }
}

/// factor(p) must equal p at sample points (factoring preserves the polynomial).
#[test]
fn differential_factor_preserves_value() {
    let polys = [
        "x^2 - 5*x + 6",
        "x^2 - 4",
        "x^3 - x",
        "2*x^2 - 2",
        "x^3 - 6*x^2 + 11*x - 6",
    ];
    for p in polys {
        let f = factor(p, "x").unwrap_or_else(|_| p.to_string());
        for x in [-2.3, 0.5, 1.7, 4.2] {
            if let (Some(a), Some(b)) = (eval_xy(p, x, 0.0), eval_xy(&f, x, 0.0)) {
                assert!(
                    (a - b).abs() <= 1e-6 * (1.0 + a.abs()),
                    "factor changed value: {} -> {} at x={}: {} vs {}",
                    p, f, x, a, b
                );
            }
        }
    }
}
