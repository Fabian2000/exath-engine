//! Randomised CAS invariant testing: generate thousands of random expressions
//! and check mathematical invariants numerically. Surfaces correctness bugs in
//! diff / simplify / expand / factor that example-based tests miss.

use exath_engine::symbolic::{differentiate, expand, factor, simplify_expr};
use exath_engine::{AngleMode, CalcResult, Session};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn n(&mut self, m: u64) -> u64 {
        self.next() % m
    }
}

/// Generate a random expression in `x` avoiding singular ops (no /, ln, sqrt),
/// so numeric checks aren't polluted by domain errors.
fn gen(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 || rng.n(100) < 35 {
        return match rng.n(3) {
            0 => "x".to_string(),
            _ => format!("{}", 1 + rng.n(5)),
        };
    }
    match rng.n(5) {
        0 => {
            let op = ["+", "-", "*"][rng.n(3) as usize];
            format!("({} {} {})", gen(rng, depth - 1), op, gen(rng, depth - 1))
        }
        1 => {
            let f = ["sin", "cos", "exp"][rng.n(3) as usize];
            format!("{}({})", f, gen(rng, depth - 1))
        }
        2 => format!("({})^{}", gen(rng, depth - 1), 1 + rng.n(3)),
        3 => format!("-({})", gen(rng, depth - 1)),
        _ => format!("({} + {})", gen(rng, depth - 1), gen(rng, depth - 1)),
    }
}

fn eval(src: &str, x: f64) -> Option<f64> {
    let mut s = Session::new(AngleMode::Rad);
    s.set_var("x", x, 0.0);
    match s.eval(src) {
        Ok(CalcResult::Real(v)) if v.is_finite() => Some(v),
        _ => None,
    }
}

const POINTS: [f64; 5] = [0.3, 0.7, 1.1, 1.9, -1.4];

/// simplify(e) must equal e at sample points, and be idempotent in value.
#[test]
fn simplify_is_value_preserving_and_idempotent() {
    let mut rng = Rng(0xCA5_5EED_01);
    for _ in 0..3000 {
        let e = gen(&mut rng, 3);
        let s1 = match simplify_expr(&e) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let s2 = simplify_expr(&s1).unwrap_or_else(|_| s1.clone());
        for x in POINTS {
            if let (Some(a), Some(b)) = (eval(&e, x), eval(&s1, x)) {
                assert!(
                    (a - b).abs() <= 1e-6 * (1.0 + a.abs()),
                    "simplify changed value:\n  e  = {}\n  s  = {}\n  at x={}: {} vs {}",
                    e, s1, x, a, b
                );
            }
            if let (Some(a), Some(b)) = (eval(&s1, x), eval(&s2, x)) {
                assert!(
                    (a - b).abs() <= 1e-6 * (1.0 + a.abs()),
                    "simplify not idempotent:\n  s1 = {}\n  s2 = {}\n  at x={}: {} vs {}",
                    s1, s2, x, a, b
                );
            }
        }
    }
}

/// d/dx(e) must match the central finite difference of e.
#[test]
fn derivative_matches_finite_difference() {
    let mut rng = Rng(0xD1FF_5EED);
    let h = 1e-6;
    for _ in 0..3000 {
        let e = gen(&mut rng, 3);
        let d = match differentiate(&e, "x") {
            Ok(s) => s,
            Err(_) => continue,
        };
        for x in POINTS {
            if let (Some(f0), Some(fp), Some(fm), Some(da)) =
                (eval(&e, x), eval(&e, x + h), eval(&e, x - h), eval(&d, x))
            {
                let fd = (fp - fm) / (2.0 * h);
                // Skip where the central difference is itself unreliable: large
                // function magnitude (catastrophic cancellation in f(x±h)) or
                // large derivative/values out of the finite-difference's range.
                if f0.abs() < 1e4 && da.abs() < 1e6 && fd.abs() < 1e6 {
                    assert!(
                        (da - fd).abs() <= 1e-3 * (1.0 + da.abs()),
                        "derivative mismatch:\n  e  = {}\n  d  = {}\n  at x={}: symbolic {} vs fd {}",
                        e, d, x, da, fd
                    );
                }
            }
        }
    }
}

/// For random integer polynomials, factor and expand must preserve value.
fn gen_poly(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 || rng.n(100) < 40 {
        return match rng.n(3) {
            0 => "x".to_string(),
            _ => format!("{}", 1 + rng.n(6)),
        };
    }
    match rng.n(4) {
        0 => format!("({} + {})", gen_poly(rng, depth - 1), gen_poly(rng, depth - 1)),
        1 => format!("({} - {})", gen_poly(rng, depth - 1), gen_poly(rng, depth - 1)),
        2 => format!("({} * {})", gen_poly(rng, depth - 1), gen_poly(rng, depth - 1)),
        _ => format!("({})^{}", gen_poly(rng, depth - 1), 1 + rng.n(3)),
    }
}

#[test]
fn factor_and_expand_preserve_value() {
    let mut rng = Rng(0xFAC_0FF);
    for _ in 0..2000 {
        let p = gen_poly(&mut rng, 3);
        let f = factor(&p, "x").unwrap_or_else(|_| p.clone());
        let ex = expand(&p).unwrap_or_else(|_| p.clone());
        for x in POINTS {
            if let (Some(a), Some(b)) = (eval(&p, x), eval(&f, x)) {
                assert!(
                    (a - b).abs() <= 1e-6 * (1.0 + a.abs()),
                    "factor changed value:\n  p = {}\n  f = {}\n  at x={}: {} vs {}",
                    p, f, x, a, b
                );
            }
            if let (Some(a), Some(b)) = (eval(&p, x), eval(&ex, x)) {
                assert!(
                    (a - b).abs() <= 1e-6 * (1.0 + a.abs()),
                    "expand changed value:\n  p = {}\n  ex = {}\n  at x={}: {} vs {}",
                    p, ex, x, a, b
                );
            }
        }
    }
}
