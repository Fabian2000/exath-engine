//! Symbolic differentiation over the expression AST.
//!
//! This is purely additive: it reuses the existing parser and `Ast` and never
//! touches the numeric evaluator. `differentiate` parses an expression,
//! differentiates it with respect to a variable, simplifies the result and
//! renders it back to an expression string.
//!
//! Symbolic calculus here is defined in the standard (radian) mathematical
//! sense, independent of any evaluation angle mode — `d/dx sin(x) = cos(x)`.
//!
//! Panic-free by contract: no `unwrap`, `expect` or `panic!`; every fallible
//! path returns `ExathError`.

use crate::ast::{parse_str, Ast, BinOp};
use crate::error::ExathError;

/// Differentiate `expr` with respect to `var` and return the simplified
/// derivative as an expression string.
pub fn differentiate(expr: &str, var: &str) -> Result<String, ExathError> {
    let ast = parse_str(expr)?;
    let d = diff(&ast, var)?;
    Ok(unparse(&simplify(d)))
}

/// Parse and simplify an expression without differentiating it.
pub fn simplify_expr(expr: &str) -> Result<String, ExathError> {
    let ast = parse_str(expr)?;
    Ok(unparse(&simplify(ast)))
}

// ── Differentiation ───────────────────────────────────────────────────────────

fn boxed(a: Ast) -> Box<Ast> {
    Box::new(a)
}

fn num(n: f64) -> Ast {
    Ast::Number(n)
}

fn add(a: Ast, b: Ast) -> Ast {
    Ast::BinOp(BinOp::Add, boxed(a), boxed(b))
}

fn sub(a: Ast, b: Ast) -> Ast {
    Ast::BinOp(BinOp::Sub, boxed(a), boxed(b))
}

fn mul(a: Ast, b: Ast) -> Ast {
    Ast::BinOp(BinOp::Mul, boxed(a), boxed(b))
}

fn div(a: Ast, b: Ast) -> Ast {
    Ast::BinOp(BinOp::Div, boxed(a), boxed(b))
}

fn pow(a: Ast, b: Ast) -> Ast {
    Ast::BinOp(BinOp::Pow, boxed(a), boxed(b))
}

fn call1(name: &str, a: Ast) -> Ast {
    Ast::Call(name.to_string(), vec![a])
}

/// True if `ast` references the variable `var` anywhere.
fn contains_var(ast: &Ast, var: &str) -> bool {
    match ast {
        Ast::Number(_) => false,
        Ast::Var(name) => name == var,
        Ast::BinOp(_, l, r) => contains_var(l, var) || contains_var(r, var),
        Ast::UnaryNeg(u) | Ast::UnaryNot(u) | Ast::Factorial(u) => contains_var(u, var),
        Ast::Call(_, args) => args.iter().any(|a| contains_var(a, var)),
    }
}

fn diff(ast: &Ast, var: &str) -> Result<Ast, ExathError> {
    match ast {
        // d/dx c = 0 ; d/dx y = 0 for y != x (constants/parameters)
        Ast::Number(_) => Ok(num(0.0)),
        Ast::Var(name) => Ok(num(if name == var { 1.0 } else { 0.0 })),

        Ast::UnaryNeg(u) => Ok(Ast::UnaryNeg(boxed(diff(u, var)?))),

        Ast::BinOp(op, l, r) => diff_binop(op, l, r, var),

        Ast::Call(name, args) => diff_call(name, args, var),

        // Not differentiable as a continuous function.
        Ast::Factorial(_) => Err(ExathError::domain(
            "symbolic derivative of factorial is not supported",
        )),
        Ast::UnaryNot(_) => Err(ExathError::domain(
            "symbolic derivative of a logical expression is not supported",
        )),
    }
}

fn diff_binop(op: &BinOp, l: &Ast, r: &Ast, var: &str) -> Result<Ast, ExathError> {
    match op {
        BinOp::Add => Ok(add(diff(l, var)?, diff(r, var)?)),
        BinOp::Sub => Ok(sub(diff(l, var)?, diff(r, var)?)),
        // product rule: (l*r)' = l'*r + l*r'
        BinOp::Mul => Ok(add(
            mul(diff(l, var)?, r.clone()),
            mul(l.clone(), diff(r, var)?),
        )),
        // quotient rule: (l/r)' = (l'*r - l*r') / r^2
        BinOp::Div => Ok(div(
            sub(mul(diff(l, var)?, r.clone()), mul(l.clone(), diff(r, var)?)),
            pow(r.clone(), num(2.0)),
        )),
        BinOp::Pow => diff_pow(l, r, var),
        _ => Err(ExathError::domain(
            "symbolic derivative of comparison/logical/modulo operators is not supported",
        )),
    }
}

fn diff_pow(base: &Ast, exp: &Ast, var: &str) -> Result<Ast, ExathError> {
    let base_has = contains_var(base, var);
    let exp_has = contains_var(exp, var);
    match (base_has, exp_has) {
        // constant^constant  → 0
        (false, false) => Ok(num(0.0)),
        // f(x)^c  → c * f^(c-1) * f'
        (true, false) => Ok(mul(
            mul(exp.clone(), pow(base.clone(), sub(exp.clone(), num(1.0)))),
            diff(base, var)?,
        )),
        // c^g(x)  → c^g * ln(c) * g'
        (false, true) => Ok(mul(
            mul(pow(base.clone(), exp.clone()), call1("ln", base.clone())),
            diff(exp, var)?,
        )),
        // f(x)^g(x) → f^g * (g'*ln(f) + g*f'/f)
        (true, true) => Ok(mul(
            pow(base.clone(), exp.clone()),
            add(
                mul(diff(exp, var)?, call1("ln", base.clone())),
                div(mul(exp.clone(), diff(base, var)?), base.clone()),
            ),
        )),
    }
}

/// Chain rule for single-argument built-in functions: d/dx f(u) = f'(u) * u'.
fn diff_call(name: &str, args: &[Ast], var: &str) -> Result<Ast, ExathError> {
    if args.len() != 1 {
        return Err(ExathError::domain(format!(
            "symbolic derivative of '{}' with {} arguments is not supported",
            name,
            args.len()
        )));
    }
    let u = &args[0];
    let du = diff(u, var)?;
    let outer = outer_derivative(name, u)?;
    Ok(mul(outer, du))
}

/// f'(u) expressed symbolically for a known function `name` applied to `u`.
fn outer_derivative(name: &str, u: &Ast) -> Result<Ast, ExathError> {
    let d = match name {
        "sin" => call1("cos", u.clone()),
        "cos" => Ast::UnaryNeg(boxed(call1("sin", u.clone()))),
        // d/dx tan(u) = 1 / cos(u)^2
        "tan" => div(num(1.0), pow(call1("cos", u.clone()), num(2.0))),
        "exp" => call1("exp", u.clone()),
        // natural log
        "ln" => div(num(1.0), u.clone()),
        // base-10 log: d/dx log10(u) = 1 / (u * ln(10))
        "log" | "lg" => div(num(1.0), mul(u.clone(), num(std::f64::consts::LN_10))),
        // d/dx sqrt(u) = 1 / (2*sqrt(u))
        "sqrt" => div(num(1.0), mul(num(2.0), call1("sqrt", u.clone()))),
        // d/dx asin(u) = 1 / sqrt(1 - u^2)
        "asin" => div(num(1.0), call1("sqrt", sub(num(1.0), pow(u.clone(), num(2.0))))),
        "acos" => Ast::UnaryNeg(boxed(div(
            num(1.0),
            call1("sqrt", sub(num(1.0), pow(u.clone(), num(2.0)))),
        ))),
        // d/dx atan(u) = 1 / (1 + u^2)
        "atan" => div(num(1.0), add(num(1.0), pow(u.clone(), num(2.0)))),
        "sinh" => call1("cosh", u.clone()),
        "cosh" => call1("sinh", u.clone()),
        // d/dx tanh(u) = 1 / cosh(u)^2
        "tanh" => div(num(1.0), pow(call1("cosh", u.clone()), num(2.0))),
        // d/dx |u| = sign(u)
        "abs" => call1("sign", u.clone()),
        _ => {
            return Err(ExathError::domain(format!(
                "symbolic derivative of function '{}' is not supported",
                name
            )))
        }
    };
    Ok(d)
}

// ── Simplification ──────────────────────────────────────────────────────────

fn is_num(a: &Ast, v: f64) -> bool {
    matches!(a, Ast::Number(n) if (*n - v).abs() < f64::EPSILON)
}

fn as_num(a: &Ast) -> Option<f64> {
    match a {
        Ast::Number(n) => Some(*n),
        _ => None,
    }
}

fn simplify(ast: Ast) -> Ast {
    match ast {
        Ast::BinOp(op, l, r) => simplify_binop(op, simplify(*l), simplify(*r)),
        Ast::UnaryNeg(u) => {
            let s = simplify(*u);
            match s {
                Ast::Number(n) => Ast::Number(-n),
                Ast::UnaryNeg(inner) => *inner,
                other => Ast::UnaryNeg(boxed(other)),
            }
        }
        Ast::UnaryNot(u) => Ast::UnaryNot(boxed(simplify(*u))),
        Ast::Factorial(u) => Ast::Factorial(boxed(simplify(*u))),
        Ast::Call(name, args) => Ast::Call(name, args.into_iter().map(simplify).collect()),
        leaf => leaf,
    }
}

fn simplify_binop(op: BinOp, l: Ast, r: Ast) -> Ast {
    match op {
        BinOp::Add => {
            if is_num(&l, 0.0) {
                return r;
            }
            if is_num(&r, 0.0) {
                return l;
            }
            if let (Some(a), Some(b)) = (as_num(&l), as_num(&r)) {
                return num(a + b);
            }
            add(l, r)
        }
        BinOp::Sub => {
            if is_num(&r, 0.0) {
                return l;
            }
            if let (Some(a), Some(b)) = (as_num(&l), as_num(&r)) {
                return num(a - b);
            }
            if is_num(&l, 0.0) {
                return simplify(Ast::UnaryNeg(boxed(r)));
            }
            sub(l, r)
        }
        BinOp::Mul => {
            if is_num(&l, 0.0) || is_num(&r, 0.0) {
                return num(0.0);
            }
            if is_num(&l, 1.0) {
                return r;
            }
            if is_num(&r, 1.0) {
                return l;
            }
            if let (Some(a), Some(b)) = (as_num(&l), as_num(&r)) {
                return num(a * b);
            }
            mul(l, r)
        }
        BinOp::Div => {
            if is_num(&l, 0.0) {
                return num(0.0);
            }
            if is_num(&r, 1.0) {
                return l;
            }
            if let (Some(a), Some(b)) = (as_num(&l), as_num(&r)) {
                if b != 0.0 {
                    return num(a / b);
                }
            }
            div(l, r)
        }
        BinOp::Pow => {
            if is_num(&r, 0.0) {
                return num(1.0);
            }
            if is_num(&r, 1.0) {
                return l;
            }
            if is_num(&l, 1.0) {
                return num(1.0);
            }
            if let (Some(a), Some(b)) = (as_num(&l), as_num(&r)) {
                let p = a.powf(b);
                if p.is_finite() {
                    return num(p);
                }
            }
            pow(l, r)
        }
        other => Ast::BinOp(other, boxed(l), boxed(r)),
    }
}

// ── Pretty printer (AST → expression string) ──────────────────────────────────

fn fmt_num(x: f64) -> String {
    if x.is_finite() && x == x.trunc() && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{}", x)
    }
}

fn prec(a: &Ast) -> u8 {
    match a {
        Ast::Number(_) | Ast::Var(_) | Ast::Call(_, _) | Ast::Factorial(_) => 5,
        Ast::UnaryNeg(_) | Ast::UnaryNot(_) => 4,
        Ast::BinOp(op, _, _) => match op {
            BinOp::Pow => 3,
            BinOp::Mul | BinOp::Div | BinOp::Mod => 2,
            BinOp::Add | BinOp::Sub => 1,
            _ => 0,
        },
    }
}

fn op_symbol(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => " + ",
        BinOp::Sub => " - ",
        BinOp::Mul => " * ",
        BinOp::Div => " / ",
        BinOp::Pow => "^",
        BinOp::Mod => " mod ",
        BinOp::Eq => " == ",
        BinOp::Ne => " != ",
        BinOp::Lt => " < ",
        BinOp::Le => " <= ",
        BinOp::Gt => " > ",
        BinOp::Ge => " >= ",
        BinOp::And => " && ",
        BinOp::Or => " || ",
    }
}

/// Render `child` parenthesised if its precedence is below `min_prec`.
fn paren(child: &Ast, min_prec: u8) -> String {
    if prec(child) < min_prec {
        format!("({})", unparse(child))
    } else {
        unparse(child)
    }
}

fn unparse(a: &Ast) -> String {
    match a {
        Ast::Number(n) => fmt_num(*n),
        Ast::Var(name) => name.clone(),
        Ast::UnaryNeg(u) => format!("-{}", paren(u, 4)),
        Ast::UnaryNot(u) => format!("!{}", paren(u, 4)),
        Ast::Factorial(u) => format!("{}!", paren(u, 5)),
        Ast::Call(name, args) => {
            let inner: Vec<String> = args.iter().map(unparse).collect();
            format!("{}({})", name, inner.join(", "))
        }
        Ast::BinOp(op, l, r) => {
            let p = prec(a);
            let (lmin, rmin) = match op {
                // right-associative power: left needs parens at equal precedence
                BinOp::Pow => (p + 1, p),
                // left-associative, non-commutative: right needs parens at equal precedence
                BinOp::Sub | BinOp::Div | BinOp::Mod => (p, p + 1),
                // commutative / associative
                _ => (p, p),
            };
            format!("{}{}{}", paren(l, lmin), op_symbol(op), paren(r, rmin))
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AngleMode, CalcResult, Session};

    /// Evaluate an expression string with `var = x` and return the real part.
    fn eval_at(expr: &str, var: &str, x: f64) -> f64 {
        let mut s = Session::new(AngleMode::Rad);
        s.set_var(var, x, 0.0);
        match s.eval(expr) {
            Ok(CalcResult::Real(v)) => v,
            Ok(CalcResult::Complex(re, _)) => re,
            Err(e) => {
                assert!(false, "eval failed for '{}': {}", expr, e);
                f64::NAN
            }
        }
    }

    /// Assert the symbolic derivative matches the numeric derivative at a point.
    fn check(expr: &str, var: &str, x: f64) {
        let d = match differentiate(expr, var) {
            Ok(s) => s,
            Err(e) => {
                assert!(false, "differentiate failed for '{}': {}", expr, e);
                return;
            }
        };
        let symbolic = eval_at(&d, var, x);
        let numeric = match crate::deriv(expr, var, x, AngleMode::Rad) {
            Ok(v) => v,
            Err(e) => {
                assert!(false, "numeric deriv failed for '{}': {}", expr, e);
                return;
            }
        };
        assert!(
            (symbolic - numeric).abs() < 1e-4,
            "d/d{} {} at {} = '{}' -> {}, numeric {}",
            var,
            expr,
            x,
            d,
            symbolic,
            numeric
        );
    }

    #[test]
    fn polynomials() {
        check("x^2", "x", 3.0);
        check("x^3 + 2*x", "x", 1.7);
        check("5", "x", 2.0);
        check("3*x^2 - x + 7", "x", -2.0);
    }

    #[test]
    fn products_and_quotients() {
        check("x*sin(x)", "x", 1.1);
        check("x / (x + 1)", "x", 2.0);
        check("(x^2 + 1) / x", "x", 3.0);
    }

    #[test]
    fn functions() {
        check("sin(x)", "x", 0.7);
        check("cos(x)", "x", 0.7);
        check("tan(x)", "x", 0.3);
        check("ln(x)", "x", 2.0);
        check("exp(x)", "x", 1.3);
        check("sqrt(x)", "x", 4.0);
        check("atan(x)", "x", 0.9);
    }

    #[test]
    fn chain_rule() {
        check("sin(x^2)", "x", 1.2);
        check("exp(2*x)", "x", 0.6);
        check("ln(x^2 + 1)", "x", 1.5);
        check("sqrt(x^2 + 1)", "x", 2.0);
    }

    #[test]
    fn exponentials() {
        check("2^x", "x", 3.0);
        check("x^x", "x", 1.5);
    }

    #[test]
    fn unsupported_is_error_not_panic() {
        assert!(differentiate("x!", "x").is_err());
        assert!(differentiate("x > 1", "x").is_err());
        assert!(differentiate("gcd(x, 2)", "x").is_err());
    }

    #[test]
    fn simplify_is_clean() {
        // exact string for a well-known simple case
        match differentiate("x^2", "x") {
            Ok(s) => assert_eq!(s, "2 * x"),
            Err(e) => assert!(false, "{}", e),
        }
    }
}
