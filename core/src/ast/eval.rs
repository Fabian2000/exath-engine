use crate::angle_mode::AngleMode;
use crate::error::ExathError;
use crate::evaluator::{Cx, apply_function, factorial};
use super::types::{Ast, BinOp};
use std::collections::HashMap;

/// A map of user-defined functions: name → (parameter names, body AST).
pub type UserFns = HashMap<String, (Vec<String>, Ast)>;

/// Evaluate an AST with a variable map and user-defined functions.
pub fn eval_ast(
    ast: &Ast,
    vars: &HashMap<String, Cx>,
    fns: &UserFns,
    angle_mode: AngleMode,
) -> Result<Cx, ExathError> {
    match ast {
        Ast::Number(value) => Ok(Cx::real(*value)),

        Ast::Var(name) => vars
            .get(name)
            .copied()
            .ok_or_else(|| ExathError::undefined(format!("Undefined variable: {}", name))),

        Ast::BinOp(op, left_ast, right_ast) => {
            // Short-circuit for logical operators
            match op {
                BinOp::And => {
                    let left = eval_ast(left_ast, vars, fns, angle_mode)?;
                    if left.re == 0.0 && left.im == 0.0 {
                        return Ok(Cx::real(0.0));
                    }
                    let right = eval_ast(right_ast, vars, fns, angle_mode)?;
                    let truthy = right.re != 0.0 || right.im != 0.0;
                    return Ok(Cx::real(if truthy { 1.0 } else { 0.0 }));
                }
                BinOp::Or => {
                    let left = eval_ast(left_ast, vars, fns, angle_mode)?;
                    if left.re != 0.0 || left.im != 0.0 {
                        return Ok(Cx::real(1.0));
                    }
                    let right = eval_ast(right_ast, vars, fns, angle_mode)?;
                    let truthy = right.re != 0.0 || right.im != 0.0;
                    return Ok(Cx::real(if truthy { 1.0 } else { 0.0 }));
                }
                _ => {}
            }

            let left = eval_ast(left_ast, vars, fns, angle_mode)?;
            let right = eval_ast(right_ast, vars, fns, angle_mode)?;
            match op {
                BinOp::Add => Ok(left.add(right)),
                BinOp::Sub => Ok(left.sub(right)),
                BinOp::Mul => Ok(left.mul(right)),
                BinOp::Div => left.div(right),
                BinOp::Pow => left.pow(right),
                BinOp::Mod => {
                    if right.re == 0.0 && right.im == 0.0 {
                        return Err(ExathError::domain("Modulo by zero"));
                    }
                    if !right.is_real() {
                        return Err(ExathError::arg_type(
                            "Modulo only defined for real numbers",
                        ));
                    }
                    Ok(Cx::real(left.re % right.re))
                }
                BinOp::Eq => cmp_op(left, right, |a, b| (a - b).abs() < 1e-12),
                BinOp::Ne => cmp_op(left, right, |a, b| (a - b).abs() >= 1e-12),
                BinOp::Lt => cmp_op(left, right, |a, b| a < b),
                BinOp::Le => cmp_op(left, right, |a, b| a <= b),
                BinOp::Gt => cmp_op(left, right, |a, b| a > b),
                BinOp::Ge => cmp_op(left, right, |a, b| a >= b),
                BinOp::And | BinOp::Or => unreachable!(),
            }
        }

        Ast::UnaryNeg(inner) => {
            Ok(eval_ast(inner, vars, fns, angle_mode)?.neg())
        }

        Ast::UnaryNot(inner) => {
            let value = eval_ast(inner, vars, fns, angle_mode)?;
            let is_zero = value.re == 0.0 && value.im == 0.0;
            Ok(Cx::real(if is_zero { 1.0 } else { 0.0 }))
        }

        Ast::Factorial(inner) => {
            let value = eval_ast(inner, vars, fns, angle_mode)?;
            if !value.is_real() {
                return Err(ExathError::arg_type("Factorial only for real numbers"));
            }
            Ok(Cx::real(factorial(value.re)?))
        }

        Ast::Call(name, args) => {
            eval_call(name, args, vars, fns, angle_mode)
        }

        Ast::Matrix(_) => Err(ExathError::domain(
            "matrices are not valid in a scalar expression",
        )),
    }
}

/// Evaluate a function call with its argument AST nodes (lazy, args not yet evaluated).
fn eval_call(
    name: &str,
    args: &[Ast],
    vars: &HashMap<String, Cx>,
    fns: &UserFns,
    angle_mode: AngleMode,
) -> Result<Cx, ExathError> {
    // User-defined functions
    if let Some((params, body)) = fns.get(name) {
        if args.len() != params.len() {
            return Err(ExathError::arg_count(format!(
                "{}() expects {} argument(s), got {}",
                name,
                params.len(),
                args.len()
            )));
        }
        let mut call_vars = vars.clone();
        for (param, arg_ast) in params.iter().zip(args.iter()) {
            let value = eval_ast(arg_ast, vars, fns, angle_mode)?;
            call_vars.insert(param.clone(), value);
        }
        return eval_ast(body, &call_vars, fns, angle_mode);
    }

    // Multi-argument / control-flow built-in functions
    match name {
        "if" => {
            if args.len() != 3 {
                return Err(ExathError::arg_count(
                    "if requires 3 arguments: if(condition, true_value, false_value)",
                ));
            }
            let condition = eval_ast(&args[0], vars, fns, angle_mode)?;
            if condition.re != 0.0 || condition.im != 0.0 {
                eval_ast(&args[1], vars, fns, angle_mode)
            } else {
                eval_ast(&args[2], vars, fns, angle_mode)
            }
        }

        "piecewise" => {
            // piecewise(c1, v1, c2, v2, ..., default): first true condition wins.
            if args.len() < 3 || args.len() % 2 == 0 {
                return Err(ExathError::arg_count(
                    "piecewise expects an odd number of arguments: cond, val, …, default",
                ));
            }
            let mut i = 0;
            while i + 1 < args.len() {
                let cond = eval_ast(&args[i], vars, fns, angle_mode)?;
                if cond.re != 0.0 || cond.im != 0.0 {
                    return eval_ast(&args[i + 1], vars, fns, angle_mode);
                }
                i += 2;
            }
            eval_ast(&args[args.len() - 1], vars, fns, angle_mode)
        }

        "min" => {
            if args.is_empty() {
                return Err(ExathError::arg_count("min requires at least one argument"));
            }
            let mut best = eval_real_arg(&args[0], vars, fns, angle_mode, "min")?;
            for arg in &args[1..] {
                let value = eval_real_arg(arg, vars, fns, angle_mode, "min")?;
                if value < best {
                    best = value;
                }
            }
            Ok(Cx::real(best))
        }

        "max" => {
            if args.is_empty() {
                return Err(ExathError::arg_count("max requires at least one argument"));
            }
            let mut best = eval_real_arg(&args[0], vars, fns, angle_mode, "max")?;
            for arg in &args[1..] {
                let value = eval_real_arg(arg, vars, fns, angle_mode, "max")?;
                if value > best {
                    best = value;
                }
            }
            Ok(Cx::real(best))
        }

        "clamp" => {
            if args.len() != 3 {
                return Err(ExathError::arg_count(
                    "clamp requires 3 arguments: clamp(x, min, max)",
                ));
            }
            let value = eval_real_arg(&args[0], vars, fns, angle_mode, "clamp")?;
            let lower = eval_real_arg(&args[1], vars, fns, angle_mode, "clamp")?;
            let upper = eval_real_arg(&args[2], vars, fns, angle_mode, "clamp")?;
            Ok(Cx::real(value.max(lower).min(upper)))
        }

        "gcd" => {
            if args.len() != 2 {
                return Err(ExathError::arg_count("gcd requires 2 arguments"));
            }
            let a = to_integer(eval_real_arg(&args[0], vars, fns, angle_mode, "gcd")?, "gcd")?;
            let b = to_integer(eval_real_arg(&args[1], vars, fns, angle_mode, "gcd")?, "gcd")?;
            Ok(Cx::real(gcd(a.abs(), b.abs()) as f64))
        }

        "lcm" => {
            if args.len() != 2 {
                return Err(ExathError::arg_count("lcm requires 2 arguments"));
            }
            let a = to_integer(eval_real_arg(&args[0], vars, fns, angle_mode, "lcm")?, "lcm")?;
            let b = to_integer(eval_real_arg(&args[1], vars, fns, angle_mode, "lcm")?, "lcm")?;
            let divisor = gcd(a.abs(), b.abs());
            if divisor == 0 {
                return Ok(Cx::real(0.0));
            }
            let result = (a as i128 / divisor as i128 * b as i128).unsigned_abs();
            Ok(Cx::real(result as f64))
        }

        // ── Numerical sum / product / derivative + unit conversion (DSL) ──────
        "sum" | "product" if args.len() == 4 => {
            // sum(expr, var, from, to), integer-stepped accumulation.
            let v = match &args[1] {
                Ast::Var(name) => name.clone(),
                _ => return Err(ExathError::arg_type(format!("{}: 2nd argument must be a variable", name))),
            };
            let from = to_integer(eval_real_arg(&args[2], vars, fns, angle_mode, name)?, name)?;
            let to = to_integer(eval_real_arg(&args[3], vars, fns, angle_mode, name)?, name)?;
            if (to - from).abs() > 10_000_000 {
                return Err(ExathError::domain(format!("{}: range too large", name)));
            }
            let mut acc = if name == "sum" { 0.0 } else { 1.0 };
            let mut local = vars.clone();
            let mut k = from;
            while k <= to {
                local.insert(v.clone(), Cx::real(k as f64));
                let term = eval_ast(&args[0], &local, fns, angle_mode)?.re;
                if name == "sum" { acc += term } else { acc *= term }
                k += 1;
            }
            Ok(Cx::real(acc))
        }
        "deriv" if args.len() == 3 => {
            // deriv(expr, var, x0), central finite difference.
            let v = match &args[1] {
                Ast::Var(name) => name.clone(),
                _ => return Err(ExathError::arg_type("deriv: 2nd argument must be a variable")),
            };
            let x0 = eval_real_arg(&args[2], vars, fns, angle_mode, "deriv")?;
            let h = (x0.abs() * 1e-7).max(1e-10);
            let mut local = vars.clone();
            local.insert(v.clone(), Cx::real(x0 + h));
            let fwd = eval_ast(&args[0], &local, fns, angle_mode)?.re;
            local.insert(v.clone(), Cx::real(x0 - h));
            let bwd = eval_ast(&args[0], &local, fns, angle_mode)?.re;
            Ok(Cx::real((fwd - bwd) / (2.0 * h)))
        }
        "convert" if args.len() == 3 => {
            // convert(value, fromUnit, toUnit), unit names as identifiers.
            let value = eval_real_arg(&args[0], vars, fns, angle_mode, "convert")?;
            let unit_name = |a: &Ast| -> Result<String, ExathError> {
                match a {
                    Ast::Var(n) => Ok(n.clone()),
                    _ => Err(ExathError::arg_type("convert: units must be names, e.g. convert(5, km, m)")),
                }
            };
            let from = unit_name(&args[1])?;
            let to = unit_name(&args[2])?;
            Ok(Cx::real(crate::units::convert(value, &from, &to)?))
        }

        // ── Statistics (variadic, real arguments) ─────────────────────────────
        "mean" | "median" | "variance" | "stddev" => {
            if args.is_empty() {
                return Err(ExathError::arg_count(format!("{} requires at least one argument", name)));
            }
            let mut xs = Vec::with_capacity(args.len());
            for a in args {
                xs.push(eval_real_arg(a, vars, fns, angle_mode, name)?);
            }
            let n = xs.len() as f64;
            let mean = xs.iter().sum::<f64>() / n;
            let value = match name {
                "mean" => mean,
                "median" => {
                    let mut s = xs.clone();
                    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let m = s.len() / 2;
                    if s.len() % 2 == 0 { (s[m - 1] + s[m]) / 2.0 } else { s[m] }
                }
                _ => {
                    // population variance / standard deviation
                    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
                    if name == "variance" { var } else { var.sqrt() }
                }
            };
            Ok(Cx::real(value))
        }

        // ── Distributions & combinatorics ─────────────────────────────────────
        "npdf" | "ncdf" => {
            if args.len() != 3 {
                return Err(ExathError::arg_count(format!(
                    "{} requires 3 arguments: {}(x, mu, sigma)", name, name
                )));
            }
            let x = eval_real_arg(&args[0], vars, fns, angle_mode, name)?;
            let mu = eval_real_arg(&args[1], vars, fns, angle_mode, name)?;
            let sigma = eval_real_arg(&args[2], vars, fns, angle_mode, name)?;
            if sigma <= 0.0 {
                return Err(ExathError::domain(format!("{}: sigma must be positive", name)));
            }
            let z = (x - mu) / sigma;
            let value = if name == "npdf" {
                (-0.5 * z * z).exp() / (sigma * (2.0 * std::f64::consts::PI).sqrt())
            } else {
                let e = apply_function("erf", Cx::real(z / std::f64::consts::SQRT_2), angle_mode)?.re;
                0.5 * (1.0 + e)
            };
            Ok(Cx::real(value))
        }
        "binom" => {
            if args.len() != 2 {
                return Err(ExathError::arg_count("binom requires 2 arguments: binom(n, k)"));
            }
            let n = to_integer(eval_real_arg(&args[0], vars, fns, angle_mode, "binom")?, "binom")?;
            let k = to_integer(eval_real_arg(&args[1], vars, fns, angle_mode, "binom")?, "binom")?;
            if k < 0 || n < 0 || k > n {
                return Ok(Cx::real(0.0));
            }
            let k = k.min(n - k);
            let mut result = 1.0f64;
            for i in 0..k {
                result = result * (n - i) as f64 / (i + 1) as f64;
            }
            Ok(Cx::real(result.round()))
        }
        "beta" => {
            if args.len() != 2 {
                return Err(ExathError::arg_count("beta requires 2 arguments: beta(a, b)"));
            }
            let a = eval_real_arg(&args[0], vars, fns, angle_mode, "beta")?;
            let b = eval_real_arg(&args[1], vars, fns, angle_mode, "beta")?;
            // B(a,b) = Γ(a)Γ(b)/Γ(a+b)
            let ga = apply_function("gamma", Cx::real(a), angle_mode)?.re;
            let gb = apply_function("gamma", Cx::real(b), angle_mode)?.re;
            let gab = apply_function("gamma", Cx::real(a + b), angle_mode)?.re;
            Ok(Cx::real(ga * gb / gab))
        }

        // ── Number theory (integer arguments, within i128 range) ──────────────
        "isprime" => {
            let n = to_integer(eval_real_arg(&args[0], vars, fns, angle_mode, "isprime")?, "isprime")?;
            Ok(Cx::real(if is_prime(n) { 1.0 } else { 0.0 }))
        }
        "nextprime" => {
            let mut n = to_integer(eval_real_arg(&args[0], vars, fns, angle_mode, "nextprime")?, "nextprime")? + 1;
            while !is_prime(n) {
                n += 1;
            }
            Ok(Cx::real(n as f64))
        }
        "totient" => {
            let n = to_integer(eval_real_arg(&args[0], vars, fns, angle_mode, "totient")?, "totient")?;
            if n < 1 {
                return Err(ExathError::domain("totient requires a positive integer"));
            }
            Ok(Cx::real(euler_totient(n) as f64))
        }
        "powmod" => {
            if args.len() != 3 {
                return Err(ExathError::arg_count("powmod requires 3 arguments: powmod(base, exp, m)"));
            }
            let a = to_integer(eval_real_arg(&args[0], vars, fns, angle_mode, "powmod")?, "powmod")?;
            let e = to_integer(eval_real_arg(&args[1], vars, fns, angle_mode, "powmod")?, "powmod")?;
            let m = to_integer(eval_real_arg(&args[2], vars, fns, angle_mode, "powmod")?, "powmod")?;
            if m <= 0 || e < 0 {
                return Err(ExathError::domain("powmod requires modulus > 0 and exponent >= 0"));
            }
            Ok(Cx::real(pow_mod(a, e, m) as f64))
        }

        // All single-argument built-in functions
        _ => {
            if args.len() != 1 {
                return Err(ExathError::arg_count(format!(
                    "'{}' requires exactly 1 argument",
                    name
                )));
            }
            let value = eval_ast(&args[0], vars, fns, angle_mode)?;
            apply_function(name, value, angle_mode)
        }
    }
}

fn eval_real_arg(
    ast: &Ast,
    vars: &HashMap<String, Cx>,
    fns: &UserFns,
    angle_mode: AngleMode,
    fname: &str,
) -> Result<f64, ExathError> {
    let value = eval_ast(ast, vars, fns, angle_mode)?;
    if !value.is_real() {
        return Err(ExathError::arg_type(format!(
            "{} only defined for real arguments",
            fname
        )));
    }
    Ok(value.re)
}

fn cmp_op(left: Cx, right: Cx, compare: impl Fn(f64, f64) -> bool) -> Result<Cx, ExathError> {
    if !left.is_real() || !right.is_real() {
        return Err(ExathError::arg_type(
            "Comparison operators only defined for real numbers",
        ));
    }
    Ok(Cx::real(if compare(left.re, right.re) { 1.0 } else { 0.0 }))
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Deterministic trial-division primality test (fine for i64-range integers).
pub(crate) fn is_prime(n: i64) -> bool {
    if n < 2 {
        return false;
    }
    if n % 2 == 0 {
        return n == 2;
    }
    if n % 3 == 0 {
        return n == 3;
    }
    let mut i: i64 = 5;
    while let Some(sq) = i.checked_mul(i) {
        if sq > n {
            break;
        }
        if n % i == 0 || n % (i + 2) == 0 {
            return false;
        }
        i += 6;
    }
    true
}

/// Euler's totient φ(n) via prime factorisation.
fn euler_totient(mut n: i64) -> i64 {
    let mut result = n;
    let mut p = 2i64;
    while p * p <= n {
        if n % p == 0 {
            while n % p == 0 {
                n /= p;
            }
            result -= result / p;
        }
        p += 1;
    }
    if n > 1 {
        result -= result / n;
    }
    result
}

/// Modular exponentiation (base^exp mod m) using i128 to avoid overflow.
fn pow_mod(base: i64, exp: i64, m: i64) -> i64 {
    let m = m as i128;
    let mut result = 1i128;
    let mut b = (base as i128).rem_euclid(m);
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = result * b % m;
        }
        b = b * b % m;
        e >>= 1;
    }
    result.rem_euclid(m) as i64
}

fn to_integer(x: f64, fname: &str) -> Result<i64, ExathError> {
    if !x.is_finite() {
        return Err(ExathError::arg_type(format!(
            "{} requires finite integer arguments",
            fname
        )));
    }
    let rounded = x.round();
    if (x - rounded).abs() > 1e-9 {
        return Err(ExathError::arg_type(format!(
            "{} requires integer arguments, got {}",
            fname, x
        )));
    }
    if rounded.abs() > 9.007_199_254_740_992e15_f64 {
        return Err(ExathError::overflow(format!(
            "{} argument too large for integer arithmetic",
            fname
        )));
    }
    Ok(rounded as i64)
}

#[cfg(test)]
mod stats_tests {
    use crate::{evaluate, AngleMode};
    fn e(s: &str) -> f64 {
        evaluate(s, AngleMode::Rad).unwrap()
    }
    #[test]
    fn stats_dists_special() {
        // spaces after commas (the engine reads `1,2` as the decimal 1.2)
        assert!((e("mean(1, 2, 3)") - 2.0).abs() < 1e-9);
        assert!((e("median(1, 2, 3, 4)") - 2.5).abs() < 1e-9);
        assert!((e("variance(2, 4, 4, 4, 5, 5, 7, 9)") - 4.0).abs() < 1e-9);
        assert!((e("stddev(2, 4, 4, 4, 5, 5, 7, 9)") - 2.0).abs() < 1e-9);
        assert!((e("binom(5, 2)") - 10.0).abs() < 1e-9);
        assert!((e("beta(2, 3)") - (1.0 / 12.0)).abs() < 1e-6);
        assert!((e("ncdf(0, 0, 1)") - 0.5).abs() < 1e-6);
        assert!((e("npdf(0, 0, 1)") - 0.3989422804).abs() < 1e-6);
        assert!((e("digamma(1)") + 0.5772156649).abs() < 1e-6); // ψ(1) = -γ
    }
}
