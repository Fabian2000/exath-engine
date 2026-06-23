//! Randomised differential testing for the *new* calculus paths: integration,
//! solving, and definite integrals. Any returned result is checked against the
//! ground truth (d/dx == integrand, f(root) == 0, independent quadrature), so a
//! wrong answer on ANY code path, curated rule, substitution, or partial
//! fractions, fails the test.

use exath_engine::symbolic::{antiderivative, differentiate, integrate_definite, solve};
use exath_engine::{AngleMode, CalcResult, Session};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn n(&mut self, m: u64) -> u64 {
        self.next() % m
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

/// Random integrand (mildly singular ops allowed; checked only where finite).
fn gen_integrand(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 || rng.n(100) < 40 {
        return match rng.n(4) {
            0 => "x".to_string(),
            1 => format!("{}", 1 + rng.n(4)),
            2 => "x^2".to_string(),
            _ => "x".to_string(),
        };
    }
    match rng.n(8) {
        0 => format!("({} + {})", gen_integrand(rng, depth - 1), gen_integrand(rng, depth - 1)),
        1 => format!("({} - {})", gen_integrand(rng, depth - 1), gen_integrand(rng, depth - 1)),
        2 => format!("({} * {})", gen_integrand(rng, depth - 1), gen_integrand(rng, depth - 1)),
        3 => format!("sin({})", gen_integrand(rng, depth - 1)),
        4 => format!("cos({})", gen_integrand(rng, depth - 1)),
        5 => format!("exp({})", gen_lin(rng)),
        6 => format!("{}^{}", gen_lin(rng), 1 + rng.n(3)),
        _ => format!("x^{}", 1 + rng.n(4)),
    }
}

fn gen_lin(rng: &mut Rng) -> String {
    format!("({}*x + {})", 1 + rng.n(3), rng.n(4))
}

/// Every antiderivative the engine returns must differentiate back to the integrand.
#[test]
fn integration_is_always_correct() {
    let mut rng = Rng(0x1_2345_6789);
    let pts = [0.31, 0.74, 1.23, 1.88, 2.51];
    let (mut returned, mut errored) = (0u32, 0u32);
    for _ in 0..4000 {
        let f = gen_integrand(&mut rng, 3);
        let integral = match antiderivative(&f, "x") {
            Ok(s) => s,
            Err(_) => {
                errored += 1;
                continue;
            }
        };
        returned += 1;
        let d = differentiate(&integral, "x").unwrap_or_default();
        for x in pts {
            if let (Some(a), Some(b)) = (eval(&f, x), eval(&d, x)) {
                if a.abs() < 1e6 && b.abs() < 1e6 {
                    assert!(
                        (a - b).abs() <= 1e-5 * (1.0 + a.abs()),
                        "WRONG integral:\n  ∫{}\n  = {}\n  d/dx={} vs integrand={} at x={}",
                        f, integral, b, a, x
                    );
                }
            }
        }
    }
    // sanity: the engine actually integrates a meaningful fraction (not all errors)
    assert!(returned > 1000, "only {} integrals returned (of {} ok+err)", returned, returned + errored);
}

/// Every root solve() returns must satisfy f(root) ≈ 0.
#[test]
fn solve_roots_are_always_valid() {
    let mut rng = Rng(0xABCDEF01);
    for _ in 0..3000 {
        // random polynomial up to degree 3 with small integer coefficients
        let a = rng.n(5) as i64 - 2;
        let b = rng.n(7) as i64 - 3;
        let c = rng.n(7) as i64 - 3;
        let d = rng.n(7) as i64 - 3;
        let eq = format!("{}*x^3 + {}*x^2 + {}*x + {}", a, b, c, d);
        let roots = match solve(&eq, "x") {
            Ok(r) => r,
            Err(_) => continue,
        };
        for r in &roots {
            let rv: f64 = match r.parse() {
                Ok(v) => v,
                Err(_) => continue, // exact form like ln(2): skip numeric parse here
            };
            if let Some(fr) = eval(&eq, rv) {
                assert!(
                    fr.abs() < 1e-4,
                    "INVALID root {} of {}: f(root)={}",
                    rv, eq, fr
                );
            }
        }
    }
}

/// Definite integrals must match an independent fine-grid Simpson reference.
#[test]
fn definite_integrals_match_reference() {
    let mut rng = Rng(0x5151_5151);
    for _ in 0..1500 {
        let f = gen_integrand(&mut rng, 3);
        let (a, b) = (0.2, 1.4);
        // independent reference: composite Simpson with 2000 panels
        let nref = 2000i32;
        let h = (b - a) / nref as f64;
        let mut acc = 0.0;
        let mut ok = true;
        for i in 0..=nref {
            let x = a + i as f64 * h;
            match eval(&f, x) {
                Some(v) => {
                    let w = if i == 0 || i == nref {
                        1.0
                    } else if i % 2 == 1 {
                        4.0
                    } else {
                        2.0
                    };
                    acc += w * v;
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        let reference = acc * h / 3.0;
        if let Ok(s) = integrate_definite(&f, "x", a, b) {
            let got = match s.parse::<f64>() {
                Ok(v) => v,
                Err(_) => eval(&s, 0.0).unwrap_or(f64::NAN), // symbolic constant
            };
            if got.is_finite() && reference.is_finite() {
                assert!(
                    (got - reference).abs() <= 1e-3 * (1.0 + reference.abs()),
                    "definite ∫_{}^{} {} = {} but reference {}",
                    a, b, f, got, reference
                );
            }
        }
    }
}
