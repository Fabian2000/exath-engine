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

use crate::ast::{collect_vars, eval_ast, parse_str, Ast, BinOp, UserFns};
use crate::error::ExathError;
use crate::evaluator::Cx;
use crate::rational::Num;
use crate::AngleMode;
use std::collections::{BTreeMap, HashMap};

/// Guard against runaway recursion when inlining (mutually) recursive user
/// function definitions. Panic-free: exceeding it returns an error.
const INLINE_DEPTH_LIMIT: usize = 64;

/// Differentiate `expr` with respect to `var` and return the simplified
/// derivative as an expression string.
pub fn differentiate(expr: &str, var: &str) -> Result<String, ExathError> {
    Ok(render(&differentiate_ast(&parse_str(expr)?, var)?))
}

/// Parse and simplify an expression without differentiating it.
pub fn simplify_expr(expr: &str) -> Result<String, ExathError> {
    Ok(render(&simplify_ast(parse_str(expr)?)))
}

/// Differentiate an AST w.r.t. `var`, returning the simplified derivative AST.
pub fn differentiate_ast(ast: &Ast, var: &str) -> Result<Ast, ExathError> {
    Ok(simplify_ast(diff(ast, var)?))
}

/// Indefinite integral of `expr` w.r.t. `var`, returned as an expression string
/// (constant of integration omitted). Handles polynomials, `c·x^n`, `1/x`, and
/// `sin/cos/exp` of a linear argument `a·x+b`; returns an error for forms it
/// cannot integrate in closed form.
pub fn antiderivative(expr: &str, var: &str) -> Result<String, ExathError> {
    Ok(render(&integrate_ast(&parse_str(expr)?, var)?))
}

/// Definite integral ∫ₐᵇ expr d(var) = F(b) − F(a).
pub fn integrate_definite(expr: &str, var: &str, a: f64, b: f64) -> Result<String, ExathError> {
    Ok(render(&integrate_definite_ast(&parse_str(expr)?, var, a, b)?))
}

/// Definite integral at the AST level (see [`integrate_definite`]).
pub fn integrate_definite_ast(f: &Ast, var: &str, a: f64, b: f64) -> Result<Ast, ExathError> {
    // Preferred: exact via the antiderivative (fundamental theorem).
    if let Ok(anti) = integrate_ast(f, var) {
        let fb = substitute(&anti, var, &num(b));
        let fa = substitute(&anti, var, &num(a));
        let result = simplify_ast(sub(fb, fa));
        // Use it only if it evaluates to a finite number (the antiderivative may
        // be undefined at an endpoint, e.g. a singularity).
        if let Ok(v) = eval_const_f64(&result) {
            if v.is_finite() {
                return Ok(result);
            }
        }
    }
    // Fallback: adaptive Simpson quadrature — so a definite integral over a
    // well-behaved integrand always returns a value.
    let g = |x: f64| -> Option<f64> {
        eval_const_f64(&substitute(f, var, &num(x)))
            .ok()
            .filter(|v| v.is_finite())
    };
    match adaptive_simpson(&g, a, b) {
        Some(v) => Ok(num((v * 1e10).round() / 1e10)),
        None => Err(ExathError::domain(
            "definite integral: integrand not finite on the interval",
        )),
    }
}

/// Adaptive Simpson's rule over [a, b]; returns None if the integrand is not
/// finite somewhere it is sampled.
fn adaptive_simpson(f: &dyn Fn(f64) -> Option<f64>, a: f64, b: f64) -> Option<f64> {
    fn simpson(_f: &dyn Fn(f64) -> Option<f64>, a: f64, b: f64, fa: f64, fb: f64, fm: f64) -> Option<f64> {
        Some((b - a) / 6.0 * (fa + 4.0 * fm + fb))
    }
    fn rec(
        f: &dyn Fn(f64) -> Option<f64>,
        a: f64,
        b: f64,
        fa: f64,
        fb: f64,
        fm: f64,
        whole: f64,
        depth: u32,
    ) -> Option<f64> {
        let m = 0.5 * (a + b);
        let lm = 0.5 * (a + m);
        let rm = 0.5 * (m + b);
        let flm = f(lm)?;
        let frm = f(rm)?;
        let left = simpson(f, a, m, fa, fm, flm)?;
        let right = simpson(f, m, b, fm, fb, frm)?;
        if depth == 0 || (left + right - whole).abs() < 1e-11 * (1.0 + whole.abs()) {
            return Some(left + right + (left + right - whole) / 15.0);
        }
        Some(rec(f, a, m, fa, fm, flm, left, depth - 1)? + rec(f, m, b, fm, fb, frm, right, depth - 1)?)
    }
    if a == b {
        return Some(0.0);
    }
    let (lo, hi, sign) = if a < b { (a, b, 1.0) } else { (b, a, -1.0) };
    let fa = f(lo)?;
    let fb = f(hi)?;
    let fm = f(0.5 * (lo + hi))?;
    let whole = simpson(f, lo, hi, fa, fb, fm)?;
    Some(sign * rec(f, lo, hi, fa, fb, fm, whole, 50)?)
}

/// Indefinite integral at the AST level (see [`antiderivative`]).
pub fn integrate_ast(ast: &Ast, var: &str) -> Result<Ast, ExathError> {
    match integrate_curated(ast, var) {
        Ok(r) => Ok(r),
        // Fall back to verified u-substitution; only returns a result whose
        // derivative provably equals the integrand.
        Err(e) => try_substitution(ast, var).ok_or(e),
    }
}

/// Term-by-term integration using the curated rule set.
fn integrate_curated(ast: &Ast, var: &str) -> Result<Ast, ExathError> {
    let p = build(ast)?;
    let mut acc: Option<Ast> = None;
    for t in p.terms.values() {
        let term_int = integrate_term(t, var)?;
        acc = Some(match acc {
            None => term_int,
            Some(a) => add(a, term_int),
        });
    }
    Ok(simplify_ast(acc.unwrap_or_else(|| num(0.0))))
}

/// Attempt ∫ expr dx via u-substitution: for each composite sub-expression
/// u = g(x), test whether expr/g'(x) becomes a function of u alone; if so
/// integrate in u and back-substitute. The result is accepted ONLY if its
/// derivative numerically equals the integrand (so a returned answer is correct).
fn try_substitution(expr: &Ast, var: &str) -> Option<Ast> {
    let u = fresh_var(expr, var);
    for g in substitution_candidates(expr, var) {
        let gp = differentiate_ast(&g, var).ok()?;
        if matches!(gp, Ast::Number(n) if n.abs() < 1e-12) {
            continue;
        }
        // q = expr / g'
        let q = simplify_ast(div(expr.clone(), gp));
        // replace every occurrence of g by the fresh variable u
        let q_u = replace_subtree(&q, &render(&g), &Ast::Var(u.clone()));
        // q must now be free of the original variable
        if crate::ast::collect_vars(&q_u).iter().any(|v| v == var) {
            continue;
        }
        // ∫ h(u) du, then back-substitute u → g
        if let Ok(inner) = integrate_curated(&q_u, &u) {
            let result = simplify_ast(substitute(&inner, &u, &g));
            if verify_integral(expr, &result, var) {
                return Some(result);
            }
        }
    }
    None
}

/// Collect composite sub-expressions that contain `var` (candidate u = g(x)),
/// deduplicated and ordered innermost-first.
fn substitution_candidates(ast: &Ast, var: &str) -> Vec<Ast> {
    let mut out: Vec<Ast> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    collect_candidates(ast, var, &mut out, &mut seen);
    out
}

fn collect_candidates(ast: &Ast, var: &str, out: &mut Vec<Ast>, seen: &mut std::collections::BTreeSet<String>) {
    match ast {
        Ast::Number(_) | Ast::Var(_) => {}
        Ast::BinOp(_, l, r) => {
            collect_candidates(l, var, out, seen);
            collect_candidates(r, var, out, seen);
            push_candidate(ast, var, out, seen);
        }
        Ast::UnaryNeg(u) | Ast::UnaryNot(u) | Ast::Factorial(u) => {
            collect_candidates(u, var, out, seen);
            push_candidate(ast, var, out, seen);
        }
        Ast::Call(_, args) => {
            for a in args {
                collect_candidates(a, var, out, seen);
                // a function's argument is a prime substitution candidate
                push_candidate(a, var, out, seen);
            }
            // …and the function application itself (e.g. u = sin(x))
            push_candidate(ast, var, out, seen);
        }
        Ast::Matrix(_) => {}
    }
}

fn push_candidate(ast: &Ast, var: &str, out: &mut Vec<Ast>, seen: &mut std::collections::BTreeSet<String>) {
    if matches!(ast, Ast::Var(_) | Ast::Number(_)) {
        return;
    }
    if !crate::ast::collect_vars(ast).iter().any(|v| v == var) {
        return;
    }
    let key = render(ast);
    if seen.insert(key) {
        out.push(ast.clone());
    }
}

/// Replace every sub-expression whose rendering equals `target` with `repl`.
fn replace_subtree(ast: &Ast, target: &str, repl: &Ast) -> Ast {
    if render(ast) == target {
        return repl.clone();
    }
    match ast {
        Ast::Number(_) | Ast::Var(_) => ast.clone(),
        Ast::BinOp(op, l, r) => Ast::BinOp(
            op.clone(),
            boxed(replace_subtree(l, target, repl)),
            boxed(replace_subtree(r, target, repl)),
        ),
        Ast::UnaryNeg(u) => Ast::UnaryNeg(boxed(replace_subtree(u, target, repl))),
        Ast::UnaryNot(u) => Ast::UnaryNot(boxed(replace_subtree(u, target, repl))),
        Ast::Factorial(u) => Ast::Factorial(boxed(replace_subtree(u, target, repl))),
        Ast::Call(name, args) => Ast::Call(
            name.clone(),
            args.iter().map(|a| replace_subtree(a, target, repl)).collect(),
        ),
        Ast::Matrix(rows) => Ast::Matrix(
            rows.iter()
                .map(|r| r.iter().map(|e| replace_subtree(e, target, repl)).collect())
                .collect(),
        ),
    }
}

/// A variable name not occurring in `ast` (for the substitution variable).
fn fresh_var(ast: &Ast, var: &str) -> String {
    let vars = crate::ast::collect_vars(ast);
    for cand in ["u", "u_", "u__", "usub"] {
        if cand != var && !vars.iter().any(|v| v == cand) {
            return cand.to_string();
        }
    }
    "u_sub_var".to_string()
}

/// Numerically verify that d/dx(result) == integrand at sample points.
fn verify_integral(integrand: &Ast, result: &Ast, var: &str) -> bool {
    let d = match differentiate_ast(result, var) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let mut checked = 0;
    for x in [0.37, 0.81, 1.23, 1.9, 2.4] {
        let a = eval_const_f64(&substitute(integrand, var, &num(x)));
        let b = eval_const_f64(&substitute(&d, var, &num(x)));
        if let (Ok(a), Ok(b)) = (a, b) {
            if a.is_finite() && b.is_finite() {
                if (a - b).abs() > 1e-6 * (1.0 + a.abs()) {
                    return false;
                }
                checked += 1;
            }
        }
    }
    checked >= 2
}

fn factor_has_var(f: &Factor, var: &str) -> bool {
    crate::ast::collect_vars(&rebuild_factor(f))
        .iter()
        .any(|v| v == var)
}

fn is_zero_ast(a: &Ast) -> bool {
    matches!(a, Ast::Number(n) if n.abs() < 1e-12)
}

/// Integrate a single monomial term w.r.t. `var`.
fn integrate_term(t: &Term, var: &str) -> Result<Ast, ExathError> {
    let mut const_factors: BTreeMap<String, (Factor, f64)> = BTreeMap::new();
    let mut var_factors: Vec<(&Factor, f64)> = Vec::new();
    for (k, (f, e)) in &t.factors {
        if factor_has_var(f, var) {
            var_factors.push((f, *e));
        } else {
            const_factors.insert(k.clone(), (f.clone(), *e));
        }
    }

    // K = constant multiplier (coeff × constant factors).
    let k_term = Term { coeff: t.coeff, factors: const_factors };
    let (k_mag, k_neg) = term_to_ast(&k_term);
    let k_ast = if k_neg { neg_ast(k_mag) } else { k_mag };
    let var_ast = Ast::Var(var.to_string());

    // ∫ K dx = K·x
    if var_factors.is_empty() {
        return Ok(mul(k_ast, var_ast));
    }

    if var_factors.len() == 1 {
        let (f, e) = var_factors[0];

        // ∫ K·sin(u)²/cos(u)² dx for linear u (power-reduction formulae):
        // ∫sin²(u) = x/2 − sin(2u)/(4a),  ∫cos²(u) = x/2 + sin(2u)/(4a).
        if (clean(e) - 2.0).abs() < 1e-12 {
            if let Factor::Func(name, fargs) = f {
                if fargs.len() == 1 && matches!(name.as_str(), "sin" | "cos") {
                    let arg = rebuild_poly(&fargs[0]);
                    if let Some(a_ast) = linear_slope(&arg, var) {
                        let half_x = div(var_ast.clone(), num(2.0));
                        let s2 = div(
                            call1("sin", mul(num(2.0), arg)),
                            mul(num(4.0), a_ast),
                        );
                        let body = if name == "sin" {
                            sub(half_x, s2)
                        } else {
                            add(half_x, s2)
                        };
                        return Ok(mul(k_ast, body));
                    }
                }
                // ∫ sec²(u) = tan(u)/a,  ∫ csc²(u) = −cot(u)/a
                if fargs.len() == 1 && matches!(name.as_str(), "sec" | "csc") {
                    let arg = rebuild_poly(&fargs[0]);
                    if let Some(a_ast) = linear_slope(&arg, var) {
                        let body = if name == "sec" {
                            call1("tan", arg)
                        } else {
                            neg_ast(call1("cot", arg))
                        };
                        return Ok(div(mul(k_ast, body), a_ast));
                    }
                }
            }
        }

        // Power rule: ∫ K·x^n dx
        if let Factor::Var(name) = f {
            if name == var {
                let n = clean(e);
                if (n + 1.0).abs() < 1e-12 {
                    return Ok(mul(k_ast, call1("ln", var_ast))); // ∫ x^-1 = ln(x)
                }
                let np1 = clean(n + 1.0);
                return Ok(div(mul(k_ast, pow(var_ast, num(np1))), num(np1)));
            }
        }

        // ∫ K·f(a·x+b) dx for f in {sin, cos, exp}, linear argument.
        if (e - 1.0).abs() < 1e-12 {
            if let Factor::Func(name, args) = f {
                if args.len() == 1 {
                    let arg = rebuild_poly(&args[0]);
                    let a_ast = linear_slope(&arg, var).ok_or_else(|| {
                        ExathError::domain("cannot integrate: non-linear function argument")
                    })?;
                    // ∫ f(a·x+b) dx = G(a·x+b)/a, where G is the antiderivative of f.
                    let antideriv = match name.as_str() {
                        "sin" => neg_ast(call1("cos", arg.clone())),
                        "cos" => call1("sin", arg.clone()),
                        "exp" => call1("exp", arg.clone()),
                        // ∫ ln(u) du = u·ln(u) − u
                        "ln" => sub(mul(arg.clone(), call1("ln", arg.clone())), arg.clone()),
                        // ∫ tan(u) du = −ln(cos(u))
                        "tan" => neg_ast(call1("ln", call1("cos", arg.clone()))),
                        // ∫ cot(u) du = ln(sin(u))
                        "cot" => call1("ln", call1("sin", arg.clone())),
                        // ∫ sec(u) du = ln(sec(u) + tan(u))
                        "sec" => call1(
                            "ln",
                            add(call1("sec", arg.clone()), call1("tan", arg.clone())),
                        ),
                        // ∫ csc(u) du = −ln(csc(u) + cot(u))
                        "csc" => neg_ast(call1(
                            "ln",
                            add(call1("csc", arg.clone()), call1("cot", arg.clone())),
                        )),
                        _ => {
                            return Err(ExathError::domain(format!(
                                "no symbolic antiderivative for '{}'",
                                name
                            )))
                        }
                    };
                    return Ok(div(mul(k_ast, antideriv), a_ast));
                }
            }
        }

        // ∫ K·(a·x+b)^e dx — a sum (or reciprocal) raised to a power.
        // e == -1 → K·ln(a·x+b)/a ;  else → K·(a·x+b)^(e+1)/(a·(e+1)).
        if let Factor::SumBase(bp) = f {
            // ∫ K/(A·x² + C) dx = K/√(A·C) · atan(x·√(A/C))  (A, C > 0)
            if (clean(e) + 1.0).abs() < 1e-12 {
                if let Some(cs) = poly_coeffs(bp, var) {
                    let pure_quadratic =
                        cs.contains_key(&2) && cs.keys().all(|k| *k == 0 || *k == 2);
                    if pure_quadratic {
                        let a_coef = cs.get(&2).copied().unwrap_or_else(Num::zero);
                        let c_coef = cs.get(&0).copied().unwrap_or_else(Num::zero);
                        if a_coef.to_f64() > 0.0 && c_coef.to_f64() > 0.0 {
                            let inner = mul(var_ast.clone(), num_sqrt_ast(&a_coef.div(&c_coef)));
                            return Ok(div(
                                mul(k_ast, call1("atan", inner)),
                                num_sqrt_ast(&a_coef.mul(&c_coef)),
                            ));
                        }
                    }
                }
            }
            let base = rebuild_poly(bp);
            if let Some(a_ast) = linear_slope(&base, var) {
                let n = clean(e);
                if (n + 1.0).abs() < 1e-12 {
                    return Ok(div(mul(k_ast, call1("ln", base)), a_ast));
                }
                let np1 = clean(n + 1.0);
                return Ok(div(
                    mul(k_ast, pow(base, num(np1))),
                    mul(a_ast, num(np1)),
                ));
            }
        }
    }

    // Integration by parts: ∫ K·x^n·f(a·x+b) dx for f ∈ {sin, cos, exp}, n ≥ 1.
    if var_factors.len() == 2 {
        let mut x_pow: Option<i64> = None;
        let mut elem: Option<(String, Ast)> = None;
        for (f, e) in &var_factors {
            match f {
                Factor::Var(name) if name == var => {
                    let n = clean(*e);
                    if n.fract() == 0.0 && n >= 1.0 {
                        x_pow = Some(n as i64);
                    }
                }
                Factor::Func(name, args)
                    if (e - &1.0).abs() < 1e-12
                        && args.len() == 1
                        && matches!(name.as_str(), "sin" | "cos" | "exp") =>
                {
                    elem = Some((name.clone(), rebuild_poly(&args[0])));
                }
                _ => {}
            }
        }
        if let (Some(n), Some((name, arg))) = (x_pow, elem) {
            if let Some(a_ast) = linear_slope(&arg, var) {
                return Ok(mul(k_ast, integrate_by_parts(n, &name, &arg, &a_ast, var)));
            }
        }
        // ∫ K·e^{u}·sin(v) / e^{u}·cos(v) dx with u, v linear (cyclic by parts):
        // ∫e^{ax}sin(bx) = e^{ax}(a sin − b cos)/(a²+b²),
        // ∫e^{ax}cos(bx) = e^{ax}(a cos + b sin)/(a²+b²).
        {
            let mut exp_arg: Option<Ast> = None;
            let mut trig: Option<(String, Ast)> = None;
            for (f, e) in &var_factors {
                if (e - &1.0).abs() < 1e-12 {
                    if let Factor::Func(name, args) = f {
                        if args.len() == 1 {
                            let a = rebuild_poly(&args[0]);
                            match name.as_str() {
                                "exp" => exp_arg = Some(a),
                                "sin" | "cos" => trig = Some((name.clone(), a)),
                                _ => {}
                            }
                        }
                    }
                }
            }
            if let (Some(uarg), Some((tname, varg))) = (exp_arg, trig) {
                if let (Some(a_ast), Some(b_ast)) =
                    (linear_slope(&uarg, var), linear_slope(&varg, var))
                {
                    let denom = add(
                        mul(a_ast.clone(), a_ast.clone()),
                        mul(b_ast.clone(), b_ast.clone()),
                    );
                    let body = if tname == "sin" {
                        sub(
                            mul(a_ast.clone(), call1("sin", varg.clone())),
                            mul(b_ast.clone(), call1("cos", varg.clone())),
                        )
                    } else {
                        add(
                            mul(a_ast.clone(), call1("cos", varg.clone())),
                            mul(b_ast.clone(), call1("sin", varg.clone())),
                        )
                    };
                    return Ok(mul(k_ast, div(mul(call1("exp", uarg), body), denom)));
                }
            }
        }
        // ∫ K·x^n·ln(x) dx = K·[ x^(n+1)/(n+1)·ln(x) − x^(n+1)/(n+1)² ]
        let mut x_pow_ln: Option<i64> = None;
        let mut has_ln_x = false;
        for (f, e) in &var_factors {
            match f {
                Factor::Var(nm) if nm == var => {
                    let n = clean(*e);
                    if n.fract() == 0.0 && n >= 1.0 {
                        x_pow_ln = Some(n as i64);
                    }
                }
                Factor::Func(nm, a)
                    if nm == "ln"
                        && (e - &1.0).abs() < 1e-12
                        && a.len() == 1
                        && matches!(&rebuild_poly(&a[0]), Ast::Var(v) if v == var) =>
                {
                    has_ln_x = true;
                }
                _ => {}
            }
        }
        if let Some(n) = x_pow_ln {
            if has_ln_x {
                let np1 = (n + 1) as f64;
                let x = Ast::Var(var.to_string());
                let xn1 = pow(x.clone(), num(np1));
                let term1 = div(mul(xn1.clone(), call1("ln", x)), num(np1));
                let term2 = div(xn1, num(np1 * np1));
                return Ok(mul(k_ast, sub(term1, term2)));
            }
        }
    }

    // Rational function: K·x^m / Q(x) with Q of degree ≥ 2 — partial fractions
    // when Q has distinct rational roots: ∫ = Σ Aᵢ·ln(x − rᵢ).
    {
        let mut q_poly: Option<Poly> = None;
        let mut m: i64 = 0;
        let mut shape_ok = true;
        for (f, e) in &var_factors {
            match f {
                Factor::SumBase(bp) if (*e + 1.0).abs() < 1e-12 && q_poly.is_none() => {
                    q_poly = Some((*bp).clone());
                }
                Factor::Var(name) if name == var => {
                    let n = clean(*e);
                    if n.fract() == 0.0 && n >= 0.0 {
                        m = n as i64;
                    } else {
                        shape_ok = false;
                    }
                }
                _ => shape_ok = false,
            }
        }
        if shape_ok {
            if let Some(q) = q_poly {
                if let Some(res) = integrate_rational(&k_ast, m, &q, var) {
                    return Ok(res);
                }
                if let Some(res) = integrate_quadratic_recip(&k_ast, m, &q, var) {
                    return Ok(res);
                }
                // General real-root case (incl. repeated roots), verified.
                if let Some(res) = integrate_rational_real_roots(&k_ast, m, &q, var) {
                    return Ok(res);
                }
            }
        }
    }

    Err(ExathError::domain(
        "cannot integrate this term symbolically",
    ))
}

/// ∫ K·x^m / Q(x) dx for Q with only real roots (any multiplicity), via numeric
/// partial-fraction decomposition. Returns a result only if d/dx verifies it.
fn integrate_rational_real_roots(k_ast: &Ast, m: i64, q: &Poly, var: &str) -> Option<Ast> {
    let k = eval_const_f64(k_ast).ok()?;
    // Q coefficients (power-indexed, f64).
    let qc_map = poly_coeffs(q, var)?;
    let dq = *qc_map.keys().max()? as usize;
    if dq < 1 {
        return None;
    }
    let mut q_co = vec![0.0; dq + 1];
    for (p, v) in &qc_map {
        q_co[*p as usize] = v.to_f64();
    }
    // Numerator N(x) = k·x^m.
    let mut n_co = vec![0.0; m as usize + 1];
    n_co[m as usize] = k;

    // All roots must be real.
    let roots_raw = roots_of(&q_co);
    if roots_raw.iter().any(|(_, im)| im.abs() > 1e-6) {
        return None;
    }
    // Group into (root, multiplicity).
    let mut grouped: Vec<(f64, usize)> = Vec::new();
    for (re, _) in &roots_raw {
        let r = (re * 1e6).round() / 1e6;
        if let Some(g) = grouped.iter_mut().find(|(x, _)| (*x - r).abs() < 1e-5) {
            g.1 += 1;
        } else {
            grouped.push((r, 1));
        }
    }

    // Polynomial division: N = quo·Q + rem (deg rem < dq).
    let (quo, rem) = poly_divmod(&n_co, &q_co);

    // Unknowns: A_{i} for each (root r_i, power j=1..=mult).
    let mut basis: Vec<(f64, usize)> = Vec::new(); // (root, power)
    for (r, mult) in &grouped {
        for j in 1..=*mult {
            basis.push((*r, j));
        }
    }
    if basis.len() != dq {
        return None;
    }
    // Sample points away from roots.
    let mut pts: Vec<f64> = Vec::new();
    let mut cand = 0.37_f64;
    while pts.len() < dq {
        if grouped.iter().all(|(r, _)| (r - cand).abs() > 1e-3) {
            pts.push(cand);
        }
        cand += 0.911;
    }
    // M·A = rhs, with M[k][i] = Q(x_k)/(x_k - r_i)^{j_i}, rhs[k] = rem(x_k).
    let mut mat = vec![vec![0.0; dq]; dq];
    let mut rhs = vec![0.0; dq];
    for (kk, &x) in pts.iter().enumerate() {
        let qx = poly_eval(&q_co, x);
        for (i, (r, j)) in basis.iter().enumerate() {
            mat[kk][i] = qx / (x - r).powi(*j as i32);
        }
        rhs[kk] = poly_eval(&rem, x);
    }
    let coeffs = gaussian_solve(mat, rhs)?;

    // Build the antiderivative.
    let x = Ast::Var(var.to_string());
    let mut result: Option<Ast> = None;
    let push = |t: Ast, result: &mut Option<Ast>| {
        *result = Some(match result.take() {
            None => t,
            Some(a) => add(a, t),
        });
    };
    // Quotient part: Σ quo[i]·x^(i+1)/(i+1).
    for (i, c) in quo.iter().enumerate() {
        if c.abs() > 1e-12 {
            let p = (i + 1) as f64;
            push(div(mul(num((c * 1e9).round() / 1e9), pow(x.clone(), num(p))), num(p)), &mut result);
        }
    }
    // Partial-fraction parts.
    for ((r, j), a) in basis.iter().zip(coeffs.iter()) {
        let a = (a * 1e9).round() / 1e9;
        if a.abs() < 1e-12 {
            continue;
        }
        let base = sub(x.clone(), num(*r)); // (x - r)
        if *j == 1 {
            push(mul(num(a), call1("ln", base)), &mut result);
        } else {
            let jm1 = (*j - 1) as f64;
            // -A/((j-1)(x-r)^(j-1))
            push(
                neg_ast(div(num(a), mul(num(jm1), pow(base, num(jm1))))),
                &mut result,
            );
        }
    }
    let candidate = simplify_ast(result.unwrap_or_else(|| num(0.0)));

    // Verify d/dx == integrand (k·x^m / Q).
    let integrand = div(mul(num(k), pow(x.clone(), num(m as f64))), rebuild_poly(q));
    if verify_integral(&integrand, &candidate, var) {
        Some(candidate)
    } else {
        None
    }
}

fn poly_eval(c: &[f64], x: f64) -> f64 {
    c.iter().rev().fold(0.0, |acc, &a| acc * x + a)
}

/// Divide numerator by divisor (both power-indexed), returning (quotient, remainder).
fn poly_divmod(num: &[f64], den: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let dd = den.iter().rposition(|c| c.abs() > 1e-12).unwrap_or(0);
    let mut rem = num.to_vec();
    let dn = rem.iter().rposition(|c| c.abs() > 1e-12).unwrap_or(0);
    if dn < dd {
        return (vec![0.0], rem);
    }
    let mut quo = vec![0.0; dn - dd + 1];
    let mut deg = dn;
    while deg >= dd && rem.iter().any(|c| c.abs() > 1e-12) {
        let factor = rem[deg] / den[dd];
        let shift = deg - dd;
        quo[shift] = factor;
        for i in 0..=dd {
            rem[shift + i] -= factor * den[i];
        }
        if deg == 0 {
            break;
        }
        deg -= 1;
    }
    (quo, rem)
}

/// Solve a dense linear system M·x = b by Gaussian elimination with partial
/// pivoting. Returns None if singular.
fn gaussian_solve(mut m: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let piv = (col..n).max_by(|&a, &c| {
            m[a][col].abs().partial_cmp(&m[c][col].abs()).unwrap_or(std::cmp::Ordering::Equal)
        })?;
        if m[piv][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, piv);
        b.swap(col, piv);
        for row in (col + 1)..n {
            let f = m[row][col] / m[col][col];
            for k in col..n {
                m[row][k] -= f * m[col][k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in (i + 1)..n {
            s -= m[i][j] * x[j];
        }
        x[i] = s / m[i][i];
    }
    Some(x)
}

/// ∫ K·x^m / Q(x) dx via partial fractions, when Q (degree ≥ 2) factors into
/// distinct linear factors with rational roots and m < deg(Q). Returns None if
/// those conditions aren't met (caller then reports it can't integrate).
fn integrate_rational(k_ast: &Ast, m: i64, q: &Poly, var: &str) -> Option<Ast> {
    let qmap = poly_coeffs(q, var)?;
    let dq = *qmap.keys().max()? as usize;
    if dq < 2 || (m as usize) >= dq {
        return None;
    }
    let mut qc = vec![Num::zero(); dq + 1];
    for (k, v) in &qmap {
        qc[*k as usize] = *v;
    }
    let k_num = ast_as_num(k_ast)?;

    // Fully factor Q into rational roots via repeated find_rational_root + deflate.
    let mut roots: Vec<Num> = Vec::new();
    let mut cur = qc.clone();
    loop {
        let mut hi = cur.len();
        while hi > 0 && cur[hi - 1].is_zero() {
            hi -= 1;
        }
        if hi <= 1 {
            break;
        }
        let active = &cur[..hi];
        let r = find_rational_root(active)?;
        roots.push(r);
        cur = deflate(active, r);
    }
    if roots.len() != dq {
        return None; // not fully split into rational linear factors
    }
    // Require distinct roots (simple poles).
    for i in 0..roots.len() {
        for j in (i + 1)..roots.len() {
            if roots[i].sub(&roots[j]).is_zero() {
                return None;
            }
        }
    }
    // Residue at each pole: Aᵢ = N(rᵢ)/Q'(rᵢ), N(x) = K·x^m.
    let mut result: Option<Ast> = None;
    for r in &roots {
        let n_ri = k_num.mul(&r.powf(m as f64));
        let qp = eval_poly_deriv(&qc, *r);
        if qp.is_zero() {
            return None;
        }
        let a = n_ri.div(&qp);
        let term = mul(
            num_to_ast(a),
            call1("ln", sub(Ast::Var(var.to_string()), num_to_ast(*r))),
        );
        result = Some(match result {
            None => term,
            Some(x) => add(x, term),
        });
    }
    result
}

/// ∫ K·x^m / (a·x²+b·x+c) dx for an irreducible quadratic (discriminant < 0)
/// and m ∈ {0,1}, via completing the square → ln and atan terms.
fn integrate_quadratic_recip(k_ast: &Ast, m: i64, q: &Poly, var: &str) -> Option<Ast> {
    if m > 1 {
        return None;
    }
    let qmap = poly_coeffs(q, var)?;
    let dq = *qmap.keys().max()? as usize;
    if dq != 2 {
        return None;
    }
    let a = qmap.get(&2).copied().unwrap_or_else(Num::zero);
    let b = qmap.get(&1).copied().unwrap_or_else(Num::zero);
    let c = qmap.get(&0).copied().unwrap_or_else(Num::zero);
    let disc = b.mul(&b).to_f64() - 4.0 * a.mul(&c).to_f64();
    if disc >= 0.0 {
        return None; // reducible (real roots) — handled elsewhere
    }
    let var_ast = Ast::Var(var.to_string());
    // D = sqrt(4ac − b²)
    let d_inner = Num::int(4).mul(&a).mul(&c).sub(&b.mul(&b));
    let d_node = num_sqrt_ast(&d_inner);
    // atan_part = (2/D)·atan((2a·x + b)/D)
    let inner = div(
        add(mul(num_to_ast(Num::int(2).mul(&a)), var_ast.clone()), num_to_ast(b)),
        d_node.clone(),
    );
    let atan_part = div(mul(num(2.0), call1("atan", inner)), d_node);

    if m == 0 {
        return Some(mul(k_ast.clone(), atan_part));
    }
    // m == 1: (1/(2a))·ln(Q) − (b/(2a))·atan_part
    let two_a = Num::int(2).mul(&a);
    let q_ast = rebuild_poly(q);
    let ln_part = mul(num_to_ast(two_a.recip()), call1("ln", q_ast));
    Some(mul(
        k_ast.clone(),
        sub(ln_part, mul(num_to_ast(b.div(&two_a)), atan_part)),
    ))
}

/// Numeric/rational value of a constant AST (number, ratio, or their negation).
fn ast_as_num(a: &Ast) -> Option<Num> {
    match a {
        Ast::Number(n) => Some(Num::from_f64(*n)),
        Ast::UnaryNeg(u) => Some(ast_as_num(u)?.neg()),
        Ast::BinOp(BinOp::Div, x, y) => Some(ast_as_num(x)?.div(&ast_as_num(y)?)),
        Ast::BinOp(BinOp::Mul, x, y) => Some(ast_as_num(x)?.mul(&ast_as_num(y)?)),
        _ => None,
    }
}

/// Q'(x0) for a coefficient vector (index = power), via Num arithmetic.
fn eval_poly_deriv(coeffs: &[Num], x0: Num) -> Num {
    let mut acc = Num::zero();
    let mut pow = Num::one(); // x0^(k-1)
    for k in 1..coeffs.len() {
        let term = Num::int(k as i128).mul(&coeffs[k]).mul(&pow);
        acc = acc.add(&term);
        pow = pow.mul(&x0);
    }
    acc
}

/// ∫ x^n · f(arg) dx via repeated integration by parts, for f ∈ {sin,cos,exp}
/// with `arg` linear (slope `a`). Recurses on `n` (bounded by the exponent).
fn integrate_by_parts(n: i64, name: &str, arg: &Ast, a_ast: &Ast, var: &str) -> Ast {
    // Antiderivative of name(arg) w.r.t. x is sign·newname(arg)/a.
    let (sign, newname) = match name {
        "sin" => (-1.0, "cos"),
        "cos" => (1.0, "sin"),
        _ => (1.0, "exp"), // exp
    };
    let v = div(mul(num(sign), call1(newname, arg.clone())), a_ast.clone());
    if n == 0 {
        return v;
    }
    let x = Ast::Var(var.to_string());
    let term1 = mul(pow(x, num(n as f64)), v);
    // ∫ x^(n-1)·V dx = (sign/a)·∫ x^(n-1)·newname(arg) dx
    let inner = integrate_by_parts(n - 1, newname, arg, a_ast, var);
    let term2 = mul(mul(num(n as f64), div(num(sign), a_ast.clone())), inner);
    sub(term1, term2)
}

/// If `arg` is linear in `var` (i.e. `a·var + b` with constant `a`, `b`),
/// return the slope `a` as an AST; otherwise None.
fn linear_slope(arg: &Ast, var: &str) -> Option<Ast> {
    let a_ast = differentiate_ast(arg, var).ok()?;
    if crate::ast::collect_vars(&a_ast).iter().any(|v| v == var) {
        return None;
    }
    let b_ast = simplify_ast(substitute(arg, var, &num(0.0)));
    let recon = simplify_ast(sub(
        arg.clone(),
        add(mul(a_ast.clone(), Ast::Var(var.to_string())), b_ast),
    ));
    if is_zero_ast(&recon) {
        Some(a_ast)
    } else {
        None
    }
}

/// Solve an equation for `var`. The input is either an expression (taken `= 0`)
/// or an equality `lhs == rhs`. Supports linear and quadratic equations with
/// numeric coefficients; returns each root as an expression string (real or, for
/// negative discriminants, complex using `i`).
pub fn solve(eq: &str, var: &str) -> Result<Vec<String>, ExathError> {
    let ast = parse_str(eq)?;
    let roots = solve_ast(&ast, var)?;
    let mut out: Vec<String> = Vec::new();
    for r in roots {
        let s = render(&simplify_ast(r));
        if !out.contains(&s) {
            out.push(s);
        }
    }
    Ok(out)
}

/// Solve at the AST level (see [`solve`]); returns root ASTs.
pub fn solve_ast(eq: &Ast, var: &str) -> Result<Vec<Ast>, ExathError> {
    // f(x) = lhs - rhs (or just the expression, taken = 0).
    let f = match eq {
        Ast::BinOp(BinOp::Eq, l, r) => sub((**l).clone(), (**r).clone()),
        other => other.clone(),
    };
    // Exact path: polynomial in var → closed form / rational roots / numeric.
    if let Ok(poly) = build(&f) {
        if let Some(coeffs) = poly_coeffs(&poly, var) {
            let degree = coeffs.keys().copied().max().unwrap_or(0) as usize;
            let mut vec = vec![Num::zero(); degree + 1];
            for (k, v) in &coeffs {
                vec[*k as usize] = *v;
            }
            return solve_coeffs(&vec);
        }
    }
    // Exact transcendental path: equation polynomial in u = g(x) for a single
    // invertible g (exp/ln/sqrt of a linear argument). Solve in u, invert, verify.
    if let Some(exact) = solve_by_substitution(&f, var) {
        if !exact.is_empty() {
            return Ok(exact);
        }
    }
    // General path: verified numeric real roots (for transcendental equations).
    let roots = numeric_solve(&f, var);
    if roots.is_empty() {
        return Err(ExathError::domain("solve: no real solution found"));
    }
    Ok(roots.into_iter().map(num).collect())
}

/// Exact solving by substitution u = g(x): if the equation is polynomial in a
/// single invertible sub-expression g (exp/ln/sqrt of a linear argument), solve
/// for u, invert g, and keep only roots that verify numerically.
fn solve_by_substitution(f: &Ast, var: &str) -> Option<Vec<Ast>> {
    let num_eval = |a: &Ast, x: f64| eval_const_f64(&substitute(a, var, &num(x))).ok();
    for g in substitution_candidates(f, var) {
        // g must be exp/ln/sqrt of a linear argument.
        let (gname, arg) = match &g {
            Ast::Call(n, a) if a.len() == 1 && matches!(n.as_str(), "exp" | "ln" | "sqrt") => {
                (n.clone(), a[0].clone())
            }
            _ => continue,
        };
        let a_slope = match linear_slope(&arg, var).and_then(|s| eval_const_f64(&s).ok()) {
            Some(a) if a.abs() > 1e-12 => a,
            _ => continue,
        };
        let b_int = match num_eval(&arg, 0.0) {
            Some(b) => b,
            None => continue,
        };
        let u = fresh_var(f, var);
        let fu = replace_subtree(f, &render(&g), &Ast::Var(u.clone()));
        if crate::ast::collect_vars(&fu).iter().any(|v| v == var) {
            continue; // not purely a function of u
        }
        let poly = match build(&fu) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let cmap = match poly_coeffs(&poly, &u) {
            Some(c) => c,
            None => continue,
        };
        let deg = *cmap.keys().max().unwrap_or(&0) as usize;
        let mut vec = vec![Num::zero(); deg + 1];
        for (k, v) in &cmap {
            vec[*k as usize] = *v;
        }
        let uroots = match solve_coeffs(&vec) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let mut xs: Vec<Ast> = Vec::new();
        for ur in &uroots {
            let uval = match eval_const_f64(ur) {
                Ok(v) => v,
                Err(_) => continue,
            };
            // invert g(x) = ur  ⇒  arg = g⁻¹(ur),  x = (g⁻¹(ur) − b)/a
            let inv_arg: Ast = match gname.as_str() {
                "exp" if uval > 0.0 => call1("ln", ur.clone()),
                "ln" => call1("exp", ur.clone()),
                "sqrt" if uval >= 0.0 => mul(ur.clone(), ur.clone()),
                _ => continue,
            };
            let xexpr = simplify_ast(div(sub(inv_arg, num(b_int)), num(a_slope)));
            // verify in the original equation
            if let Some(xval) = eval_const_f64(&xexpr).ok() {
                if num_eval(f, xval).map(|v| v.abs() < 1e-7).unwrap_or(false)
                    && !xs.iter().any(|e| render(e) == render(&xexpr))
                {
                    xs.push(xexpr);
                }
            }
        }
        if !xs.is_empty() {
            return Some(xs);
        }
    }
    None
}

/// Find real roots of `f(var)=0` numerically: scan a range for sign changes,
/// bisect, polish with Newton, dedupe, and keep only verified roots.
fn numeric_solve(f: &Ast, var: &str) -> Vec<f64> {
    let eval = |x: f64| -> Option<f64> {
        eval_const_f64(&substitute(f, var, &num(x)))
            .ok()
            .filter(|v| v.is_finite())
    };
    let fp = differentiate_ast(f, var).ok();
    let mut roots: Vec<f64> = Vec::new();
    let push_root = |r: f64, roots: &mut Vec<f64>| {
        if let Some(v) = eval(r) {
            if v.abs() < 1e-7 && !roots.iter().any(|x| (x - r).abs() < 1e-6) {
                roots.push((r * 1e9).round() / 1e9);
            }
        }
    };
    let (lo, hi, step) = (-50.0_f64, 50.0_f64, 0.05_f64);
    let mut x = lo;
    let mut prev = eval(x);
    while x < hi {
        let nx = x + step;
        let cur = eval(nx);
        if let (Some(a), Some(b)) = (prev, cur) {
            if a == 0.0 {
                push_root(x, &mut roots);
            }
            if a * b < 0.0 {
                // bisection
                let (mut l, mut r) = (x, nx);
                let mut fl = a;
                for _ in 0..80 {
                    let m = 0.5 * (l + r);
                    let fm = match eval(m) {
                        Some(v) => v,
                        None => break,
                    };
                    if fl * fm <= 0.0 {
                        r = m;
                    } else {
                        l = m;
                        fl = fm;
                    }
                }
                let mut root = 0.5 * (l + r);
                // Newton polish
                if let Some(fp) = &fp {
                    for _ in 0..20 {
                        let fv = match eval(root) {
                            Some(v) => v,
                            None => break,
                        };
                        let dv = eval_const_f64(&substitute(fp, var, &num(root))).unwrap_or(0.0);
                        if dv.abs() < 1e-12 {
                            break;
                        }
                        let nr = root - fv / dv;
                        if (nr - root).abs() < 1e-13 {
                            root = nr;
                            break;
                        }
                        root = nr;
                    }
                }
                push_root(root, &mut roots);
            }
        }
        prev = cur;
        x = nx;
    }
    roots.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    roots
}

/// Solve a univariate polynomial given its coefficients (index = power).
/// Linear & quadratic in closed form; degree ≥ 3 via rational roots + deflation.
fn solve_coeffs(coeffs: &[Num]) -> Result<Vec<Ast>, ExathError> {
    // Trim leading-zero (highest) coefficients to find the true degree.
    let mut hi = coeffs.len();
    while hi > 0 && coeffs[hi - 1].is_zero() {
        hi -= 1;
    }
    if hi == 0 {
        return Err(ExathError::domain("solve: identity — every value is a solution"));
    }
    let degree = hi - 1;
    let get = |i: usize| coeffs.get(i).copied().unwrap_or_else(Num::zero);

    match degree {
        0 => Err(ExathError::domain("solve: no solution")),
        1 => Ok(vec![num_to_ast(get(0).neg().div(&get(1)))]),
        2 => {
            let (a, b, c) = (get(2), get(1), get(0));
            let disc = b.mul(&b).sub(&Num::int(4).mul(&a).mul(&c));
            let two_a = num_to_ast(Num::int(2).mul(&a));
            let neg_b = num_to_ast(b.neg());
            let sqrt_disc = num_sqrt_ast(&disc.abs());
            if disc.to_f64() >= 0.0 {
                Ok(vec![
                    div(add(neg_b.clone(), sqrt_disc.clone()), two_a.clone()),
                    div(sub(neg_b, sqrt_disc), two_a),
                ])
            } else {
                let i = Ast::Var("i".to_string());
                Ok(vec![
                    div(add(neg_b.clone(), mul(sqrt_disc.clone(), i.clone())), two_a.clone()),
                    div(sub(neg_b, mul(sqrt_disc, i)), two_a),
                ])
            }
        }
        _ => {
            let active = &coeffs[..hi];
            // Factor out x if 0 is a root.
            if get(0).is_zero() {
                let mut rest = solve_coeffs(&active[1..])?;
                rest.push(num(0.0));
                return Ok(rest);
            }
            if let Some(root) = find_rational_root(active) {
                let quotient = deflate(active, root);
                let mut roots = vec![num_to_ast(root)];
                roots.extend(solve_coeffs(&quotient)?);
                return Ok(roots);
            }
            // No rational root: fall back to numeric (Durand–Kerner) roots.
            let approx = numeric_roots(active)
                .ok_or_else(|| ExathError::domain("solve: could not find roots numerically"))?;
            Ok(approx.into_iter().map(complex_to_ast).collect())
        }
    }
}

// ── Numeric polynomial roots (Durand–Kerner) ──────────────────────────────────

type C = (f64, f64);

fn c_add(a: C, b: C) -> C {
    (a.0 + b.0, a.1 + b.1)
}
fn c_sub(a: C, b: C) -> C {
    (a.0 - b.0, a.1 - b.1)
}
fn c_mul(a: C, b: C) -> C {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}
fn c_div(a: C, b: C) -> C {
    let d = b.0 * b.0 + b.1 * b.1;
    if d == 0.0 {
        (0.0, 0.0)
    } else {
        ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
    }
}
fn c_abs(a: C) -> f64 {
    a.0.hypot(a.1)
}

/// Numeric roots of a real polynomial given power-indexed f64 coefficients.
/// Returns `(re, im)` pairs (empty if degenerate). Public for reuse (e.g. matrix
/// eigenvalues = roots of the characteristic polynomial).
pub fn roots_of(coeffs: &[f64]) -> Vec<(f64, f64)> {
    let nums: Vec<Num> = coeffs.iter().map(|c| Num::from_f64(*c)).collect();
    numeric_roots(&nums).unwrap_or_default()
}

/// Find all roots of a polynomial (power-indexed coeffs) numerically. Returns
/// `(re, im)` pairs, or None if the leading coefficient is zero.
fn numeric_roots(coeffs: &[Num]) -> Option<Vec<C>> {
    let n = coeffs.len() - 1;
    let lead = coeffs[n].to_f64();
    if lead == 0.0 || !lead.is_finite() {
        return None;
    }
    // Monic real coefficients.
    let a: Vec<f64> = coeffs.iter().map(|c| c.to_f64() / lead).collect();
    let peval = |x: C| -> C {
        let mut acc = (0.0, 0.0);
        for k in (0..=n).rev() {
            acc = c_add(c_mul(acc, x), (a[k], 0.0));
        }
        acc
    };
    // Initial guesses: powers of a fixed complex seed.
    let seed = (0.4, 0.9);
    let mut r: Vec<C> = Vec::with_capacity(n);
    let mut p = (1.0, 0.0);
    for _ in 0..n {
        r.push(p);
        p = c_mul(p, seed);
    }
    for _ in 0..1000 {
        let mut max_delta = 0.0f64;
        for i in 0..n {
            let num = peval(r[i]);
            let mut den = (1.0, 0.0);
            for j in 0..n {
                if j != i {
                    den = c_mul(den, c_sub(r[i], r[j]));
                }
            }
            if c_abs(den) < 1e-300 {
                continue;
            }
            let delta = c_div(num, den);
            r[i] = c_sub(r[i], delta);
            max_delta = max_delta.max(c_abs(delta));
        }
        if max_delta < 1e-13 {
            break;
        }
    }
    Some(r)
}

/// Turn a numeric complex root into a (rounded) AST: real → number, else a+b·i.
fn complex_to_ast(root: C) -> Ast {
    let re = clean(round_sig(root.0));
    let im = clean(round_sig(root.1));
    if im.abs() < 1e-7 {
        return num(re);
    }
    let i = Ast::Var("i".to_string());
    let imag = mul(num(im), i);
    if re.abs() < 1e-12 {
        imag
    } else {
        add(num(re), imag)
    }
}

/// Round to ~10 significant digits to suppress numerical noise.
fn round_sig(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let factor = 1e10;
    (x * factor).round() / factor
}

fn eval_poly(coeffs: &[Num], x: Num) -> Num {
    // Horner's method, highest degree first.
    let mut acc = Num::zero();
    for c in coeffs.iter().rev() {
        acc = acc.mul(&x).add(c);
    }
    acc
}

fn divisors(n: i128) -> Vec<i128> {
    let n = n.abs();
    if n == 0 {
        return vec![1];
    }
    let mut out = Vec::new();
    let mut d = 1i128;
    while d <= n && d <= 100_000 {
        if n % d == 0 {
            out.push(d);
        }
        d += 1;
    }
    out
}

/// Find a rational root via the rational-root theorem (requires rational coeffs).
fn find_rational_root(coeffs: &[Num]) -> Option<Num> {
    let deg = coeffs.len() - 1;
    // Clear denominators to integer coefficients.
    let mut denom: i128 = 1;
    for c in coeffs {
        let (_, q) = c.as_ratio()?; // None if irrational
        denom = lcm_i128(denom, q)?;
    }
    let int_coeff = |c: &Num| -> Option<i128> {
        let (p, q) = c.as_ratio()?;
        p.checked_mul(denom / q)
    };
    let a0 = int_coeff(&coeffs[0])?;
    let ad = int_coeff(&coeffs[deg])?;
    for p in divisors(a0) {
        for q in divisors(ad) {
            for sign in [1i128, -1] {
                let cand = Num::rat(sign * p, q);
                if eval_poly(coeffs, cand).is_zero() {
                    return Some(cand);
                }
            }
        }
    }
    None
}

/// Synthetic division of `coeffs` (index = power) by `(x - r)`; returns the
/// quotient coefficients (degree one lower). Assumes `r` is a root.
fn deflate(coeffs: &[Num], r: Num) -> Vec<Num> {
    let n = coeffs.len() - 1; // degree
    let mut b = vec![Num::zero(); n]; // quotient, index 0..n-1
    if n == 0 {
        return b;
    }
    b[n - 1] = coeffs[n];
    let mut k = n - 1;
    while k > 0 {
        b[k - 1] = coeffs[k].add(&r.mul(&b[k]));
        k -= 1;
    }
    b
}

fn lcm_i128(a: i128, b: i128) -> Option<i128> {
    if a == 0 || b == 0 {
        return Some(0);
    }
    let mut x = a.abs();
    let mut y = b.abs();
    while y != 0 {
        let t = x % y;
        x = y;
        y = t;
    }
    let g = x; // gcd
    (a.abs() / g).checked_mul(b.abs())
}

/// Expand products/powers and apply identities in the *expanding* direction:
/// `ln(a·b)=ln a+ln b`, `ln(a^n)=n·ln a`, `exp(a+b)=exp a·exp b`, and the
/// trigonometric sum / multiple-angle formulas. Complements `simplify` (which
/// collapses); kept explicit so `simplify`'s never-larger guarantee holds.
pub fn expand(expr: &str) -> Result<String, ExathError> {
    Ok(render(&expand_tree(&parse_str(expr)?)))
}

/// Expand at the AST level (see [`expand`]).
pub fn expand_tree(ast: &Ast) -> Ast {
    normalize_keep_exp(expand_ast(ast))
}

/// Taylor polynomial of `expr` about `var = x0` up to (and including) degree
/// `order`. Coefficients stay exact (rational) when the derivative values are
/// integers — e.g. the series of `exp(x)` has `1/2`, `1/6`, …
pub fn taylor(expr: &str, var: &str, x0: f64, order: usize) -> Result<String, ExathError> {
    Ok(render(&taylor_ast(&parse_str(expr)?, var, x0, order)?))
}

pub fn taylor_ast(f: &Ast, var: &str, x0: f64, order: usize) -> Result<Ast, ExathError> {
    if order > 32 {
        return Err(ExathError::domain("taylor: order too high (max 32)"));
    }
    let base = if x0 == 0.0 {
        Ast::Var(var.to_string())
    } else {
        sub(Ast::Var(var.to_string()), num(x0))
    };
    let mut deriv = f.clone();
    let mut factorial = 1.0f64;
    let mut acc: Option<Ast> = None;
    for k in 0..=order {
        if k > 0 {
            deriv = differentiate_ast(&deriv, var)?;
            factorial *= k as f64;
        }
        // value of the k-th derivative at x0 (folded by simplify)
        let at = substitute(&deriv, var, &num(x0));
        let powk = if k == 0 {
            num(1.0)
        } else {
            pow(base.clone(), num(k as f64))
        };
        let term = div(mul(at, powk), num(factorial));
        acc = Some(match acc {
            None => term,
            Some(a) => add(a, term),
        });
    }
    Ok(simplify_ast(acc.unwrap_or_else(|| num(0.0))))
}

/// Limit of `expr` as `var → x0`. Handles continuous points directly and the
/// indeterminate `0/0` form via L'Hôpital's rule. Returns the value as a string.
pub fn limit(expr: &str, var: &str, x0: f64) -> Result<String, ExathError> {
    let v = limit_value(&parse_str(expr)?, var, x0)?;
    Ok(render(&simplify_ast(num(v))))
}

/// Numeric value of `ast` with no free variables (radian mode).
fn eval_const_f64(ast: &Ast) -> Result<f64, ExathError> {
    let vars: HashMap<String, Cx> = HashMap::new();
    let fns = UserFns::new();
    let cx = eval_ast(ast, &vars, &fns, AngleMode::Rad)?;
    Ok(cx.re)
}

fn value_at(f: &Ast, var: &str, x0: f64) -> Option<f64> {
    eval_const_f64(&substitute(f, var, &num(x0)))
        .ok()
        .filter(|v| v.is_finite())
}

pub fn limit_value(f: &Ast, var: &str, x0: f64) -> Result<f64, ExathError> {
    // Limit at ±∞: probe at growing magnitudes and require convergence.
    if x0.is_infinite() {
        let sign = if x0 > 0.0 { 1.0 } else { -1.0 };
        let mut vals: Vec<f64> = Vec::new();
        for mag in [1e2, 1e3, 1e4, 1e5, 1e6, 1e8, 1e10, 1e12] {
            if let Some(val) = value_at(f, var, sign * mag) {
                if let Some(&p) = vals.last() {
                    if (val - p).abs() <= 1e-6 * (1.0 + val.abs()) {
                        return Ok(if val.abs() < 1e-7 { 0.0 } else { round_sig(val) });
                    }
                }
                vals.push(val);
            }
        }
        // Slowly-converging vanishing limit: |f| monotonically shrinks toward 0.
        if let Some(&last) = vals.last() {
            if last.abs() < 1e-7
                && vals.len() >= 3
                && vals.windows(2).all(|w| w[1].abs() <= w[0].abs() + 1e-12)
            {
                return Ok(0.0);
            }
        }
        return Err(ExathError::domain("limit: does not converge at infinity"));
    }
    // Continuous case: direct substitution (also try a simplified form).
    if let Some(v) = value_at(f, var, x0) {
        return Ok(v);
    }
    if let Some(v) = value_at(&simplify_ast(f.clone()), var, x0) {
        return Ok(v);
    }
    // Indeterminate 0/0 or ∞/∞ via L'Hôpital, applied repeatedly.
    if let Ast::BinOp(BinOp::Div, p, q) = f {
        let (mut p, mut q) = ((**p).clone(), (**q).clone());
        for _ in 0..16 {
            let pv = eval_const_f64(&substitute(&p, var, &num(x0)));
            let qv = eval_const_f64(&substitute(&q, var, &num(x0)));
            let near_zero = |r: &Result<f64, ExathError>| matches!(r, Ok(v) if v.abs() < 1e-9);
            let diverges = |r: &Result<f64, ExathError>| {
                matches!(r, Ok(v) if !v.is_finite() || v.abs() > 1e8) || r.is_err()
            };
            let indeterminate = (near_zero(&pv) && near_zero(&qv))
                || (diverges(&pv) && diverges(&qv));
            if indeterminate {
                p = differentiate_ast(&p, var)?;
                q = differentiate_ast(&q, var)?;
                // Simplify the ratio so cancellations (e.g. t²/t²) resolve.
                let cand = simplify_ast(div(p.clone(), q.clone()));
                if let Some(v) = value_at(&cand, var, x0) {
                    return Ok(v);
                }
            } else {
                break;
            }
        }
    }
    Err(ExathError::domain("limit: could not determine the limit"))
}

/// Normal form that collects like terms but does NOT recombine `exp` factors
/// (so expansion isn't immediately undone).
fn normalize_keep_exp(ast: Ast) -> Ast {
    match build(&ast) {
        Ok(p) => rebuild_poly(&simplify_trig(p)),
        Err(_) => ast,
    }
}

fn expand_ast(a: &Ast) -> Ast {
    match a {
        Ast::Matrix(rows) => Ast::Matrix(
            rows.iter().map(|r| r.iter().map(expand_ast).collect()).collect(),
        ),
        Ast::Number(_) | Ast::Var(_) => a.clone(),
        Ast::UnaryNeg(u) => Ast::UnaryNeg(boxed(expand_ast(u))),
        Ast::UnaryNot(u) => Ast::UnaryNot(boxed(expand_ast(u))),
        Ast::Factorial(u) => Ast::Factorial(boxed(expand_ast(u))),
        Ast::BinOp(op, l, r) => {
            Ast::BinOp(op.clone(), boxed(expand_ast(l)), boxed(expand_ast(r)))
        }
        Ast::Call(name, args) => {
            let ea: Vec<Ast> = args.iter().map(expand_ast).collect();
            if ea.len() == 1 {
                let u = &ea[0];
                match name.as_str() {
                    "ln" | "lg" | "log" => return expand_log(name, u),
                    "exp" => return expand_exp(u),
                    "sin" => {
                        if let Some((p, q)) = split_sum(u) {
                            return expand_ast(&add(
                                mul(call1("sin", p.clone()), call1("cos", q.clone())),
                                mul(call1("cos", p), call1("sin", q)),
                            ));
                        }
                    }
                    "cos" => {
                        if let Some((p, q)) = split_sum(u) {
                            return expand_ast(&sub(
                                mul(call1("cos", p.clone()), call1("cos", q.clone())),
                                mul(call1("sin", p), call1("sin", q)),
                            ));
                        }
                    }
                    _ => {}
                }
            }
            Ast::Call(name.clone(), ea)
        }
    }
}

fn expand_log(name: &str, u: &Ast) -> Ast {
    match u {
        Ast::BinOp(BinOp::Mul, a, b) => add(expand_log(name, a), expand_log(name, b)),
        Ast::BinOp(BinOp::Div, a, b) => sub(expand_log(name, a), expand_log(name, b)),
        Ast::BinOp(BinOp::Pow, a, n) => mul(expand_ast(n), expand_log(name, a)),
        other => call1(name, expand_ast(other)),
    }
}

fn expand_exp(u: &Ast) -> Ast {
    match u {
        Ast::BinOp(BinOp::Add, a, b) => mul(expand_exp(a), expand_exp(b)),
        Ast::BinOp(BinOp::Sub, a, b) => div(expand_exp(a), expand_exp(b)),
        other => call1("exp", expand_ast(other)),
    }
}

/// Split an angle into two parts for the sum formula: `a+b`, `a-b`, or `n·x`
/// (as `x + (n-1)·x`). Returns None if it is not a sum/integer multiple.
fn split_sum(u: &Ast) -> Option<(Ast, Ast)> {
    match u {
        Ast::BinOp(BinOp::Add, a, b) => Some(((**a).clone(), (**b).clone())),
        Ast::BinOp(BinOp::Sub, a, b) => {
            Some(((**a).clone(), Ast::UnaryNeg(boxed((**b).clone()))))
        }
        Ast::BinOp(BinOp::Mul, a, b) => {
            let split_mul = |coeff: &Ast, rest: &Ast| -> Option<(Ast, Ast)> {
                if let Ast::Number(n) = coeff {
                    let n = clean(*n);
                    if n.fract() == 0.0 && n >= 2.0 {
                        return Some((
                            rest.clone(),
                            mul(num(n - 1.0), rest.clone()),
                        ));
                    }
                }
                None
            };
            split_mul(a, b).or_else(|| split_mul(b, a))
        }
        _ => None,
    }
}

/// Factor a univariate polynomial over the rationals: pull out rational linear
/// factors `(x − r)` (and `x` for a zero root); any non-rational remainder is
/// left as a polynomial factor. E.g. `x^2 - 5x + 6 → (x - 2)·(x - 3)`.
pub fn factor(expr: &str, var: &str) -> Result<String, ExathError> {
    Ok(render(&factor_tree(&parse_str(expr)?, var)?))
}

/// Factor at the AST level (see [`factor`]).
pub fn factor_tree(ast: &Ast, var: &str) -> Result<Ast, ExathError> {
    let poly = build(ast)?;
    let coeffs = poly_coeffs(&poly, var)
        .ok_or_else(|| ExathError::domain("factor: not a univariate polynomial"))?;
    let degree = coeffs.keys().copied().max().unwrap_or(0) as usize;
    let mut cur = vec![Num::zero(); degree + 1];
    for (k, v) in &coeffs {
        cur[*k as usize] = *v;
    }

    let var_ast = Ast::Var(var.to_string());
    let mut linear: Vec<Ast> = Vec::new();
    let mut roots: Vec<Num> = Vec::new();
    loop {
        // trim leading zeros
        let mut hi = cur.len();
        while hi > 0 && cur[hi - 1].is_zero() {
            hi -= 1;
        }
        if hi <= 1 {
            break;
        }
        // zero root: factor out x
        if cur[0].is_zero() {
            linear.push(var_ast.clone());
            cur = cur[1..hi].to_vec();
            continue;
        }
        match find_rational_root(&cur[..hi]) {
            Some(r) => {
                roots.push(r);
                cur = deflate(&cur[..hi], r);
            }
            None => break,
        }
    }
    roots.sort_by(|a, b| a.to_f64().partial_cmp(&b.to_f64()).unwrap_or(std::cmp::Ordering::Equal));
    for r in roots {
        if r.is_negative() {
            linear.push(add(var_ast.clone(), num_to_ast(r.neg())));
        } else {
            linear.push(sub(var_ast.clone(), num_to_ast(r)));
        }
    }

    // Remaining (irreducible-over-ℚ) factor, including any leading constant.
    let remainder = coeffs_to_poly(&cur, var);
    let mut factors: Vec<Ast> = Vec::new();
    let rem_ast = rebuild_poly(&remainder);
    if !matches!(rem_ast, Ast::Number(n) if (n - 1.0).abs() < 1e-12) {
        factors.push(rem_ast);
    }
    factors.extend(linear);
    if factors.is_empty() {
        factors.push(num(1.0));
    }
    Ok(mul_fold(factors))
}

/// Greatest common divisor of two univariate polynomials over ℚ (monic).
pub fn poly_gcd(p: &str, q: &str, var: &str) -> Result<String, ExathError> {
    Ok(render(&poly_gcd_ast(&parse_str(p)?, &parse_str(q)?, var)?))
}

/// Polynomial GCD at the AST level (see [`poly_gcd`]).
pub fn poly_gcd_ast(p: &Ast, q: &Ast, var: &str) -> Result<Ast, ExathError> {
    let pa = poly_coeffs(&build(p)?, var)
        .ok_or_else(|| ExathError::domain("polygcd: arguments must be polynomials"))?;
    let qa = poly_coeffs(&build(q)?, var)
        .ok_or_else(|| ExathError::domain("polygcd: arguments must be polynomials"))?;
    let to_vec = |m: &BTreeMap<i64, Num>| -> Vec<Num> {
        let d = m.keys().copied().max().unwrap_or(0) as usize;
        let mut v = vec![Num::zero(); d + 1];
        for (k, val) in m {
            v[*k as usize] = *val;
        }
        v
    };
    let g = gcd_coeffs(to_vec(&pa), to_vec(&qa))
        .ok_or_else(|| ExathError::domain("polygcd: could not compute gcd"))?;
    Ok(rebuild_poly(&coeffs_to_poly(&g, var)))
}

fn poly_deg(v: &[Num]) -> Option<usize> {
    (0..v.len()).rev().find(|&i| !v[i].is_zero())
}

/// Polynomial remainder of `a` divided by `b` (power-indexed coefficients).
fn poly_rem(a: &[Num], b: &[Num]) -> Option<Vec<Num>> {
    let db = poly_deg(b)?;
    let lead = b[db];
    let mut r = a.to_vec();
    while let Some(dr) = poly_deg(&r) {
        if dr < db {
            break;
        }
        let coef = r[dr].div(&lead);
        let shift = dr - db;
        for j in 0..=db {
            r[shift + j] = r[shift + j].sub(&coef.mul(&b[j]));
        }
        r[dr] = Num::zero(); // guard against fp residue at the top
    }
    Some(r)
}

fn gcd_coeffs(mut a: Vec<Num>, mut b: Vec<Num>) -> Option<Vec<Num>> {
    while poly_deg(&b).is_some() {
        let r = poly_rem(&a, &b)?;
        a = b;
        b = r;
    }
    let da = poly_deg(&a)?;
    // normalise to monic
    let lead = a[da];
    let g: Vec<Num> = a[..=da].iter().map(|c| c.div(&lead)).collect();
    Some(g)
}

/// Newton's method: a numeric root of `f` w.r.t. `var` starting from `x0`.
pub fn newton(f: &Ast, var: &str, x0: f64) -> Result<f64, ExathError> {
    let fp = differentiate_ast(f, var)?;
    let mut x = x0;
    for _ in 0..100 {
        let fx = eval_const_f64(&substitute(f, var, &num(x)))?;
        if fx.abs() < 1e-12 {
            return Ok(round_sig(x));
        }
        let dfx = eval_const_f64(&substitute(&fp, var, &num(x)))?;
        if dfx.abs() < 1e-14 {
            return Err(ExathError::domain("nsolve: derivative vanished"));
        }
        let next = x - fx / dfx;
        if (next - x).abs() < 1e-13 {
            return Ok(round_sig(next));
        }
        x = next;
    }
    Err(ExathError::domain("nsolve: did not converge"))
}

/// Symbolic closed form of Σ_{k=1}^{n} expr for a polynomial `expr` in `k`,
/// returned as a polynomial in `n` (via Faulhaber's formula; exact rationals).
pub fn sum_closed(expr: &str, k: &str, n: &str) -> Result<String, ExathError> {
    Ok(render(&sum_closed_ast(&parse_str(expr)?, k, n)?))
}

pub fn sum_closed_ast(expr: &Ast, k: &str, n: &str) -> Result<Ast, ExathError> {
    let coeffs = poly_coeffs(&build(expr)?, k)
        .ok_or_else(|| ExathError::domain("sumc: summand must be a polynomial in the index"))?;
    let mut result: Vec<Num> = vec![Num::zero()];
    for (p, c) in &coeffs {
        let fp = faulhaber(*p as usize);
        if fp.len() > result.len() {
            result.resize(fp.len(), Num::zero());
        }
        for (deg, coef) in fp.iter().enumerate() {
            result[deg] = result[deg].add(&c.mul(coef));
        }
    }
    Ok(rebuild_poly(&coeffs_to_poly(&result, n)))
}

fn binom_i128(n: i128, k: i128) -> i128 {
    if k < 0 || k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut r = 1i128;
    for i in 0..k {
        r = r * (n - i) / (i + 1);
    }
    r
}

/// Bernoulli numbers B₀..B_pmax (convention B₁ = +1/2).
fn bernoulli(pmax: usize) -> Vec<Num> {
    let mut b = vec![Num::zero(); pmax + 1];
    b[0] = Num::one();
    for m in 1..=pmax {
        let mut s = Num::zero();
        for j in 0..m {
            s = s.add(&Num::int(binom_i128(m as i128 + 1, j as i128)).mul(&b[j]));
        }
        b[m] = s.div(&Num::int(-(m as i128 + 1)));
    }
    if pmax >= 1 {
        b[1] = Num::rat(1, 2);
    }
    b
}

/// Σ_{k=1}^{n} k^p as a polynomial in n (power-indexed coefficients).
fn faulhaber(p: usize) -> Vec<Num> {
    let b = bernoulli(p);
    let mut poly = vec![Num::zero(); p + 2];
    let inv = Num::rat(1, p as i128 + 1);
    for j in 0..=p {
        let coef = inv.mul(&Num::int(binom_i128(p as i128 + 1, j as i128))).mul(&b[j]);
        poly[p + 1 - j] = poly[p + 1 - j].add(&coef);
    }
    poly
}

fn coeffs_to_poly(c: &[Num], var: &str) -> Poly {
    let mut p = empty_poly();
    for (k, coef) in c.iter().enumerate() {
        if coef.is_zero() {
            continue;
        }
        let mut factors = BTreeMap::new();
        if k > 0 {
            let f = Factor::Var(var.to_string());
            factors.insert(fkey(&f), (f, k as f64));
        }
        let t = Term { coeff: *coef, factors };
        add_term(&mut p, t);
    }
    p
}

fn num_to_ast(n: Num) -> Ast {
    match n.as_ratio() {
        Some((p, q)) if q == 1 => num(p as f64),
        Some((p, q)) => div(num(p as f64), num(q as f64)),
        None => num(n.to_f64()),
    }
}

/// `sqrt(n)` as an AST — exact when `n` is a perfect-square rational.
fn num_sqrt_ast(n: &Num) -> Ast {
    if let Some((p, q)) = n.as_ratio() {
        if p >= 0 && q > 0 {
            let sp = (p as f64).sqrt().round() as i128;
            let sq = (q as f64).sqrt().round() as i128;
            if sp * sp == p && sq * sq == q {
                return num_to_ast(Num::rat(sp, sq));
            }
        }
    }
    call1("sqrt", num(n.to_f64()))
}

/// Univariate polynomial coefficients (power → coefficient) of `p` in `var`,
/// or None if `p` is not a polynomial in `var` with numeric coefficients.
fn poly_coeffs(p: &Poly, var: &str) -> Option<BTreeMap<i64, Num>> {
    let mut coeffs: BTreeMap<i64, Num> = BTreeMap::new();
    for t in p.terms.values() {
        let mut power: i64 = 0;
        for (_, (f, e)) in &t.factors {
            match f {
                Factor::Var(name) if name == var => {
                    let ce = clean(*e);
                    if ce.fract() != 0.0 || ce < 0.0 {
                        return None;
                    }
                    power = ce as i64;
                }
                // Any other factor means a non-numeric / symbolic coefficient.
                _ => return None,
            }
        }
        let entry = coeffs.entry(power).or_insert_with(Num::zero);
        *entry = entry.add(&t.coeff);
    }
    coeffs.retain(|_, v| !v.is_zero());
    Some(coeffs)
}

/// Laplace transform L{f(t)}(s) via a standard table, for linear combinations
/// of c, t^n, e^(a·t), sin(b·t), cos(b·t). Returns F(s) as an expression string.
pub fn laplace(expr: &str, t: &str, s: &str) -> Result<String, ExathError> {
    Ok(render(&simplify_ast(laplace_ast(&parse_str(expr)?, t, s)?)))
}

pub fn laplace_ast(f: &Ast, t: &str, s: &str) -> Result<Ast, ExathError> {
    let sv = Ast::Var(s.to_string());
    // Constant (no t): L{c} = c/s.
    if !contains_var(f, t) {
        return Ok(div(f.clone(), sv));
    }
    match f {
        Ast::BinOp(BinOp::Add, a, b) => Ok(add(laplace_ast(a, t, s)?, laplace_ast(b, t, s)?)),
        Ast::BinOp(BinOp::Sub, a, b) => Ok(sub(laplace_ast(a, t, s)?, laplace_ast(b, t, s)?)),
        Ast::UnaryNeg(u) => Ok(Ast::UnaryNeg(boxed(laplace_ast(u, t, s)?))),
        Ast::BinOp(BinOp::Mul, a, b) => {
            if !contains_var(a, t) {
                Ok(mul((**a).clone(), laplace_ast(b, t, s)?))
            } else if !contains_var(b, t) {
                Ok(mul((**b).clone(), laplace_ast(a, t, s)?))
            } else {
                Err(ExathError::domain("laplace: unsupported product of t-terms"))
            }
        }
        // L{t} = 1/s²
        Ast::Var(name) if name == t => Ok(div(num(1.0), pow(sv, num(2.0)))),
        // L{t^n} = n!/s^(n+1)
        Ast::BinOp(BinOp::Pow, base, exp) => {
            if matches!(base.as_ref(), Ast::Var(v) if v == t) {
                if let Ast::Number(nf) = exp.as_ref() {
                    let n = clean(*nf);
                    if n.fract() == 0.0 && n >= 0.0 {
                        let mut fact = 1.0;
                        for i in 1..=(n as i64) {
                            fact *= i as f64;
                        }
                        return Ok(div(num(fact), pow(sv, num(n + 1.0))));
                    }
                }
            }
            Err(ExathError::domain("laplace: unsupported power"))
        }
        Ast::Call(name, args) if args.len() == 1 => {
            let arg = &args[0];
            let coeff = laplace_linear_coeff(arg, t)?; // arg must be (coeff)·t
            match name.as_str() {
                // L{e^{a t}} = 1/(s - a)
                "exp" => Ok(div(num(1.0), sub(sv, coeff))),
                // L{sin(b t)} = b/(s² + b²)
                "sin" => Ok(div(coeff.clone(), add(pow(sv, num(2.0)), pow(coeff, num(2.0))))),
                // L{cos(b t)} = s/(s² + b²)
                "cos" => Ok(div(sv.clone(), add(pow(sv, num(2.0)), pow(coeff, num(2.0))))),
                _ => Err(ExathError::domain(format!(
                    "laplace: no table entry for '{}'",
                    name
                ))),
            }
        }
        _ => Err(ExathError::domain("laplace: expression not in the transform table")),
    }
}

/// Require `arg == coeff·t` (no constant term) and return `coeff` as an AST.
fn laplace_linear_coeff(arg: &Ast, t: &str) -> Result<Ast, ExathError> {
    let a = differentiate_ast(arg, t)?;
    if crate::ast::collect_vars(&a).iter().any(|v| v == t) {
        return Err(ExathError::domain("laplace: argument must be linear in t"));
    }
    let b0 = simplify_ast(substitute(arg, t, &num(0.0)));
    if !is_zero_ast(&b0) {
        return Err(ExathError::domain("laplace: argument must have no constant term"));
    }
    Ok(a)
}

/// Solve a linear homogeneous ODE with constant coefficients. `coeffs` are the
/// coefficients of `a_n·y⁽ⁿ⁾ + … + a_1·y' + a_0·y = 0`, highest order first.
/// Returns the general solution in `var` with constants C1, C2, … (real,
/// repeated and complex-conjugate roots all handled).
pub fn dsolve(coeffs: &[f64], var: &str) -> Result<String, ExathError> {
    if coeffs.len() < 2 {
        return Err(ExathError::domain("dsolve: need at least a first-order ODE"));
    }
    // Characteristic polynomial: power-indexed = reversed coefficient list.
    let charpoly: Vec<f64> = coeffs.iter().rev().cloned().collect();
    let raw = roots_of(&charpoly);
    let r6 = |x: f64| (x * 1e6).round() / 1e6;
    let mut roots: Vec<(f64, f64)> = raw.iter().map(|(a, b)| (r6(*a), r6(*b))).collect();
    roots.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });

    // coefficient·var, prettified: 1→"x", -1→"-x", else "k*x".
    let cv = |k: f64| -> String {
        if (k - 1.0).abs() < 1e-9 {
            var.to_string()
        } else if (k + 1.0).abs() < 1e-9 {
            format!("-{}", var)
        } else {
            format!("{}*{}", fmt_plain(k), var)
        }
    };

    let mut used = vec![false; roots.len()];
    let mut terms: Vec<String> = Vec::new();
    let mut c = 1usize;
    for i in 0..roots.len() {
        if used[i] {
            continue;
        }
        let (re, im) = roots[i];
        if im.abs() < 1e-6 {
            // real root r with multiplicity m → Σ C·x^k·e^{r x}
            let mult = (0..roots.len())
                .filter(|&j| !used[j] && roots[j].1.abs() < 1e-6 && (roots[j].0 - re).abs() < 1e-6)
                .collect::<Vec<_>>();
            for &j in &mult {
                used[j] = true;
            }
            for k in 0..mult.len() {
                let mut parts = vec![format!("C{}", c)];
                c += 1;
                if k == 1 {
                    parts.push(var.to_string());
                } else if k >= 2 {
                    parts.push(format!("{}^{}", var, k));
                }
                if re.abs() > 1e-12 {
                    parts.push(format!("exp({})", cv(re)));
                }
                terms.push(parts.join(" * "));
            }
        } else if im > 0.0 {
            // complex pair a±bi with multiplicity m → x^k·e^{a x}·(C·cos+C·sin)
            let members = (0..roots.len())
                .filter(|&j| {
                    !used[j] && (roots[j].0 - re).abs() < 1e-6 && (roots[j].1.abs() - im).abs() < 1e-6
                })
                .collect::<Vec<_>>();
            for &j in &members {
                used[j] = true;
            }
            let pairs = (members.len() / 2).max(1);
            for k in 0..pairs {
                let trig = format!(
                    "C{}*cos({}) + C{}*sin({})",
                    c,
                    cv(im),
                    c + 1,
                    cv(im)
                );
                c += 2;
                let mut s = format!("({})", trig);
                if k == 1 {
                    s = format!("{} * {}", var, s);
                } else if k >= 2 {
                    s = format!("{}^{} * {}", var, k, s);
                }
                if re.abs() > 1e-12 {
                    s = format!("exp({}) * {}", cv(re), s);
                }
                terms.push(s);
            }
        } else {
            used[i] = true; // negative-imaginary conjugate, handled with its pair
        }
    }
    Ok(terms.join(" + "))
}

fn fmt_plain(x: f64) -> String {
    if x.is_finite() && (x - x.round()).abs() < 1e-9 && x.abs() < 1e15 {
        format!("{}", x.round() as i64)
    } else {
        format!("{}", x)
    }
}

/// Render an AST back to an expression string with minimal parentheses.
pub fn render(ast: &Ast) -> String {
    unparse(ast)
}

/// Replace every `Ast::Var(name)` with `replacement`.
pub fn substitute(ast: &Ast, name: &str, replacement: &Ast) -> Ast {
    match ast {
        Ast::Matrix(rows) => Ast::Matrix(
            rows.iter()
                .map(|r| r.iter().map(|e| substitute(e, name, replacement)).collect())
                .collect(),
        ),
        Ast::Var(n) if n == name => replacement.clone(),
        Ast::Number(_) | Ast::Var(_) => ast.clone(),
        Ast::BinOp(op, l, r) => Ast::BinOp(
            op.clone(),
            boxed(substitute(l, name, replacement)),
            boxed(substitute(r, name, replacement)),
        ),
        Ast::UnaryNeg(u) => Ast::UnaryNeg(boxed(substitute(u, name, replacement))),
        Ast::UnaryNot(u) => Ast::UnaryNot(boxed(substitute(u, name, replacement))),
        Ast::Factorial(u) => Ast::Factorial(boxed(substitute(u, name, replacement))),
        Ast::Call(fname, args) => Ast::Call(
            fname.clone(),
            args.iter().map(|a| substitute(a, name, replacement)).collect(),
        ),
    }
}

/// Inline calls to user-defined functions, substituting their bodies. Built-in
/// functions are left intact (their arguments are still inlined). Returns an
/// error on arity mismatch or excessive recursion depth.
pub fn inline_user_fns(ast: &Ast, fns: &UserFns) -> Result<Ast, ExathError> {
    inline_rec(ast, fns, 0)
}

fn inline_rec(ast: &Ast, fns: &UserFns, depth: usize) -> Result<Ast, ExathError> {
    if depth > INLINE_DEPTH_LIMIT {
        return Err(ExathError::domain(
            "function inlining too deep (recursive definition?)",
        ));
    }
    match ast {
        Ast::Number(_) | Ast::Var(_) => Ok(ast.clone()),
        Ast::Matrix(rows) => {
            let mut out = Vec::with_capacity(rows.len());
            for r in rows {
                let mut nr = Vec::with_capacity(r.len());
                for e in r {
                    nr.push(inline_rec(e, fns, depth + 1)?);
                }
                out.push(nr);
            }
            Ok(Ast::Matrix(out))
        }
        Ast::BinOp(op, l, r) => Ok(Ast::BinOp(
            op.clone(),
            boxed(inline_rec(l, fns, depth + 1)?),
            boxed(inline_rec(r, fns, depth + 1)?),
        )),
        Ast::UnaryNeg(u) => Ok(Ast::UnaryNeg(boxed(inline_rec(u, fns, depth + 1)?))),
        Ast::UnaryNot(u) => Ok(Ast::UnaryNot(boxed(inline_rec(u, fns, depth + 1)?))),
        Ast::Factorial(u) => Ok(Ast::Factorial(boxed(inline_rec(u, fns, depth + 1)?))),
        Ast::Call(name, args) => {
            let mut inlined_args = Vec::with_capacity(args.len());
            for a in args {
                inlined_args.push(inline_rec(a, fns, depth + 1)?);
            }
            match fns.get(name) {
                Some((params, body)) => {
                    if params.len() != inlined_args.len() {
                        return Err(ExathError::arg_count(format!(
                            "{}() expects {} argument(s), got {}",
                            name,
                            params.len(),
                            inlined_args.len()
                        )));
                    }
                    let mut b = body.clone();
                    for (param, arg) in params.iter().zip(inlined_args.iter()) {
                        b = substitute(&b, param, arg);
                    }
                    inline_rec(&b, fns, depth + 1)
                }
                None => Ok(Ast::Call(name.clone(), inlined_args)),
            }
        }
    }
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
        Ast::Matrix(rows) => rows.iter().any(|r| r.iter().any(|e| contains_var(e, var))),
        Ast::Number(_) => false,
        Ast::Var(name) => name == var,
        Ast::BinOp(_, l, r) => contains_var(l, var) || contains_var(r, var),
        Ast::UnaryNeg(u) | Ast::UnaryNot(u) | Ast::Factorial(u) => contains_var(u, var),
        Ast::Call(_, args) => args.iter().any(|a| contains_var(a, var)),
    }
}

fn diff(ast: &Ast, var: &str) -> Result<Ast, ExathError> {
    match ast {
        Ast::Matrix(_) => Err(ExathError::domain(
            "cannot differentiate a matrix expression",
        )),
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
    // Piecewise: differentiate branch-wise (valid away from the breakpoints).
    if name == "if" && args.len() == 3 {
        return Ok(Ast::Call(
            "if".to_string(),
            vec![args[0].clone(), diff(&args[1], var)?, diff(&args[2], var)?],
        ));
    }
    if name == "piecewise" && args.len() >= 3 {
        // piecewise(c1, v1, c2, v2, ..., default): differentiate every value.
        let mut out = Vec::with_capacity(args.len());
        let mut i = 0;
        while i + 1 < args.len() {
            out.push(args[i].clone()); // condition unchanged
            out.push(diff(&args[i + 1], var)?); // value differentiated
            i += 2;
        }
        if i < args.len() {
            out.push(diff(&args[i], var)?); // default differentiated
        }
        return Ok(Ast::Call("piecewise".to_string(), out));
    }
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

// ── Simplification: canonical polynomial normal form ──────────────────────────
//
// An expression is normalised into a sum of terms. Each term is a numeric
// coefficient times a product of factors raised to (possibly negative or
// fractional) exponents. Like terms are collected, like factors merged into
// powers, products expanded, constants folded, and division handled via
// reciprocals. The result is rebuilt into a clean, deterministically-ordered
// AST. Non-polynomial pieces (function calls, symbolic powers like 2^x,
// factorials, comparisons) are carried as opaque atomic factors so nothing is
// ever lost or evaluated incorrectly.

/// Largest integer exponent that is expanded by repeated multiplication; beyond
/// this a power of a sum is kept as an atomic base to avoid blow-up.
const MAX_POW_EXPAND: i64 = 16;

/// Round values that are within 1e-9 of an integer (cosmetic + key stability).
fn clean(x: f64) -> f64 {
    let r = x.round();
    if (x - r).abs() < 1e-9 {
        r
    } else {
        x
    }
}

fn key_num(x: f64) -> String {
    format!("{}", clean(x))
}

/// An irreducible multiplicative factor.
#[derive(Clone)]
enum Factor {
    Var(String),
    Func(String, Vec<Poly>),
    /// A non-monomial base (a sum) raised to some exponent stored in the term.
    SumBase(Poly),
    /// `base ^ exp` where the exponent is not a numeric constant (e.g. 2^x).
    OpaquePow(Poly, Poly),
    /// Anything we do not decompose (factorial, comparison, …). The string is a
    /// canonical key; the Ast is used to rebuild it.
    Atom(String, Ast),
}

#[derive(Clone)]
struct Term {
    coeff: Num,
    /// factor canonical key → (factor, exponent)
    factors: BTreeMap<String, (Factor, f64)>,
}

#[derive(Clone)]
struct Poly {
    /// monomial key → term
    terms: BTreeMap<String, Term>,
}

fn fkey(f: &Factor) -> String {
    match f {
        Factor::Var(s) => format!("v:{}", s),
        Factor::Func(name, args) => {
            let inner: Vec<String> = args.iter().map(pkey).collect();
            format!("f:{}({})", name, inner.join(","))
        }
        Factor::SumBase(p) => format!("s:({})", pkey(p)),
        Factor::OpaquePow(b, e) => format!("o:({})^({})", pkey(b), pkey(e)),
        Factor::Atom(k, _) => format!("a:{}", k),
    }
}

fn mkey(t: &Term) -> String {
    if t.factors.is_empty() {
        return "1".to_string();
    }
    t.factors
        .iter()
        .map(|(k, (_, e))| format!("{}^{}", k, key_num(*e)))
        .collect::<Vec<_>>()
        .join("*")
}

fn pkey(p: &Poly) -> String {
    p.terms
        .iter()
        .map(|(k, t)| format!("{}:{}", t.coeff.key(), k))
        .collect::<Vec<_>>()
        .join("+")
}

fn empty_poly() -> Poly {
    Poly { terms: BTreeMap::new() }
}

fn poly_const(c: f64) -> Poly {
    poly_const_num(Num::from_f64(c))
}

fn poly_const_num(c: Num) -> Poly {
    let mut p = empty_poly();
    if !c.is_zero() {
        p.terms.insert("1".to_string(), Term { coeff: c, factors: BTreeMap::new() });
    }
    p
}

fn poly_factor(f: Factor) -> Poly {
    let mut factors = BTreeMap::new();
    factors.insert(fkey(&f), (f, 1.0));
    let t = Term { coeff: Num::one(), factors };
    let mut p = empty_poly();
    p.terms.insert(mkey(&t), t);
    p
}

fn poly_atom(ast: Ast) -> Poly {
    poly_factor(Factor::Atom(unparse(&ast), ast))
}

fn add_term(p: &mut Poly, t: Term) {
    if t.coeff.is_zero() {
        return;
    }
    let key = mkey(&t);
    match p.terms.get_mut(&key) {
        Some(existing) => {
            existing.coeff = existing.coeff.add(&t.coeff);
            if existing.coeff.is_zero() {
                p.terms.remove(&key);
            }
        }
        None => {
            p.terms.insert(key, t);
        }
    }
}

fn poly_add(a: &Poly, b: &Poly) -> Poly {
    let mut r = a.clone();
    for t in b.terms.values() {
        add_term(&mut r, t.clone());
    }
    r
}

fn poly_scale(p: &Poly, s: f64) -> Poly {
    let mut r = empty_poly();
    let s = Num::from_f64(s);
    if s.is_zero() {
        return r;
    }
    for t in p.terms.values() {
        add_term(&mut r, Term { coeff: t.coeff.mul(&s), factors: t.factors.clone() });
    }
    r
}

fn poly_neg(p: &Poly) -> Poly {
    poly_scale(p, -1.0)
}

fn term_mul(a: &Term, b: &Term) -> Term {
    let mut factors = a.factors.clone();
    for (k, (f, e)) in &b.factors {
        match factors.get_mut(k) {
            Some(entry) => {
                entry.1 += *e;
                if clean(entry.1) == 0.0 {
                    factors.remove(k);
                }
            }
            None => {
                factors.insert(k.clone(), (f.clone(), *e));
            }
        }
    }
    Term { coeff: a.coeff.mul(&b.coeff), factors }
}

fn poly_mul(a: &Poly, b: &Poly) -> Poly {
    let mut r = empty_poly();
    for ta in a.terms.values() {
        for tb in b.terms.values() {
            add_term(&mut r, term_mul(ta, tb));
        }
    }
    r
}

/// Some(value) if `p` is a numeric constant (including 0), else None.
fn poly_as_const(p: &Poly) -> Option<Num> {
    if p.terms.is_empty() {
        return Some(Num::zero());
    }
    if p.terms.len() == 1 {
        if let Some(t) = p.terms.values().next() {
            if t.factors.is_empty() {
                return Some(t.coeff);
            }
        }
    }
    None
}

fn reciprocal(p: &Poly) -> Result<Poly, ExathError> {
    if p.terms.is_empty() {
        return Err(ExathError::domain("division by zero"));
    }
    if p.terms.len() == 1 {
        if let Some(t) = p.terms.values().next() {
            if t.coeff.is_zero() {
                return Err(ExathError::domain("division by zero"));
            }
            let mut factors = BTreeMap::new();
            for (k, (f, e)) in &t.factors {
                factors.insert(k.clone(), (f.clone(), -*e));
            }
            let mut r = empty_poly();
            add_term(&mut r, Term { coeff: t.coeff.recip(), factors });
            return Ok(r);
        }
    }
    // Multi-term denominator: keep the whole sum as an atomic base to the -1.
    Ok(factor_with_exp(Factor::SumBase(p.clone()), -1.0))
}

fn factor_with_exp(f: Factor, e: f64) -> Poly {
    if clean(e) == 0.0 {
        return poly_const(1.0);
    }
    let mut factors = BTreeMap::new();
    factors.insert(fkey(&f), (f, e));
    let t = Term { coeff: Num::one(), factors };
    let mut p = empty_poly();
    p.terms.insert(mkey(&t), t);
    p
}

fn build_pow(base: &Poly, exp: &Poly) -> Result<Poly, ExathError> {
    let c = match poly_as_const(exp) {
        Some(c) => clean(c.to_f64()),
        // Non-constant exponent (e.g. 2^x): keep opaque.
        None => return Ok(factor_with_exp(Factor::OpaquePow(base.clone(), exp.clone()), 1.0)),
    };

    // Zero base.
    if base.terms.is_empty() {
        if c > 0.0 {
            return Ok(empty_poly());
        }
        if c == 0.0 {
            return Ok(poly_const(1.0));
        }
        return Err(ExathError::domain("division by zero"));
    }
    if c == 0.0 {
        return Ok(poly_const(1.0));
    }

    let is_int = c.fract() == 0.0;
    if is_int && c.abs() <= MAX_POW_EXPAND as f64 {
        let n = c as i64;
        if n > 0 {
            let mut acc = poly_const(1.0);
            for _ in 0..n {
                acc = poly_mul(&acc, base);
            }
            return Ok(acc);
        }
        // n < 0
        let mut acc = poly_const(1.0);
        for _ in 0..(-n) {
            acc = poly_mul(&acc, base);
        }
        return reciprocal(&acc);
    }

    // Non-integer exponent, or integer too large to expand.
    if base.terms.len() == 1 {
        if let Some(t) = base.terms.values().next() {
            if t.coeff.to_f64() > 0.0 || is_int {
                let new_coeff = t.coeff.powf(c);
                if new_coeff.to_f64().is_finite() {
                    let mut factors = BTreeMap::new();
                    for (k, (f, e)) in &t.factors {
                        factors.insert(k.clone(), (f.clone(), e * c));
                    }
                    let mut r = empty_poly();
                    add_term(&mut r, Term { coeff: new_coeff, factors });
                    return Ok(r);
                }
            }
        }
    }
    // Sum base with non-expandable exponent → atomic base ^ c.
    Ok(factor_with_exp(Factor::SumBase(base.clone()), c))
}

fn opaque_binop(op: &BinOp, l: &Ast, r: &Ast) -> Result<Poly, ExathError> {
    let lhs = rebuild_poly(&build(l)?);
    let rhs = rebuild_poly(&build(r)?);
    Ok(poly_atom(Ast::BinOp(op.clone(), boxed(lhs), boxed(rhs))))
}

/// If `ast` has no variables and evaluates (in radians) to a clean integer,
/// return that integer — used to fold constant sub-expressions like `sin(0)`,
/// `cos(pi)`, `ln(1)`, `4!`. Non-integer constants (e.g. `ln(2)`, `sqrt(2)`) are
/// deliberately left symbolic.
fn fold_const(ast: &Ast) -> Option<f64> {
    if !collect_vars(ast).is_empty() {
        return None;
    }
    let vars: HashMap<String, Cx> = HashMap::new();
    let fns = UserFns::new();
    match eval_ast(ast, &vars, &fns, AngleMode::Rad) {
        Ok(cx) if cx.im == 0.0 && cx.re.is_finite() => {
            let c = clean(cx.re);
            if c.fract() == 0.0 {
                Some(c)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Factor `n = a²·b` with `b` square-free; returns `(a, b)` so √n = a·√b.
fn extract_square(n: i128) -> (i128, i128) {
    if n <= 0 {
        return (1, n.max(0));
    }
    let (mut a, mut b) = (1i128, n);
    let mut d = 2i128;
    while d * d <= b {
        while b % (d * d) == 0 {
            b /= d * d;
            a *= d;
        }
        d += 1;
    }
    (a, b)
}

/// Build the canonical normal form of an AST.
fn build(ast: &Ast) -> Result<Poly, ExathError> {
    if let Some(c) = fold_const(ast) {
        return Ok(poly_const(c));
    }
    match ast {
        Ast::Matrix(_) => Err(ExathError::domain(
            "matrix literal is not valid in a scalar expression",
        )),
        Ast::Number(n) => Ok(poly_const(*n)),
        Ast::Var(s) => Ok(poly_factor(Factor::Var(s.clone()))),
        Ast::UnaryNeg(u) => Ok(poly_neg(&build(u)?)),
        Ast::BinOp(op, l, r) => match op {
            BinOp::Add => Ok(poly_add(&build(l)?, &build(r)?)),
            BinOp::Sub => Ok(poly_add(&build(l)?, &poly_neg(&build(r)?))),
            BinOp::Mul => Ok(poly_mul(&build(l)?, &build(r)?)),
            BinOp::Div => {
                let denom = build(r)?;
                Ok(poly_mul(&build(l)?, &reciprocal(&denom)?))
            }
            BinOp::Pow => build_pow(&build(l)?, &build(r)?),
            _ => opaque_binop(op, l, r),
        },
        Ast::Call(name, args) => {
            // Surd simplification: sqrt of a non-negative integer constant
            // → a·sqrt(b) with b square-free (e.g. sqrt(8) → 2·sqrt(2)).
            if name == "sqrt" && args.len() == 1 {
                if let Some(v) = fold_const(&args[0]) {
                    if v >= 0.0 {
                        let n = v as i128;
                        let (a, b) = extract_square(n);
                        if b == 1 {
                            return Ok(poly_const(a as f64));
                        }
                        let surd = Factor::Func("sqrt".to_string(), vec![poly_const(b as f64)]);
                        return Ok(poly_scale(&poly_factor(surd), a as f64));
                    }
                }
            }
            let mut ps = Vec::with_capacity(args.len());
            for a in args {
                ps.push(build(a)?);
            }
            Ok(poly_factor(Factor::Func(name.clone(), ps)))
        }
        Ast::Factorial(u) => Ok(poly_atom(Ast::Factorial(boxed(rebuild_poly(&build(u)?))))),
        Ast::UnaryNot(u) => Ok(poly_atom(Ast::UnaryNot(boxed(rebuild_poly(&build(u)?))))),
    }
}

// ── Normal form → AST ─────────────────────────────────────────────────────────

fn degree(t: &Term) -> f64 {
    t.factors.values().map(|(_, e)| *e).sum()
}

fn rebuild_factor(f: &Factor) -> Ast {
    match f {
        Factor::Var(s) => Ast::Var(s.clone()),
        Factor::Func(name, args) => {
            Ast::Call(name.clone(), args.iter().map(rebuild_poly).collect())
        }
        Factor::SumBase(p) => rebuild_poly(p),
        Factor::OpaquePow(b, e) => pow(rebuild_poly(b), rebuild_poly(e)),
        Factor::Atom(_, ast) => ast.clone(),
    }
}

fn factor_ast(f: &Factor, e: f64) -> Ast {
    let base = rebuild_factor(f);
    if (e - 1.0).abs() < 1e-12 {
        base
    } else {
        pow(base, num(clean(e)))
    }
}

fn mul_fold(nodes: Vec<Ast>) -> Ast {
    let mut iter = nodes.into_iter();
    let mut acc = match iter.next() {
        Some(a) => a,
        None => return num(1.0),
    };
    for n in iter {
        acc = mul(acc, n);
    }
    acc
}

fn term_to_ast(t: &Term) -> (Ast, bool) {
    let neg = t.coeff.is_negative();
    let mag = t.coeff.abs();
    // Numerator coefficient value (cn) plus an optional integer denominator (cd),
    // so an exact rational like 1/3 renders as a division rather than 0.333…
    let (cn, cd): (f64, Option<i128>) = match mag.as_ratio() {
        Some((p, q)) => (p as f64, if q != 1 { Some(q) } else { None }),
        None => (clean(mag.to_f64()), None),
    };

    let mut nums: Vec<(&Factor, f64)> = Vec::new();
    let mut dens: Vec<(&Factor, f64)> = Vec::new();
    for (_, (f, e)) in &t.factors {
        let e = clean(*e);
        if e > 0.0 {
            nums.push((f, e));
        } else if e < 0.0 {
            dens.push((f, -e));
        }
    }
    nums.sort_by(|a, b| fkey(a.0).cmp(&fkey(b.0)));
    dens.sort_by(|a, b| fkey(a.0).cmp(&fkey(b.0)));

    let mut num_nodes: Vec<Ast> = Vec::new();
    if (cn - 1.0).abs() > 1e-12 || (nums.is_empty() && cd.is_none()) {
        num_nodes.push(num(cn));
    }
    for (f, e) in &nums {
        num_nodes.push(factor_ast(f, *e));
    }
    let num_ast = mul_fold(num_nodes);

    let mut den_nodes: Vec<Ast> = dens.iter().map(|(f, e)| factor_ast(f, *e)).collect();
    if let Some(q) = cd {
        den_nodes.push(num(q as f64));
    }
    let ast = if den_nodes.is_empty() {
        num_ast
    } else {
        div(num_ast, mul_fold(den_nodes))
    };
    (ast, neg)
}

fn neg_ast(a: Ast) -> Ast {
    match a {
        Ast::Number(n) => num(-n),
        // Push the sign into the numerator: -(p/q) → (-p)/q.
        Ast::BinOp(BinOp::Div, n, d) => Ast::BinOp(BinOp::Div, boxed(neg_ast(*n)), d),
        other => Ast::UnaryNeg(boxed(other)),
    }
}

fn rebuild_poly(p: &Poly) -> Ast {
    if p.terms.is_empty() {
        return num(0.0);
    }
    let mut items: Vec<&Term> = p.terms.values().collect();
    items.sort_by(|a, b| {
        let (da, db) = (degree(a), degree(b));
        db.partial_cmp(&da)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| mkey(a).cmp(&mkey(b)))
    });

    let mut iter = items.into_iter();
    let first = match iter.next() {
        Some(t) => t,
        None => return num(0.0),
    };
    let (mag, neg) = term_to_ast(first);
    let mut acc = if neg { neg_ast(mag) } else { mag };
    for t in iter {
        let (mag, neg) = term_to_ast(t);
        acc = if neg { sub(acc, mag) } else { add(acc, mag) };
    }
    acc
}

/// Simplify an expression to canonical normal form. Falls back to the input
/// unchanged if normalisation is not possible (e.g. literal division by zero).
///
/// Two candidates are produced — the direct normal form, and one where
/// `tan/cot/sec/csc` (and hyperbolic counterparts) are first rewritten to
/// `sin/cos` (`sinh/cosh`) — and the structurally smaller result is kept. This
/// way `tan(x)*cos(x) → sin(x)` and `sec(x)^2 - tan(x)^2 → 1` collapse, while a
/// lone `tan(x)` is preserved unchanged.
pub fn simplify_ast(ast: Ast) -> Ast {
    // Inverse-function pairs always simplify, so apply them up front.
    let ast = rewrite_inverses(&ast);
    let direct = match build(&ast) {
        Ok(p) => rebuild_poly(&finish(p)),
        Err(_) => return ast,
    };
    let rewritten = rewrite_reciprocal_trig(&ast);
    let candidate = match build(&rewritten) {
        Ok(p) => Some(rebuild_poly(&finish(p))),
        Err(_) => None,
    };
    match candidate {
        Some(c) if node_count(&c) < node_count(&direct) => c,
        _ => direct,
    }
}

/// Post-normalisation passes: Pythagorean/hyperbolic identities, exp & log laws.
fn finish(p: Poly) -> Poly {
    combine_logs(&combine_exps(&simplify_trig(p)))
}

/// Combine additive logarithm terms: Σ cᵢ·ln(aᵢ) → ln(∏ aᵢ^cᵢ). Only applied
/// when it reduces the number of terms (≥ 2 log terms), so e.g.
/// `ln(x) + ln(y) → ln(x·y)` and `ln(x) − ln(y) → ln(x/y)`.
fn combine_logs(p: &Poly) -> Poly {
    let mut logs: Vec<(Num, Poly)> = Vec::new();
    let mut others = empty_poly();
    for t in p.terms.values() {
        let mut is_log = false;
        if t.factors.len() == 1 {
            if let Some((_, (f, e))) = t.factors.iter().next() {
                if (e - 1.0).abs() < 1e-12 {
                    if let Factor::Func(name, args) = f {
                        if name == "ln" && args.len() == 1 {
                            logs.push((t.coeff, args[0].clone()));
                            is_log = true;
                        }
                    }
                }
            }
        }
        if !is_log {
            add_term(&mut others, t.clone());
        }
    }
    if logs.len() < 2 {
        return p.clone();
    }
    // product = ∏ argᵢ ^ coeffᵢ
    let mut product = poly_const(1.0);
    for (c, arg) in &logs {
        let powp = build_pow(arg, &poly_const_num(*c)).unwrap_or_else(|_| arg.clone());
        product = poly_mul(&product, &powp);
    }
    let lnf = Factor::Func("ln".to_string(), vec![product]);
    for t in poly_factor(lnf).terms.values() {
        add_term(&mut others, t.clone());
    }
    others
}

/// Apply inverse-function identities `exp(ln(u)) = u` and `ln(exp(u)) = u`.
fn rewrite_inverses(a: &Ast) -> Ast {
    match a {
        Ast::Matrix(rows) => Ast::Matrix(
            rows.iter().map(|r| r.iter().map(rewrite_inverses).collect()).collect(),
        ),
        Ast::Number(_) | Ast::Var(_) => a.clone(),
        Ast::UnaryNeg(u) => Ast::UnaryNeg(boxed(rewrite_inverses(u))),
        Ast::UnaryNot(u) => Ast::UnaryNot(boxed(rewrite_inverses(u))),
        Ast::Factorial(u) => Ast::Factorial(boxed(rewrite_inverses(u))),
        Ast::BinOp(op, l, r) => {
            Ast::BinOp(op.clone(), boxed(rewrite_inverses(l)), boxed(rewrite_inverses(r)))
        }
        Ast::Call(name, args) => {
            let args: Vec<Ast> = args.iter().map(rewrite_inverses).collect();
            if args.len() == 1 {
                if let Ast::Call(inner, iargs) = &args[0] {
                    if iargs.len() == 1
                        && ((name == "exp" && inner == "ln") || (name == "ln" && inner == "exp"))
                    {
                        return iargs[0].clone();
                    }
                }
            }
            Ast::Call(name.clone(), args)
        }
    }
}

/// Combine `exp` factors within each monomial: ∏ exp(aᵢ)^eᵢ = exp(Σ eᵢ·aᵢ).
fn combine_exps(p: &Poly) -> Poly {
    let mut result = empty_poly();
    for t in p.terms.values() {
        let mut arg_sum = empty_poly();
        let mut had_exp = false;
        let mut others: BTreeMap<String, (Factor, f64)> = BTreeMap::new();
        for (k, (f, e)) in &t.factors {
            if let Factor::Func(name, args) = f {
                if name == "exp" && args.len() == 1 {
                    had_exp = true;
                    arg_sum = poly_add(&arg_sum, &poly_scale(&args[0], *e));
                    continue;
                }
            }
            others.insert(k.clone(), (f.clone(), *e));
        }
        let mut nt = Term { coeff: t.coeff, factors: others };
        if had_exp && !arg_sum.terms.is_empty() {
            // exp(0) = 1 is dropped (empty arg_sum); otherwise re-attach exp(Σ).
            let f = Factor::Func("exp".to_string(), vec![arg_sum]);
            nt.factors.insert(fkey(&f), (f, 1.0));
        }
        add_term(&mut result, nt);
    }
    result
}

fn node_count(a: &Ast) -> usize {
    match a {
        Ast::Matrix(rows) => {
            1 + rows.iter().flatten().map(node_count).sum::<usize>()
        }
        Ast::Number(_) | Ast::Var(_) => 1,
        Ast::UnaryNeg(u) | Ast::UnaryNot(u) | Ast::Factorial(u) => 1 + node_count(u),
        Ast::BinOp(_, l, r) => 1 + node_count(l) + node_count(r),
        Ast::Call(_, args) => 1 + args.iter().map(node_count).sum::<usize>(),
    }
}

/// Rewrite reciprocal/derived trig & hyperbolic functions to sin/cos (sinh/cosh):
/// tan=sin/cos, cot=cos/sin, sec=1/cos, csc=1/sin, and the hyperbolic analogues.
fn rewrite_reciprocal_trig(a: &Ast) -> Ast {
    match a {
        Ast::Matrix(rows) => Ast::Matrix(
            rows.iter().map(|r| r.iter().map(rewrite_reciprocal_trig).collect()).collect(),
        ),
        Ast::Number(_) | Ast::Var(_) => a.clone(),
        Ast::UnaryNeg(u) => Ast::UnaryNeg(boxed(rewrite_reciprocal_trig(u))),
        Ast::UnaryNot(u) => Ast::UnaryNot(boxed(rewrite_reciprocal_trig(u))),
        Ast::Factorial(u) => Ast::Factorial(boxed(rewrite_reciprocal_trig(u))),
        Ast::BinOp(op, l, r) => Ast::BinOp(
            op.clone(),
            boxed(rewrite_reciprocal_trig(l)),
            boxed(rewrite_reciprocal_trig(r)),
        ),
        Ast::Call(name, args) => {
            let args: Vec<Ast> = args.iter().map(rewrite_reciprocal_trig).collect();
            if args.len() == 1 {
                let u = args[0].clone();
                let s = |f: &str| call1(f, u.clone());
                match name.as_str() {
                    "tan" => return div(s("sin"), s("cos")),
                    "cot" => return div(s("cos"), s("sin")),
                    "sec" => return div(num(1.0), s("cos")),
                    "csc" => return div(num(1.0), s("sin")),
                    "tanh" => return div(s("sinh"), s("cosh")),
                    "coth" => return div(s("cosh"), s("sinh")),
                    "sech" => return div(num(1.0), s("cosh")),
                    "csch" => return div(num(1.0), s("sinh")),
                    _ => {}
                }
            }
            Ast::Call(name.clone(), args)
        }
    }
}

// ── Pythagorean-family simplification ─────────────────────────────────────────
//
// Applies the identities  sin^2 + cos^2 = 1  and  cosh^2 - sinh^2 = 1  by
// rewriting an even power of one function in terms of the other, keeping the
// result only when it strictly reduces the term count. So `sin^2 + cos^2 → 1`
// and `cosh^2 - sinh^2 → 1` collapse, while a lone `sin(x)^2` is left untouched.

/// Arguments `u` for which both `fa(u)` and `fb(u)` occur, with their factors.
fn func_pair_args(p: &Poly, fa: &str, fb: &str) -> Vec<(Factor, Factor)> {
    let mut by_arg: BTreeMap<String, (Poly, bool, bool)> = BTreeMap::new();
    for t in p.terms.values() {
        for (_, (f, _)) in &t.factors {
            if let Factor::Func(name, args) = f {
                if args.len() == 1 && (name == fa || name == fb) {
                    let entry = by_arg
                        .entry(pkey(&args[0]))
                        .or_insert_with(|| (args[0].clone(), false, false));
                    if name == fa {
                        entry.1 = true;
                    } else {
                        entry.2 = true;
                    }
                }
            }
        }
    }
    by_arg
        .into_values()
        .filter(|(_, a, b)| *a && *b)
        .map(|(arg, _, _)| {
            (
                Factor::Func(fa.to_string(), vec![arg.clone()]),
                Factor::Func(fb.to_string(), vec![arg]),
            )
        })
        .collect()
}

/// Rewrite `target^2 → c0 + s*other^2` throughout the polynomial and renormalise.
/// (sin: c0=1,s=-1,other=cos; cosh: c0=1,s=1,other=sinh; sinh: c0=-1,s=1,other=cosh)
fn reduce_identity(
    p: &Poly,
    target_key: &str,
    other_key: &str,
    other_factor: &Factor,
    c0: f64,
    s: f64,
) -> Poly {
    let mut result = empty_poly();
    let mut stack: Vec<Term> = p.terms.values().cloned().collect();
    while let Some(term) = stack.pop() {
        let exp = term.factors.get(target_key).map(|(_, e)| clean(*e));
        match exp {
            Some(e) if e.fract() == 0.0 && e >= 2.0 => {
                let mut base = term.clone();
                if let Some(entry) = base.factors.get_mut(target_key) {
                    entry.1 = clean(entry.1 - 2.0);
                    if entry.1 == 0.0 {
                        base.factors.remove(target_key);
                    }
                }
                // term A: c0 * base
                let mut a = base.clone();
                a.coeff = a.coeff.mul(&Num::from_f64(c0));
                stack.push(a);
                // term B: s * base * other^2
                let mut b = base;
                b.coeff = b.coeff.mul(&Num::from_f64(s));
                match b.factors.get_mut(other_key) {
                    Some(entry) => {
                        entry.1 = clean(entry.1 + 2.0);
                        if entry.1 == 0.0 {
                            b.factors.remove(other_key);
                        }
                    }
                    None => {
                        b.factors
                            .insert(other_key.to_string(), (other_factor.clone(), 2.0));
                    }
                }
                stack.push(b);
            }
            _ => add_term(&mut result, term),
        }
    }
    result
}

/// One improving Pythagorean/hyperbolic reduction, or None if none helps.
fn try_reduce(p: &Poly) -> Option<Poly> {
    let attempt = |tk: &str, ok: &str, of: &Factor, c0: f64, s: f64| -> Option<Poly> {
        let r = reduce_identity(p, tk, ok, of, c0, s);
        if r.terms.len() < p.terms.len() {
            Some(r)
        } else {
            None
        }
    };
    // sin^2 + cos^2 = 1
    for (sinf, cosf) in func_pair_args(p, "sin", "cos") {
        let (sk, ck) = (fkey(&sinf), fkey(&cosf));
        if let Some(r) = attempt(&sk, &ck, &cosf, 1.0, -1.0) {
            return Some(r);
        }
        if let Some(r) = attempt(&ck, &sk, &sinf, 1.0, -1.0) {
            return Some(r);
        }
    }
    // cosh^2 - sinh^2 = 1
    for (sinhf, coshf) in func_pair_args(p, "sinh", "cosh") {
        let (sk, ck) = (fkey(&sinhf), fkey(&coshf));
        // cosh^2 = 1 + sinh^2
        if let Some(r) = attempt(&ck, &sk, &sinhf, 1.0, 1.0) {
            return Some(r);
        }
        // sinh^2 = cosh^2 - 1
        if let Some(r) = attempt(&sk, &ck, &coshf, -1.0, 1.0) {
            return Some(r);
        }
    }
    None
}

/// Apply Pythagorean/hyperbolic identities until no improving step remains.
fn simplify_trig(p: Poly) -> Poly {
    let mut current = p;
    // Each improving step lowers the term count, so this terminates.
    for _ in 0..128 {
        match try_reduce(&current) {
            Some(r) => current = r,
            None => break,
        }
    }
    current
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
        Ast::Matrix(_) => 5,
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
        Ast::Matrix(rows) => {
            let body: Vec<String> = rows
                .iter()
                .map(|r| {
                    let elems: Vec<String> = r.iter().map(unparse).collect();
                    format!("[{}]", elems.join(", "))
                })
                .collect();
            format!("[{}]", body.join(", "))
        }
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
        let numeric = match crate::numerics::deriv(expr, var, x, AngleMode::Rad) {
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

    #[test]
    fn collects_like_terms_exact() {
        let cases = [
            ("x + x", "2 * x"),
            ("2*x + 3*x", "5 * x"),
            ("x*x", "x^2"),
            ("x*x*x", "x^3"),
            ("(x + 1)^2", "x^2 + 2 * x + 1"),
            ("(x + 2)*(x - 2)", "x^2 - 4"),
            ("2*(x + 3)", "2 * x + 6"),
            ("sin(x) + sin(x)", "2 * sin(x)"),
            ("x*y + y*x", "2 * x * y"),
            ("x^2 / x", "x"),
            ("x - x", "0"),
            ("0*x + 5", "5"),
        ];
        for (input, expected) in cases {
            match simplify_expr(input) {
                Ok(s) => assert_eq!(s, expected, "simplify({})", input),
                Err(e) => assert!(false, "simplify({}) errored: {}", input, e),
            }
        }
    }

    #[test]
    fn pythagorean_identity() {
        let cases = [
            ("sin(x)^2 + cos(x)^2", "1"),
            ("cos(x)^2 + sin(x)^2", "1"),
            ("2*sin(x)^2 + 2*cos(x)^2", "2"),
            ("x*sin(x)^2 + x*cos(x)^2", "x"),
            ("sin(y)^2 + cos(y)^2 + x", "x + 1"),
            // not beneficial → left untouched
            ("sin(x)^2", "sin(x)^2"),
            ("sin(x)^2 * cos(x)^2", "cos(x)^2 * sin(x)^2"),
        ];
        for (input, expected) in cases {
            match simplify_expr(input) {
                Ok(s) => assert_eq!(s, expected, "simplify({})", input),
                Err(e) => assert!(false, "simplify({}) errored: {}", input, e),
            }
        }
    }

    #[test]
    fn trig_rewrites_and_hyperbolic() {
        let cases = [
            ("tan(x) * cos(x)", "sin(x)"),
            ("sec(x)^2 - tan(x)^2", "1"),
            ("cosh(x)^2 - sinh(x)^2", "1"),
            ("x*cosh(x)^2 - x*sinh(x)^2", "x"),
            // lone reciprocal funcs are preserved (rewrite would be larger)
            ("tan(x)", "tan(x)"),
            ("sec(x)", "sec(x)"),
        ];
        for (input, expected) in cases {
            match simplify_expr(input) {
                Ok(s) => assert_eq!(s, expected, "simplify({})", input),
                Err(e) => assert!(false, "simplify({}) errored: {}", input, e),
            }
        }
    }

    #[test]
    fn integration_exact() {
        let cases = [
            ("1", "x", "x"),
            ("x", "x", "x^2 / 2"),
            ("x^2", "x", "x^3 / 3"),
            ("2*x + 3", "x", "x^2 + 3 * x"),
            ("1/x", "x", "ln(x)"),
            ("cos(x)", "x", "sin(x)"),
            ("exp(x)", "x", "exp(x)"),
            ("1/(x + 1)", "x", "ln(x + 1)"),
            ("1/(2*x + 1)", "x", "ln(2 * x + 1) / 2"),
            ("1/(x^2 + 1)", "x", "atan(x)"),
            ("ln(x)", "x", "ln(x) * x - x"),
            ("tan(x)", "x", "-ln(cos(x))"),
        ];
        for (input, var, expected) in cases {
            match antiderivative(input, var) {
                Ok(s) => assert_eq!(s, expected, "integral({}, {})", input, var),
                Err(e) => assert!(false, "integral({}, {}) errored: {}", input, var, e),
            }
        }
    }

    #[test]
    fn definite_integrals() {
        // ∫₀¹ x² dx = 1/3
        assert_eq!(integrate_definite("x^2", "x", 0.0, 1.0).unwrap_or_default(), "1 / 3");
        // ∫₀² (2x) dx = 4
        assert_eq!(integrate_definite("2*x", "x", 0.0, 2.0).unwrap_or_default(), "4");
        // ∫₁ᵉ 1/x dx = 1
        let e = std::f64::consts::E;
        assert_eq!(integrate_definite("1/x", "x", 1.0, e).unwrap_or_default(), "1");
    }

    #[test]
    fn partial_fraction_integration() {
        use crate::{AngleMode, CalcResult, Session};
        // ∫ 1/((x-1)(x-2)) dx and ∫ (x+3)/(x^2-3x+2) dx — verify via d/dx = integrand.
        let cases = [
            "1/((x - 1)*(x - 2))",
            "(x + 3)/(x^2 - 3*x + 2)",
            "x/(x^2 - 1)",
            "1/(x^2 + x + 1)",   // irreducible quadratic → atan
            "(2*x + 1)/(x^2 + x + 1)",
            "x^2*ln(x)",          // by parts with ln
            "sin(x)^2",           // power-reduction
            "cos(x)^2",
            "sin(2*x)^2",
            "exp(x)*sin(x)",      // cyclic by parts
            "exp(2*x)*cos(x)",
            "tan(x)",             // trig antiderivatives
            "cot(x)",
            "sec(x)",
            "csc(x)",
            "sec(x)^2",
            "csc(x)^2",
        ];
        for f in cases {
            let integral = match antiderivative(f, "x") {
                Ok(s) => s,
                Err(e) => {
                    assert!(false, "integral({}) errored: {}", f, e);
                    continue;
                }
            };
            let back = differentiate(&integral, "x").unwrap_or_default();
            for x in [3.5, 4.2, -2.7] {
                let ev = |src: &str| -> Option<f64> {
                    let mut s = Session::new(AngleMode::Rad);
                    s.set_var("x", x, 0.0);
                    match s.eval(src) {
                        Ok(CalcResult::Real(v)) => Some(v),
                        _ => None,
                    }
                };
                if let (Some(a), Some(b)) = (ev(f), ev(&back)) {
                    assert!(
                        (a - b).abs() < 1e-6,
                        "d/dx integral({}) = {} mismatch at x={}: {} vs {}",
                        f, back, x, b, a
                    );
                }
            }
        }
    }

    #[test]
    fn limits_at_infinity() {
        assert!((limit_value(&parse_str("ln(x)/x").unwrap(), "x", f64::INFINITY).unwrap()).abs() < 1e-9);
        assert!((limit_value(&parse_str("1/x").unwrap(), "x", f64::INFINITY).unwrap()).abs() < 1e-9);
        assert!((limit_value(&parse_str("(1 + 1/x)^x").unwrap(), "x", f64::INFINITY).unwrap()
            - std::f64::consts::E).abs() < 1e-4);
    }

    #[test]
    fn rational_integration_repeated_roots() {
        use crate::{AngleMode, CalcResult, Session};
        // These need repeated/multi-root partial fractions:
        let cases = [
            "1/(x^2 - 1)",          // distinct roots
            "1/((x - 1)^2)",        // repeated root
            "x/((x - 1)^2*(x + 2))",// mixed multiplicity
            "1/(x^2*(x - 1))",      // repeated root at 0
        ];
        for f in cases {
            let integral = match antiderivative(f, "x") {
                Ok(s) => s,
                Err(e) => {
                    assert!(false, "could not integrate {}: {}", f, e);
                    continue;
                }
            };
            let d = differentiate(&integral, "x").unwrap_or_default();
            for x in [2.3_f64, 3.1, 4.7, -3.2] {
                let ev = |src: &str| -> Option<f64> {
                    let mut s = Session::new(AngleMode::Rad);
                    s.set_var("x", x, 0.0);
                    match s.eval(src) {
                        Ok(CalcResult::Real(v)) => Some(v),
                        _ => None,
                    }
                };
                if let (Some(a), Some(b)) = (ev(f), ev(&d)) {
                    assert!((a - b).abs() < 1e-5, "∫{} wrong at x={}: {} vs {}", f, x, b, a);
                }
            }
        }
    }

    #[test]
    fn definite_integral_numeric_fallback() {
        // exact path: ∫_0^1 x^2 dx = 1/3
        let exact = integrate_definite("x^2", "x", 0.0, 1.0).unwrap();
        assert!((eval_const_f64(&parse_str(&exact).unwrap()).unwrap() - 1.0 / 3.0).abs() < 1e-9);
        // no elementary antiderivative → Simpson: ∫_0^1 e^(x^2) dx ≈ 1.4626517
        let v = integrate_definite("exp(x^2)", "x", 0.0, 1.0)
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert!((v - 1.46265174).abs() < 1e-5);
        // ∫_0^pi sin(x) dx = 2
        let s = integrate_definite("sin(x)", "x", 0.0, std::f64::consts::PI).unwrap();
        let sv = eval_const_f64(&parse_str(&s).unwrap()).unwrap();
        assert!((sv - 2.0).abs() < 1e-6);
    }

    #[test]
    fn integration_by_substitution() {
        use crate::{AngleMode, CalcResult, Session};
        // These require u-substitution (beyond the curated rules):
        let integrands = [
            "2*x*cos(x^2)",   // → sin(x^2)
            "x*exp(x^2)",     // → exp(x^2)/2
            "cos(x)*sin(x)",  // → sin(x)^2/2
            "2*x*(x^2 + 1)^3",// → (x^2+1)^4/4
            "exp(sin(x))*cos(x)", // → exp(sin(x))
        ];
        let points = [0.4, 0.9, 1.6];
        for f in integrands {
            let integral = match antiderivative(f, "x") {
                Ok(s) => s,
                Err(e) => {
                    assert!(false, "could not integrate {}: {}", f, e);
                    continue;
                }
            };
            let back = differentiate(&integral, "x").unwrap_or_default();
            for x in points {
                let ev = |src: &str| -> Option<f64> {
                    let mut s = Session::new(AngleMode::Rad);
                    s.set_var("x", x, 0.0);
                    match s.eval(src) {
                        Ok(CalcResult::Real(v)) => Some(v),
                        _ => None,
                    }
                };
                if let (Some(a), Some(b)) = (ev(f), ev(&back)) {
                    assert!(
                        (a - b).abs() < 1e-6,
                        "∫{} = {} wrong at x={}: d/dx={} vs {}",
                        f, integral, x, b, a
                    );
                }
            }
        }
    }

    /// Fundamental theorem: d/dx of the antiderivative must equal the integrand.
    #[test]
    fn integration_inverts_differentiation() {
        use crate::{AngleMode, CalcResult, Session};
        let integrands = [
            "x^3 + 2*x - 5",
            "x^2",
            "1/x",
            "sin(2*x)",
            "cos(x)",
            "exp(3*x)",
            "exp(x) + x",
            "4*x^3 - x",
            "x*exp(x)",
            "x*sin(x)",
            "x^2*cos(x)",
            "x*exp(2*x)",
        ];
        let points = [0.7, 1.9, 2.6, -1.3];
        for f in integrands {
            let integral = match antiderivative(f, "x") {
                Ok(s) => s,
                Err(e) => {
                    assert!(false, "integral({}) errored: {}", f, e);
                    continue;
                }
            };
            let back = match differentiate(&integral, "x") {
                Ok(s) => s,
                Err(e) => {
                    assert!(false, "d/dx({}) errored: {}", integral, e);
                    continue;
                }
            };
            for x in points {
                let eval = |src: &str| -> Option<f64> {
                    let mut s = Session::new(AngleMode::Rad);
                    s.set_var("x", x, 0.0);
                    match s.eval(src) {
                        Ok(CalcResult::Real(v)) => Some(v),
                        _ => None,
                    }
                };
                if let (Some(a), Some(b)) = (eval(f), eval(&back)) {
                    if a.is_finite() && b.is_finite() {
                        assert!(
                            (a - b).abs() < 1e-6,
                            "d/dx integral({}) = {} mismatch at x={}: {} vs {}",
                            f,
                            back,
                            x,
                            b,
                            a
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn taylor_series() {
        // exp around 0: 1 + x + x^2/2 + x^3/6
        assert_eq!(
            taylor("exp(x)", "x", 0.0, 3).unwrap_or_default(),
            "x^3 / 6 + x^2 / 2 + x + 1"
        );
        // sin around 0: x - x^3/6 + x^5/120
        assert_eq!(
            taylor("sin(x)", "x", 0.0, 5).unwrap_or_default(),
            "x^5 / 120 - x^3 / 6 + x"
        );
        // 1/(1-x) around 0: 1 + x + x^2 + x^3
        assert_eq!(
            taylor("1/(1 - x)", "x", 0.0, 3).unwrap_or_default(),
            "x^3 + x^2 + x + 1"
        );
    }

    #[test]
    fn limits() {
        // sin(x)/x → 1
        assert!((limit_value(&parse_str("sin(x)/x").unwrap(), "x", 0.0).unwrap() - 1.0).abs() < 1e-9);
        // (x^2 - 1)/(x - 1) → 2 at x=1
        assert!((limit_value(&parse_str("(x^2 - 1)/(x - 1)").unwrap(), "x", 1.0).unwrap() - 2.0).abs() < 1e-9);
        // (1 - cos(x))/x^2 → 1/2
        assert!((limit_value(&parse_str("(1 - cos(x))/x^2").unwrap(), "x", 0.0).unwrap() - 0.5).abs() < 1e-6);
        // continuous: x^2 + 1 at 3 → 10
        assert!((limit_value(&parse_str("x^2 + 1").unwrap(), "x", 3.0).unwrap() - 10.0).abs() < 1e-9);
        // at infinity: (2x+1)/(x+3) → 2
        assert!((limit_value(&parse_str("(2*x + 1)/(x + 3)").unwrap(), "x", f64::INFINITY).unwrap() - 2.0).abs() < 1e-6);
        // 1/x → 0 as x → ∞
        assert!(limit_value(&parse_str("1/x").unwrap(), "x", f64::INFINITY).unwrap().abs() < 1e-6);
    }

    #[test]
    fn expand_identities() {
        let cases = [
            ("ln(x*y)", "ln(x) + ln(y)"),
            ("ln(x/y)", "ln(x) - ln(y)"),
            ("ln(x^2)", "2 * ln(x)"),
            ("exp(x + y)", "exp(x) * exp(y)"),
            ("(x + 1)^2", "x^2 + 2 * x + 1"),
        ];
        for (input, expected) in cases {
            match expand(input) {
                Ok(s) => assert_eq!(s, expected, "expand({})", input),
                Err(e) => assert!(false, "expand({}) errored: {}", input, e),
            }
        }
    }

    /// sin/cos sum & double-angle expansions must be value-correct.
    #[test]
    fn expand_trig_value() {
        use crate::{AngleMode, CalcResult, Session};
        let exprs = ["sin(x + y)", "cos(x + y)", "sin(2*x)", "cos(2*x)", "sin(x - y)"];
        let points = [(0.6, 1.1), (2.3, -0.7)];
        for e in exprs {
            let ex = match expand(e) {
                Ok(s) => s,
                Err(err) => {
                    assert!(false, "expand({}) errored: {}", e, err);
                    continue;
                }
            };
            for (x, y) in points {
                let eval = |src: &str| -> Option<f64> {
                    let mut s = Session::new(AngleMode::Rad);
                    s.set_var("x", x, 0.0);
                    s.set_var("y", y, 0.0);
                    match s.eval(src) {
                        Ok(CalcResult::Real(v)) => Some(v),
                        _ => None,
                    }
                };
                if let (Some(a), Some(b)) = (eval(e), eval(&ex)) {
                    assert!(
                        (a - b).abs() < 1e-9,
                        "expand({}) = {} mismatch at ({},{}): {} vs {}",
                        e, ex, x, y, a, b
                    );
                }
            }
        }
    }

    #[test]
    fn poly_gcd_and_nsolve() {
        // gcd((x-1)(x-2), (x-1)(x-3)) = x-1
        assert_eq!(poly_gcd("x^2 - 3*x + 2", "x^2 - 4*x + 3", "x").unwrap_or_default(), "x - 1");
        // gcd(x^2-1, x-1) = x-1
        assert_eq!(poly_gcd("x^2 - 1", "x - 1", "x").unwrap_or_default(), "x - 1");
        // Newton: sqrt(2) as root of x^2-2 near 1.5
        assert!((newton(&parse_str("x^2 - 2").unwrap(), "x", 1.5).unwrap() - 2.0_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn dsolve_linear_ode() {
        // y'' + 3y' + 2y = 0  → roots -1, -2
        assert_eq!(dsolve(&[1.0, 3.0, 2.0], "x").unwrap_or_default(), "C1 * exp(-2*x) + C2 * exp(-x)");
        // y'' + y = 0  → ±i → C1 cos(x) + C2 sin(x)
        assert_eq!(dsolve(&[1.0, 0.0, 1.0], "x").unwrap_or_default(), "(C1*cos(x) + C2*sin(x))");
        // y'' - 2y' + y = 0  → double root 1 → C1 e^x + C2 x e^x
        assert_eq!(dsolve(&[1.0, -2.0, 1.0], "x").unwrap_or_default(), "C1 * exp(x) + C2 * x * exp(x)");
        // y' - 3y = 0 → root 3
        assert_eq!(dsolve(&[1.0, -3.0], "x").unwrap_or_default(), "C1 * exp(3*x)");
    }

    #[test]
    fn laplace_transforms() {
        let cases = [
            ("1", "t", "s", "1 / s"),
            ("t", "t", "s", "1 / s^2"),
            ("t^2", "t", "s", "2 / s^3"),
            ("exp(3*t)", "t", "s", "1 / (s - 3)"),
            ("sin(2*t)", "t", "s", "2 / (s^2 + 4)"),
            ("cos(t)", "t", "s", "s / (s^2 + 1)"),
        ];
        for (f, t, s, expected) in cases {
            match laplace(f, t, s) {
                Ok(out) => assert_eq!(out, expected, "laplace({})", f),
                Err(e) => assert!(false, "laplace({}) errored: {}", f, e),
            }
        }
    }

    #[test]
    fn faulhaber_sums() {
        // Σ_{k=1}^n k = (n² + n)/2
        assert_eq!(sum_closed("k", "k", "n").unwrap_or_default(), "n^2 / 2 + n / 2");
        // Σ_{k=1}^n k² = (2n³ + 3n² + n)/6  → check value at n=10 equals 385
        let mut s = crate::Session::new(crate::AngleMode::Rad);
        let closed = sum_closed("k^2", "k", "n").unwrap_or_default();
        s.set_var("n", 10.0, 0.0);
        match s.eval(&closed) {
            Ok(crate::CalcResult::Real(v)) => assert!((v - 385.0).abs() < 1e-6),
            other => assert!(false, "{:?}", other),
        }
    }

    #[test]
    fn factoring() {
        let cases = [
            ("x^2 - 5*x + 6", "x", "(x - 2) * (x - 3)"),
            ("x^2 - 4", "x", "(x + 2) * (x - 2)"),
            ("x^3 - x", "x", "x * (x + 1) * (x - 1)"),
            ("2*x^2 - 2", "x", "2 * (x + 1) * (x - 1)"),
        ];
        for (input, var, expected) in cases {
            match factor(input, var) {
                Ok(s) => assert_eq!(s, expected, "factor({})", input),
                Err(e) => assert!(false, "factor({}) errored: {}", input, e),
            }
        }
    }

    #[test]
    fn solve_equations() {
        let lin = solve("2*x - 6", "x").unwrap_or_default();
        assert_eq!(lin, vec!["3".to_string()]);

        let quad = solve("x^2 - 4", "x").unwrap_or_default();
        assert_eq!(quad, vec!["2".to_string(), "-2".to_string()]);

        // x^2 - 5x + 6 = (x-2)(x-3)
        let quad2 = solve("x^2 - 5*x + 6", "x").unwrap_or_default();
        assert_eq!(quad2, vec!["3".to_string(), "2".to_string()]);

        // equality form, fractional root: 2x + 1 = 0 → -1/2
        let frac = solve("2*x + 1 == 0", "x").unwrap_or_default();
        assert_eq!(frac, vec!["-1 / 2".to_string()]);

        // double root x^2 - 2x + 1 → 1 (deduped)
        let dbl = solve("x^2 - 2*x + 1", "x").unwrap_or_default();
        assert_eq!(dbl, vec!["1".to_string()]);

        // cubic with rational roots: (x-1)(x-2)(x+3) = x^3 - 7x + 6
        let mut cubic = solve("x^3 - 7*x + 6", "x").unwrap_or_default();
        cubic.sort();
        assert_eq!(cubic, vec!["-3".to_string(), "1".to_string(), "2".to_string()]);

        // cubic with a zero root: x^3 - x = x(x-1)(x+1)
        let mut z = solve("x^3 - x", "x").unwrap_or_default();
        z.sort();
        assert_eq!(z, vec!["-1".to_string(), "0".to_string(), "1".to_string()]);

        // complex roots x^2 + 1 → ± i
        let cplx = solve("x^2 + 1", "x").unwrap_or_default();
        assert_eq!(cplx, vec!["i".to_string(), "-i".to_string()]);

        // exact transcendental via substitution: exp(x) = 2 → x = ln(2)
        let ex = solve("exp(x) - 2", "x").unwrap_or_default();
        assert!(ex.iter().any(|r| {
            let mut s = crate::Session::new(crate::AngleMode::Rad);
            matches!(s.eval(r), Ok(crate::CalcResult::Real(v)) if (v - 2.0_f64.ln()).abs() < 1e-6)
        }), "exp(x)=2 should give ln(2), got {:?}", ex);

        // transcendental → verified numeric real roots
        let tr = solve("exp(x) - x - 2", "x").unwrap_or_default();
        assert!(tr.iter().any(|r| r.parse::<f64>().map(|v| (v - 1.146193).abs() < 1e-4).unwrap_or(false)));
        let sn = solve("2*sin(x) - 1", "x").unwrap_or_default();
        // x = pi/6 ≈ 0.5236 is among the roots
        assert!(sn.iter().any(|r| r.parse::<f64>().map(|v| (v - 0.523599).abs() < 1e-4).unwrap_or(false)));

        assert!(solve("5", "x").is_err()); // no solution
        // x^3 - 1 = (x-1)(x^2+x+1): one real + two complex roots
        let cube1 = solve("x^3 - 1", "x").unwrap_or_default();
        assert_eq!(cube1.len(), 3);
        assert!(cube1.contains(&"1".to_string()));
        // irrational-only cubic: numeric fallback returns 3 roots (1 real + 2 complex)
        let irr = solve("x^3 - 2", "x").unwrap_or_default();
        assert_eq!(irr.len(), 3);
        // the real root is the cube root of 2 ≈ 1.2599
        assert!(irr.iter().any(|r| r.parse::<f64>().map(|v| (v - 1.259921).abs() < 1e-4).unwrap_or(false)));
    }

    #[test]
    fn surd_simplification() {
        let cases = [
            ("sqrt(8)", "2 * sqrt(2)"),
            ("sqrt(50)", "5 * sqrt(2)"),
            ("sqrt(12)", "2 * sqrt(3)"),
            ("sqrt(9)", "3"),
            ("sqrt(2)", "sqrt(2)"),
            ("sqrt(18) + sqrt(2)", "4 * sqrt(2)"), // 3√2 + √2
        ];
        for (input, expected) in cases {
            match simplify_expr(input) {
                Ok(s) => assert_eq!(s, expected, "simplify({})", input),
                Err(e) => assert!(false, "simplify({}) errored: {}", input, e),
            }
        }
    }

    #[test]
    fn exact_rationals() {
        let cases = [
            ("1/3 + 1/3", "2 / 3"),
            ("x/2 + x/2", "x"),
            ("x/3", "x / 3"),
            ("1/2 * x", "x / 2"),
            ("2/4", "1 / 2"),       // reduced to lowest terms
            ("1/2 + 1/3", "5 / 6"),
            ("x^2 / 4 + x^2 / 4", "x^2 / 2"),
        ];
        for (input, expected) in cases {
            match simplify_expr(input) {
                Ok(s) => assert_eq!(s, expected, "simplify({})", input),
                Err(e) => assert!(false, "simplify({}) errored: {}", input, e),
            }
        }
    }

    #[test]
    fn constant_folding() {
        let cases = [
            ("sin(0)", "0"),
            ("cos(0)", "1"),
            ("ln(1)", "0"),
            ("cos(pi)", "-1"),
            ("sqrt(4)", "2"),
            ("4!", "24"),
            ("2^10", "1024"),
            ("ln(2)", "ln(2)"), // non-integer constant stays symbolic
        ];
        for (input, expected) in cases {
            match simplify_expr(input) {
                Ok(s) => assert_eq!(s, expected, "simplify({})", input),
                Err(e) => assert!(false, "simplify({}) errored: {}", input, e),
            }
        }
    }

    #[test]
    fn combine_log_terms() {
        assert_eq!(simplify_expr("ln(x) + ln(y)").unwrap_or_default(), "ln(x * y)");
        assert_eq!(simplify_expr("ln(x) - ln(y)").unwrap_or_default(), "ln(x / y)");
        // single log term is left as-is
        assert_eq!(simplify_expr("2*ln(x)").unwrap_or_default(), "2 * ln(x)");
    }

    #[test]
    fn exp_log_laws() {
        let cases = [
            ("exp(ln(x))", "x"),
            ("ln(exp(x))", "x"),
            ("exp(x) * exp(y)", "exp(x + y)"),
            ("exp(x)^2", "exp(2 * x)"),
            ("exp(x) / exp(x)", "1"),
        ];
        for (input, expected) in cases {
            match simplify_expr(input) {
                Ok(s) => assert_eq!(s, expected, "simplify({})", input),
                Err(e) => assert!(false, "simplify({}) errored: {}", input, e),
            }
        }
    }

    /// Strongest guarantee: a simplified expression must evaluate to the same
    /// number as the original at several sample points. Catches any algebraic
    /// normalisation bug regardless of output form.
    #[test]
    fn simplify_preserves_value() {
        use crate::{AngleMode, CalcResult, Session};

        let exprs = [
            "x + x",
            "2*x + 3*x - x",
            "(x + 1)^3",
            "(x + 2)*(x - 2)",
            "x/(x + 1)",
            "(x^2 - 1)/(x - 1)",
            "x^2 / x",
            "2*(x + 3) - x",
            "sin(x) + 2*sin(x)",
            "x*y + y*x - x*y",
            "x^2*y - y*x^2 + 7",
            "(x + y)^2",
            "1/(x + 1) + 1/(x + 1)",
            "x^3 - 3*x^2 + 3*x - 1",
            "cos(x)^2 + sin(x)^2",
            "tan(x) * cos(x)",
            "sec(x)^2 - tan(x)^2",
            "cosh(x)^2 - sinh(x)^2",
            "x*sin(x)^2 + x*cos(x)^2",
        ];
        let points = [(0.7, 1.3), (2.5, -0.4), (3.1, 2.2), (-1.6, 0.9)];

        for e in exprs {
            let simplified = match simplify_expr(e) {
                Ok(s) => s,
                Err(err) => {
                    assert!(false, "simplify({}) errored: {}", e, err);
                    continue;
                }
            };
            for (x, y) in points {
                let eval = |src: &str| -> Option<f64> {
                    let mut s = Session::new(AngleMode::Rad);
                    s.set_var("x", x, 0.0);
                    s.set_var("y", y, 0.0);
                    match s.eval(src) {
                        Ok(CalcResult::Real(v)) => Some(v),
                        Ok(CalcResult::Complex(re, _)) => Some(re),
                        Err(_) => None,
                    }
                };
                if let (Some(a), Some(b)) = (eval(e), eval(&simplified)) {
                    if a.is_finite() && b.is_finite() {
                        assert!(
                            (a - b).abs() < 1e-6,
                            "value mismatch for {} -> {} at x={}, y={}: {} vs {}",
                            e,
                            simplified,
                            x,
                            y,
                            a,
                            b
                        );
                    }
                }
            }
        }
    }
}
