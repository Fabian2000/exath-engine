/// Numerical methods: derivative, integral, sum, product.
///
/// All functions operate on real-valued single-variable expressions
/// and return f64 (complex input/output is not supported here).

use crate::angle_mode::AngleMode;
use crate::ast::{parse_str, eval_ast, UserFns};
use crate::error::ExathError;
use crate::evaluator::Cx;
use std::collections::HashMap;

// ── Helper: evaluate expr with one real variable ──────────────────────────────

fn eval_at(
    ast: &crate::ast::Ast,
    var: &str,
    x: f64,
    angle_mode: AngleMode,
) -> Result<f64, ExathError> {
    let mut vars = HashMap::new();
    vars.insert(var.to_string(), Cx::real(x));
    let empty_fns = UserFns::new();
    let result = eval_ast(ast, &vars, &empty_fns, angle_mode)?;
    if result.is_real() {
        Ok(result.re)
    } else {
        Err(ExathError::complex_result(format!(
            "Expression produced a complex value at x={}",
            x
        )))
    }
}

// ── Derivative (central finite difference) ────────────────────────────────────

/// Numerically differentiate `expr` with respect to `var` at `x`.
///
/// Uses central finite difference: f'(x) ≈ (f(x+h) - f(x-h)) / (2h)
/// Step size h = max(|x| * 1e-7, 1e-10) for relative scaling.
pub fn deriv(
    expr: &str,
    var: &str,
    x: f64,
    angle_mode: AngleMode,
) -> Result<f64, ExathError> {
    let ast = parse_str(expr)?;
    let h = (x.abs() * 1e-7_f64).max(1e-10_f64);
    let forward = eval_at(&ast, var, x + h, angle_mode)?;
    let backward = eval_at(&ast, var, x - h, angle_mode)?;
    Ok((forward - backward) / (2.0 * h))
}

// ── Integral (composite Simpson's rule) ───────────────────────────────────────

/// Numerically integrate `expr` with respect to `var` from `a` to `b`.
///
/// Uses composite Simpson's rule with n=1000 intervals (must be even).
pub fn integrate(
    expr: &str,
    var: &str,
    a: f64,
    b: f64,
    angle_mode: AngleMode,
) -> Result<f64, ExathError> {
    const N: usize = 1000;
    let ast = parse_str(expr)?;
    let step = (b - a) / N as f64;

    let first = eval_at(&ast, var, a, angle_mode)?;
    let last = eval_at(&ast, var, b, angle_mode)?;

    let mut total = first + last;
    for i in 1..N {
        let x = a + i as f64 * step;
        let value = eval_at(&ast, var, x, angle_mode)?;
        total += if i % 2 == 0 { 2.0 * value } else { 4.0 * value };
    }
    Ok(total * step / 3.0)
}

// ── Sum / Product ─────────────────────────────────────────────────────────────

const MAX_TERMS: i64 = 10_000_000;

/// Compute Σ expr for `var` = `from` to `to` (inclusive, integer steps).
pub fn sum(
    expr: &str,
    var: &str,
    from: i64,
    to: i64,
    angle_mode: AngleMode,
) -> Result<f64, ExathError> {
    if to - from > MAX_TERMS {
        return Err(ExathError::range_too_large(format!(
            "Sum range too large (max {} terms)",
            MAX_TERMS
        )));
    }
    let ast = parse_str(expr)?;
    let mut accumulator = 0.0f64;
    for k in from..=to {
        accumulator += eval_at(&ast, var, k as f64, angle_mode)?;
    }
    Ok(accumulator)
}

/// Compute Π expr for `var` = `from` to `to` (inclusive, integer steps).
pub fn prod(
    expr: &str,
    var: &str,
    from: i64,
    to: i64,
    angle_mode: AngleMode,
) -> Result<f64, ExathError> {
    if to - from > MAX_TERMS {
        return Err(ExathError::range_too_large(format!(
            "Product range too large (max {} terms)",
            MAX_TERMS
        )));
    }
    let ast = parse_str(expr)?;
    let mut accumulator = 1.0f64;
    for k in from..=to {
        accumulator *= eval_at(&ast, var, k as f64, angle_mode)?;
    }
    Ok(accumulator)
}
