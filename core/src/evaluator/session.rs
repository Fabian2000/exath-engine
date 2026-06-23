use crate::angle_mode::AngleMode;
use crate::ast::{eval_ast, parse_str, Ast, UserFns};
use crate::error::ExathError;
use crate::symbolic;
use super::calc_result::CalcResult;
use super::cx::Cx;
use std::collections::HashMap;

/// Result of [`Session::eval_line`]: either a computed number or, for symbolic
/// forms like `diff(...)` / `simplify(...)`, an expression rendered as a string.
#[derive(Debug, Clone, PartialEq)]
pub enum LineResult {
    /// A numeric (real or complex) value.
    Value(CalcResult),
    /// A symbolic expression (e.g. the derivative `2 * x`).
    Expression(String),
}

/// A stateful evaluation context that persists variables and user-defined functions
/// across multiple eval calls.
///
/// ```
/// use exath_engine::{Session, AngleMode};
/// let mut s = Session::new(AngleMode::Rad);
/// s.eval("a = 5").unwrap();
/// s.eval("b = sqrt(a)").unwrap();
/// let r = s.eval("a + b").unwrap();  // CalcResult::Real(7.2360...)
/// // User-defined functions
/// s.eval("f(x) = x^2 + 1").unwrap();
/// let r2 = s.eval("f(4)").unwrap();  // CalcResult::Real(17.0)
/// ```
pub struct Session {
    pub angle_mode: AngleMode,
    vars: HashMap<String, Cx>,
    fns: UserFns,
    /// Symbolic variables — names bound to an expression (e.g. via
    /// `g = diff(x^2, x)`). Used only by [`Session::eval_line`].
    sym_vars: HashMap<String, Ast>,
    /// Sign assumptions on variables (+1 = nonnegative, −1 = nonpositive),
    /// set via `assume(x > 0)`; consulted by `simplify`.
    assumptions: HashMap<String, i8>,
}

impl Session {
    pub fn new(angle_mode: AngleMode) -> Self {
        Session {
            angle_mode,
            vars: HashMap::new(),
            fns: UserFns::new(),
            sym_vars: HashMap::new(),
            assumptions: HashMap::new(),
        }
    }

    /// Evaluate one line to a NUMERIC result. Handles three forms:
    /// - `f(x, y) = expr` — defines a user function (stored, returns 0)
    /// - `ident = expr`   — assigns a variable, returns its value
    /// - `expr`           — evaluates the expression, returns its value
    ///
    /// This is the numeric-only path: symbolic forms such as `diff(x^2, x)` or
    /// `factor(...)` are NOT understood here and return an error. Use
    /// [`Session::eval_line`] for those — it is a superset that runs the same
    /// lines and additionally returns symbolic (expression) results.
    pub fn eval(&mut self, line: &str) -> Result<CalcResult, ExathError> {
        let line = line.trim();

        if let Some((name, params, body_str)) = split_fn_def(line) {
            let body_ast = crate::ast::parse_str(body_str)?;
            self.fns.insert(name.to_string(), (params, body_ast));
            return Ok(CalcResult::Real(0.0));
        }

        if let Some((lhs, rhs)) = split_assignment(line) {
            let result = super::evaluate_with_vars_and_fns(
                rhs, self.angle_mode, &self.vars, &self.fns,
            )?;
            let cx = match &result {
                CalcResult::Real(value) => Cx::real(*value),
                CalcResult::Complex(re, im) => Cx { re: *re, im: *im },
            };
            self.vars.insert(lhs.to_string(), cx);
            return Ok(result);
        }

        super::evaluate_with_vars_and_fns(line, self.angle_mode, &self.vars, &self.fns)
    }

    /// Like [`Session::eval`], but additionally understands every DSL form —
    /// symbolic (`diff`, `simplify`, `expand`, `factor`, `solve`, `integral`,
    /// `taylor`, `limit`, `laplace`, `dsolve`, …), linear algebra (`det`, `inv`,
    /// `eigenvalues`, …) and numeric range forms (`sum`, `product`, `deriv`) —
    /// returning a [`LineResult::Expression`] for symbolic results and a
    /// [`LineResult::Value`] for numeric ones. A name can be bound to a symbolic
    /// expression and reused. Plain numeric lines behave exactly as in
    /// [`Session::eval`]; user-defined functions and previously-bound symbolic
    /// variables are expanded inside the forms.
    pub fn eval_line(&mut self, line: &str) -> Result<LineResult, ExathError> {
        let line = line.trim();

        // f(x) = body  — define a user function.
        if let Some((name, params, body_str)) = split_fn_def(line) {
            let body_ast = parse_str(body_str)?;
            self.fns.insert(name.to_string(), (params, body_ast));
            return Ok(LineResult::Value(CalcResult::Real(0.0)));
        }

        // ident = rhs  — assignment (numeric or symbolic).
        if let Some((lhs, rhs)) = split_assignment(line) {
            let ast = parse_str(rhs)?;
            if let Some(expr) = self.try_symbolic(&ast)? {
                self.vars.remove(lhs);
                self.sym_vars.insert(lhs.to_string(), expr.clone());
                return Ok(LineResult::Expression(symbolic::render(&expr)));
            }
            let value = self.eval_numeric(&ast)?;
            self.sym_vars.remove(lhs);
            self.vars.insert(lhs.to_string(), cx_of(&value));
            return Ok(LineResult::Value(value));
        }

        // Bare expression.
        let ast = parse_str(line)?;
        if let Some(expr) = self.try_symbolic(&ast)? {
            return Ok(LineResult::Expression(symbolic::render(&expr)));
        }
        if let Some(solution) = self.try_solve(&ast)? {
            return Ok(LineResult::Expression(solution));
        }
        // Eigenvalues: roots of the characteristic polynomial (may be complex).
        if let Ast::Call(name, cargs) = &ast {
            if name == "eigenvalues" && cargs.len() == 1 {
                let m = match crate::matrix::eval_matrix_ast(
                    &cargs[0],
                    &self.vars,
                    &self.fns,
                    self.angle_mode,
                )? {
                    crate::matrix::MValue::Mat(m) => m,
                    crate::matrix::MValue::Scalar(_) => {
                        return Err(ExathError::arg_type("eigenvalues expects a matrix"))
                    }
                };
                // Symmetric → stable Jacobi solver; else characteristic polynomial.
                let mut evs_real: Option<Vec<f64>> = None;
                if m.is_symmetric() {
                    if let Ok((vals, _)) = m.jacobi_eigen() {
                        evs_real = Some(vals);
                    }
                }
                let coeffs = m.char_poly_coeffs()?;
                let fmt = |x: f64| -> String {
                    let r = (x * 1e9).round() / 1e9;
                    if (r - r.round()).abs() < 1e-9 {
                        format!("{}", r.round() as i64)
                    } else {
                        format!("{}", r)
                    }
                };
                let mut evs: Vec<(f64, f64)> = match evs_real {
                    Some(vals) => vals.into_iter().map(|v| (v, 0.0)).collect(),
                    None => symbolic::roots_of(&coeffs),
                };
                evs.sort_by(|a, b| {
                    a.0.partial_cmp(&b.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                });
                let parts: Vec<String> = evs
                    .into_iter()
                    .map(|(re, im)| {
                        if im.abs() < 1e-7 {
                            fmt(re)
                        } else {
                            format!("{} {} {}i", fmt(re), if im < 0.0 { "-" } else { "+" }, fmt(im.abs()))
                        }
                    })
                    .collect();
                return Ok(LineResult::Expression(parts.join(", ")));
            }
        }
        // Sign assumption: assume(x > 0) / assume(x < 0) / >= / <=.
        if let Ast::Call(name, cargs) = &ast {
            if name == "assume" && cargs.len() == 1 {
                use crate::ast::BinOp;
                if let Ast::BinOp(op, l, r) = &cargs[0] {
                    if let (Ast::Var(v), Ast::Number(n)) = (l.as_ref(), r.as_ref()) {
                        if *n == 0.0 {
                            let sign: Option<i8> = match op {
                                BinOp::Gt | BinOp::Ge => Some(1),
                                BinOp::Lt | BinOp::Le => Some(-1),
                                _ => None,
                            };
                            if let Some(s) = sign {
                                self.assumptions.insert(v.clone(), s);
                                return Ok(LineResult::Expression(format!(
                                    "{} {} 0",
                                    v,
                                    if s > 0 { ">" } else { "<" }
                                )));
                            }
                        }
                    }
                }
                return Err(ExathError::parse(
                    "assume expects a sign condition like assume(x > 0)",
                ));
            }
        }
        // Linear constant-coefficient ODE: dsolve([a_n, …, a_0], x).
        if let Ast::Call(name, cargs) = &ast {
            if name == "dsolve" && cargs.len() == 2 {
                let coeffs: Vec<f64> = match &cargs[0] {
                    Ast::Matrix(rows) => {
                        let mut v = Vec::new();
                        for e in rows.iter().flatten() {
                            v.push(self.eval_scalar(e)?);
                        }
                        v
                    }
                    _ => {
                        return Err(ExathError::parse(
                            "dsolve: first argument must be a coefficient list [a_n, …, a_0]",
                        ))
                    }
                };
                let var = var_name(&cargs[1])?;
                return Ok(LineResult::Expression(symbolic::dsolve(&coeffs, &var)?));
            }
        }
        // Characteristic polynomial of a matrix as a symbolic expression in var.
        if let Ast::Call(name, cargs) = &ast {
            if name == "charpoly" && cargs.len() == 2 {
                let m = match crate::matrix::eval_matrix_ast(
                    &cargs[0], &self.vars, &self.fns, self.angle_mode,
                )? {
                    crate::matrix::MValue::Mat(m) => m,
                    crate::matrix::MValue::Scalar(_) => {
                        return Err(ExathError::arg_type("charpoly expects a matrix"))
                    }
                };
                let var = var_name(&cargs[1])?;
                let coeffs = m.char_poly_coeffs()?;
                let mut terms: Vec<String> = Vec::new();
                for (k, c) in coeffs.iter().enumerate() {
                    let cr = (c * 1e9).round() / 1e9;
                    if cr == 0.0 {
                        continue;
                    }
                    let cs = if (cr - cr.round()).abs() < 1e-9 {
                        format!("{}", cr.round() as i64)
                    } else {
                        format!("{}", cr)
                    };
                    terms.push(match k {
                        0 => cs,
                        1 => format!("({})*{}", cs, var),
                        _ => format!("({})*{}^{}", cs, var, k),
                    });
                }
                let expr_str = if terms.is_empty() { "0".to_string() } else { terms.join(" + ") };
                let simplified = symbolic::simplify_ast(crate::ast::parse_str(&expr_str)?);
                return Ok(LineResult::Expression(symbolic::render(&simplified)));
            }
        }
        // Integer factorisation: factorint(n) → "p1^e1 * p2^e2 * ...".
        if let Ast::Call(name, cargs) = &ast {
            if name == "factorint" && cargs.len() == 1 {
                let nf = self.eval_scalar(&cargs[0])?;
                if nf.fract() != 0.0 || nf.abs() < 1.0 {
                    return Err(ExathError::domain("factorint requires a non-zero integer"));
                }
                let mut n = nf.abs() as i64;
                let mut parts: Vec<String> = Vec::new();
                if nf < 0.0 {
                    parts.push("-1".to_string());
                }
                let mut p = 2i64;
                while p * p <= n {
                    if n % p == 0 {
                        let mut e = 0;
                        while n % p == 0 {
                            n /= p;
                            e += 1;
                        }
                        parts.push(if e == 1 { format!("{}", p) } else { format!("{}^{}", p, e) });
                    }
                    p += 1;
                }
                if n > 1 {
                    parts.push(format!("{}", n));
                }
                if parts.is_empty() {
                    parts.push("1".to_string());
                }
                return Ok(LineResult::Expression(parts.join(" * ")));
            }
        }
        // Eigenvectors: null space of (A − λI) for each distinct real eigenvalue.
        if let Ast::Call(name, cargs) = &ast {
            if name == "eigenvectors" && cargs.len() == 1 {
                let m = match crate::matrix::eval_matrix_ast(
                    &cargs[0], &self.vars, &self.fns, self.angle_mode,
                )? {
                    crate::matrix::MValue::Mat(m) => m,
                    crate::matrix::MValue::Scalar(_) => {
                        return Err(ExathError::arg_type("eigenvectors expects a matrix"))
                    }
                };
                let n = m.rows();
                // Symmetric → orthonormal eigenvectors directly from Jacobi.
                if m.is_symmetric() {
                    if let Ok((_, vecs)) = m.jacobi_eigen() {
                        let rounded: Vec<Vec<f64>> = (0..n)
                            .map(|r| {
                                (0..n)
                                    .map(|c| (vecs.get(r, c) * 1e9).round() / 1e9)
                                    .collect()
                            })
                            .collect();
                        let result =
                            crate::matrix::MValue::Mat(crate::matrix::Matrix::new(rounded)?);
                        return Ok(LineResult::Expression(crate::matrix::render_mvalue(&result)));
                    }
                }
                let coeffs = m.char_poly_coeffs()?;
                let mut evs: Vec<f64> = symbolic::roots_of(&coeffs)
                    .into_iter()
                    .filter(|(_, im)| im.abs() < 1e-7)
                    .map(|(re, _)| re)
                    .collect();
                evs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                evs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

                let mut cols: Vec<Vec<f64>> = Vec::new();
                for lambda in evs {
                    let shifted =
                        m.sub(&crate::matrix::Matrix::identity(n).scale(lambda))?;
                    if let Some(v) = shifted.null_space_vector() {
                        cols.push(v);
                    }
                }
                if cols.is_empty() {
                    return Err(ExathError::domain("no real eigenvectors found"));
                }
                let data: Vec<Vec<f64>> = (0..n)
                    .map(|r| cols.iter().map(|c| c[r]).collect())
                    .collect();
                let result = crate::matrix::MValue::Mat(crate::matrix::Matrix::new(data)?);
                return Ok(LineResult::Expression(crate::matrix::render_mvalue(&result)));
            }
        }
        // Matrix expressions (literals like [[1,2],[3,4]], det/inv/transpose/…).
        if crate::matrix::is_matrix_expr(&ast) {
            let v = crate::matrix::eval_matrix_ast(&ast, &self.vars, &self.fns, self.angle_mode)?;
            return match v {
                crate::matrix::MValue::Scalar(s) => {
                    Ok(LineResult::Value(CalcResult::Real(s)))
                }
                crate::matrix::MValue::Mat(_) => {
                    Ok(LineResult::Expression(crate::matrix::render_mvalue(&v)))
                }
            };
        }
        Ok(LineResult::Value(self.eval_numeric(&ast)?))
    }

    /// Handle a top-level `solve(equation, variable)` form, returning the
    /// solution set formatted as `var = r1, var = r2`. `Ok(None)` otherwise.
    fn try_solve(&self, ast: &Ast) -> Result<Option<String>, ExathError> {
        if let Ast::Call(name, args) = ast {
            if name == "solve" {
                if args.len() != 2 {
                    return Err(ExathError::arg_count(
                        "solve requires 2 arguments: solve(equation, variable)",
                    ));
                }
                let var = match &args[1] {
                    Ast::Var(v) => v.clone(),
                    _ => {
                        return Err(ExathError::parse(
                            "solve: the second argument must be a variable name",
                        ))
                    }
                };
                let inner = self.expand(&args[0])?;
                let roots = symbolic::solve_ast(&inner, &var)?;
                let mut seen: Vec<String> = Vec::new();
                for r in roots {
                    let s = symbolic::render(&symbolic::simplify_ast(r));
                    if !seen.contains(&s) {
                        seen.push(s);
                    }
                }
                let formatted = seen
                    .iter()
                    .map(|r| format!("{} = {}", var, r))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Ok(Some(formatted));
            }
        }
        Ok(None)
    }

    /// If `ast` is a top-level `diff`/`simplify` form, return the resulting
    /// symbolic expression with user functions and symbolic variables expanded.
    /// Returns `Ok(None)` for ordinary numeric expressions.
    fn try_symbolic(&self, ast: &Ast) -> Result<Option<Ast>, ExathError> {
        if let Ast::Call(name, args) = ast {
            match name.as_str() {
                "diff" => {
                    if args.len() != 2 {
                        return Err(ExathError::arg_count(
                            "diff requires 2 arguments: diff(expr, variable)",
                        ));
                    }
                    let var = match &args[1] {
                        Ast::Var(v) => v.clone(),
                        _ => {
                            return Err(ExathError::parse(
                                "diff: the second argument must be a variable name",
                            ))
                        }
                    };
                    let inner = self.expand(&args[0])?;
                    return Ok(Some(symbolic::differentiate_ast(&inner, &var)?));
                }
                "simplify" => {
                    if args.len() != 1 {
                        return Err(ExathError::arg_count(
                            "simplify requires 1 argument: simplify(expr)",
                        ));
                    }
                    let inner = self.expand(&args[0])?;
                    let simplified = symbolic::simplify_ast(inner);
                    return Ok(Some(apply_assumptions(&simplified, &self.assumptions)));
                }
                "expand" => {
                    if args.len() != 1 {
                        return Err(ExathError::arg_count(
                            "expand requires 1 argument: expand(expr)",
                        ));
                    }
                    let inner = self.expand(&args[0])?;
                    return Ok(Some(symbolic::expand_tree(&inner)));
                }
                "factor" => {
                    if args.len() != 2 {
                        return Err(ExathError::arg_count(
                            "factor requires 2 arguments: factor(expr, variable)",
                        ));
                    }
                    let var = match &args[1] {
                        Ast::Var(v) => v.clone(),
                        _ => {
                            return Err(ExathError::parse(
                                "factor: the second argument must be a variable name",
                            ))
                        }
                    };
                    let inner = self.expand(&args[0])?;
                    return Ok(Some(symbolic::factor_tree(&inner, &var)?));
                }
                "grad" => {
                    if args.len() != 2 {
                        return Err(ExathError::arg_count(
                            "grad requires 2 arguments: grad(f, [x, y, ...])",
                        ));
                    }
                    let vars = matrix_var_names(&args[1])?;
                    let f = self.expand(&args[0])?;
                    let mut rows = Vec::new();
                    for v in &vars {
                        rows.push(vec![symbolic::differentiate_ast(&f, v)?]);
                    }
                    return Ok(Some(Ast::Matrix(rows)));
                }
                "hessian" => {
                    if args.len() != 2 {
                        return Err(ExathError::arg_count(
                            "hessian requires 2 arguments: hessian(f, [x, y, ...])",
                        ));
                    }
                    let vars = matrix_var_names(&args[1])?;
                    let f = self.expand(&args[0])?;
                    let mut rows = Vec::new();
                    for vi in &vars {
                        let di = symbolic::differentiate_ast(&f, vi)?;
                        let mut row = Vec::new();
                        for vj in &vars {
                            row.push(symbolic::differentiate_ast(&di, vj)?);
                        }
                        rows.push(row);
                    }
                    return Ok(Some(Ast::Matrix(rows)));
                }
                "jacobian" => {
                    if args.len() != 2 {
                        return Err(ExathError::arg_count(
                            "jacobian requires 2 arguments: jacobian([f, g, ...], [x, y, ...])",
                        ));
                    }
                    let funcs = match &args[0] {
                        Ast::Matrix(r) => r.iter().flatten().cloned().collect::<Vec<_>>(),
                        _ => return Err(ExathError::parse("jacobian: first argument must be [f, g, ...]")),
                    };
                    let vars = matrix_var_names(&args[1])?;
                    let mut rows = Vec::new();
                    for f in &funcs {
                        let fe = self.expand(f)?;
                        let mut row = Vec::new();
                        for v in &vars {
                            row.push(symbolic::differentiate_ast(&fe, v)?);
                        }
                        rows.push(row);
                    }
                    return Ok(Some(Ast::Matrix(rows)));
                }
                "laplace" => {
                    if args.len() != 3 {
                        return Err(ExathError::arg_count(
                            "laplace requires 3 arguments: laplace(f, t, s)",
                        ));
                    }
                    let t = var_name(&args[1])?;
                    let s_ = var_name(&args[2])?;
                    let inner = self.expand(&args[0])?;
                    return Ok(Some(symbolic::laplace_ast(&inner, &t, &s_)?));
                }
                "sumc" => {
                    if args.len() != 3 {
                        return Err(ExathError::arg_count(
                            "sumc requires 3 arguments: sumc(expr, k, n) for sum_{k=1}^n",
                        ));
                    }
                    let k = var_name(&args[1])?;
                    let n = var_name(&args[2])?;
                    let inner = self.expand(&args[0])?;
                    return Ok(Some(symbolic::sum_closed_ast(&inner, &k, &n)?));
                }
                "polygcd" => {
                    if args.len() != 3 {
                        return Err(ExathError::arg_count(
                            "polygcd requires 3 arguments: polygcd(p, q, variable)",
                        ));
                    }
                    let var = match &args[2] {
                        Ast::Var(v) => v.clone(),
                        _ => return Err(ExathError::parse("polygcd: third argument must be a variable")),
                    };
                    let p = self.expand(&args[0])?;
                    let q = self.expand(&args[1])?;
                    return Ok(Some(symbolic::poly_gcd_ast(&p, &q, &var)?));
                }
                "nsolve" => {
                    if args.len() != 3 {
                        return Err(ExathError::arg_count(
                            "nsolve requires 3 arguments: nsolve(f, variable, guess)",
                        ));
                    }
                    let var = match &args[1] {
                        Ast::Var(v) => v.clone(),
                        _ => return Err(ExathError::parse("nsolve: second argument must be a variable")),
                    };
                    let guess = self.eval_scalar(&args[2])?;
                    let inner = self.expand(&args[0])?;
                    let root = symbolic::newton(&inner, &var, guess)?;
                    return Ok(Some(crate::ast::parse_str(&format!("{}", root))?));
                }
                "odesolve" => {
                    // odesolve(f, x, y, x0, y0, x1): solve y' = f(x,y), return y(x1) via RK4.
                    if args.len() != 6 {
                        return Err(ExathError::arg_count(
                            "odesolve requires 6 arguments: odesolve(f, x, y, x0, y0, x1)",
                        ));
                    }
                    let xv = var_name(&args[1])?;
                    let yv = var_name(&args[2])?;
                    let x0 = self.eval_scalar(&args[3])?;
                    let y0 = self.eval_scalar(&args[4])?;
                    let x1 = self.eval_scalar(&args[5])?;
                    let f = self.expand(&args[0])?;
                    let eval_f = |x: f64, y: f64| -> Result<f64, ExathError> {
                        let mut m = self.vars.clone();
                        m.insert(xv.clone(), Cx::real(x));
                        m.insert(yv.clone(), Cx::real(y));
                        Ok(eval_ast(&f, &m, &self.fns, self.angle_mode)?.re)
                    };
                    let n = 2000;
                    let h = (x1 - x0) / n as f64;
                    let (mut x, mut y) = (x0, y0);
                    for _ in 0..n {
                        let k1 = eval_f(x, y)?;
                        let k2 = eval_f(x + h / 2.0, y + h / 2.0 * k1)?;
                        let k3 = eval_f(x + h / 2.0, y + h / 2.0 * k2)?;
                        let k4 = eval_f(x + h, y + h * k3)?;
                        y += h / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
                        x += h;
                    }
                    return Ok(Some(crate::ast::parse_str(&format!("{}", y))?));
                }
                "minimize" | "maximize" => {
                    // minimize(f, x, a, b): golden-section search; returns argmin/argmax x.
                    if args.len() != 4 {
                        return Err(ExathError::arg_count(
                            "minimize requires 4 arguments: minimize(f, x, a, b)",
                        ));
                    }
                    let v = var_name(&args[1])?;
                    let mut a = self.eval_scalar(&args[2])?;
                    let mut b = self.eval_scalar(&args[3])?;
                    let f = self.expand(&args[0])?;
                    let sign = if name == "maximize" { -1.0 } else { 1.0 };
                    let fx = |x: f64| -> Result<f64, ExathError> {
                        let mut m = self.vars.clone();
                        m.insert(v.clone(), Cx::real(x));
                        Ok(sign * eval_ast(&f, &m, &self.fns, self.angle_mode)?.re)
                    };
                    let gr = (5.0_f64.sqrt() - 1.0) / 2.0;
                    let mut c = b - gr * (b - a);
                    let mut d = a + gr * (b - a);
                    for _ in 0..200 {
                        if fx(c)? < fx(d)? {
                            b = d;
                        } else {
                            a = c;
                        }
                        c = b - gr * (b - a);
                        d = a + gr * (b - a);
                        if (b - a).abs() < 1e-10 {
                            break;
                        }
                    }
                    let xm = (a + b) / 2.0;
                    return Ok(Some(symbolic::simplify_ast(crate::ast::parse_str(
                        &format!("{}", (xm * 1e9).round() / 1e9),
                    )?)));
                }
                "integral" => {
                    if args.len() != 2 && args.len() != 4 {
                        return Err(ExathError::arg_count(
                            "integral requires 2 args integral(expr, var) or 4 for a definite \
                             integral integral(expr, var, a, b)",
                        ));
                    }
                    let var = match &args[1] {
                        Ast::Var(v) => v.clone(),
                        _ => {
                            return Err(ExathError::parse(
                                "integral: the second argument must be a variable name",
                            ))
                        }
                    };
                    let inner = self.expand(&args[0])?;
                    if args.len() == 4 {
                        let a = self.eval_scalar(&args[2])?;
                        let b = self.eval_scalar(&args[3])?;
                        return Ok(Some(symbolic::integrate_definite_ast(&inner, &var, a, b)?));
                    }
                    return Ok(Some(symbolic::integrate_ast(&inner, &var)?));
                }
                "taylor" => {
                    if args.len() != 4 {
                        return Err(ExathError::arg_count(
                            "taylor requires 4 arguments: taylor(expr, variable, x0, order)",
                        ));
                    }
                    let var = match &args[1] {
                        Ast::Var(v) => v.clone(),
                        _ => {
                            return Err(ExathError::parse(
                                "taylor: the second argument must be a variable name",
                            ))
                        }
                    };
                    let x0 = self.eval_scalar(&args[2])?;
                    let order_f = self.eval_scalar(&args[3])?;
                    if order_f < 0.0 || order_f.fract() != 0.0 {
                        return Err(ExathError::domain("taylor: order must be a non-negative integer"));
                    }
                    let inner = self.expand(&args[0])?;
                    return Ok(Some(symbolic::taylor_ast(&inner, &var, x0, order_f as usize)?));
                }
                "limit" => {
                    if args.len() != 3 {
                        return Err(ExathError::arg_count(
                            "limit requires 3 arguments: limit(expr, variable, x0)",
                        ));
                    }
                    let var = match &args[1] {
                        Ast::Var(v) => v.clone(),
                        _ => {
                            return Err(ExathError::parse(
                                "limit: the second argument must be a variable name",
                            ))
                        }
                    };
                    // Accept `inf` / `infinity` (optionally negated) as the point.
                    let x0 = match &args[2] {
                        Ast::Var(n) if matches!(n.as_str(), "inf" | "infinity" | "oo") => {
                            f64::INFINITY
                        }
                        Ast::UnaryNeg(inner) => match inner.as_ref() {
                            Ast::Var(n) if matches!(n.as_str(), "inf" | "infinity" | "oo") => {
                                f64::NEG_INFINITY
                            }
                            _ => self.eval_scalar(&args[2])?,
                        },
                        _ => self.eval_scalar(&args[2])?,
                    };
                    let inner = self.expand(&args[0])?;
                    let v = symbolic::limit_value(&inner, &var, x0)?;
                    return Ok(Some(symbolic::simplify_ast(crate::ast::parse_str(&format!("{}", v))?)));
                }
                _ => {}
            }
        }
        Ok(None)
    }

    /// Evaluate `ast` to a real scalar using the current variables.
    fn eval_scalar(&self, ast: &Ast) -> Result<f64, ExathError> {
        let prepared = self.substitute_sym_vars(ast.clone());
        Ok(eval_ast(&prepared, &self.vars, &self.fns, self.angle_mode)?.to_calc_result().to_f64_lossy())
    }

    /// Expand user-defined functions and symbolic variables (for symbolic use).
    fn expand(&self, ast: &Ast) -> Result<Ast, ExathError> {
        let inlined = symbolic::inline_user_fns(ast, &self.fns)?;
        Ok(self.substitute_sym_vars(inlined))
    }

    /// Evaluate numerically, first substituting any symbolic variables in.
    fn eval_numeric(&self, ast: &Ast) -> Result<CalcResult, ExathError> {
        let prepared = self.substitute_sym_vars(ast.clone());
        Ok(eval_ast(&prepared, &self.vars, &self.fns, self.angle_mode)?.to_calc_result())
    }

    /// Substitute symbolic variables into `ast`. Repeated passes resolve chains
    /// (`a = …; b = a + 1`); bounded so cyclic definitions cannot loop forever.
    fn substitute_sym_vars(&self, mut ast: Ast) -> Ast {
        if self.sym_vars.is_empty() {
            return ast;
        }
        for _ in 0..self.sym_vars.len() + 1 {
            for (name, expr) in &self.sym_vars {
                ast = symbolic::substitute(&ast, name, expr);
            }
        }
        ast
    }

    /// List all symbolic variable names.
    pub fn sym_var_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.sym_vars.keys().cloned().collect();
        names.sort();
        names
    }

    /// Read a variable value by name.
    pub fn get_var(&self, name: &str) -> Option<CalcResult> {
        self.vars.get(name).map(|cx| cx.to_calc_result())
    }

    /// Set a variable manually (e.g. from C/WASM host).
    pub fn set_var(&mut self, name: &str, re: f64, im: f64) {
        self.vars.insert(name.to_string(), Cx { re, im });
    }

    /// Remove a variable.
    pub fn remove_var(&mut self, name: &str) {
        self.vars.remove(name);
    }

    /// Clear all variables (numeric and symbolic).
    pub fn clear_vars(&mut self) {
        self.vars.clear();
        self.sym_vars.clear();
    }

    /// List all variable names.
    pub fn var_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.vars.keys().cloned().collect();
        names.sort();
        names
    }

    /// List all user-defined function names.
    pub fn fn_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.fns.keys().cloned().collect();
        names.sort();
        names
    }

    /// Remove a user-defined function.
    pub fn remove_fn(&mut self, name: &str) {
        self.fns.remove(name);
    }
}

/// Apply sign assumptions to canonical forms: `sqrt(v^2) → v` / `-v`,
/// `abs(v) → v` / `-v` when the sign of `v` is known. Additive — does not
/// touch the core simplifier.
fn apply_assumptions(ast: &Ast, assume: &HashMap<String, i8>) -> Ast {
    if assume.is_empty() {
        return ast.clone();
    }
    let signed = |v: &str| assume.get(v).copied();
    match ast {
        Ast::Call(name, args) if name == "abs" && args.len() == 1 => {
            if let Ast::Var(v) = &args[0] {
                match signed(v) {
                    Some(1) => return Ast::Var(v.clone()),
                    Some(-1) => return Ast::UnaryNeg(Box::new(Ast::Var(v.clone()))),
                    _ => {}
                }
            }
            Ast::Call(name.clone(), args.iter().map(|a| apply_assumptions(a, assume)).collect())
        }
        // sqrt(v^2) → v (v≥0) or −v (v≤0)
        Ast::Call(name, args) if name == "sqrt" && args.len() == 1 => {
            if let Ast::BinOp(crate::ast::BinOp::Pow, base, exp) = &args[0] {
                if let (Ast::Var(v), Ast::Number(n)) = (base.as_ref(), exp.as_ref()) {
                    if (*n - 2.0).abs() < 1e-12 {
                        match signed(v) {
                            Some(1) => return Ast::Var(v.clone()),
                            Some(-1) => return Ast::UnaryNeg(Box::new(Ast::Var(v.clone()))),
                            _ => {}
                        }
                    }
                }
            }
            Ast::Call(name.clone(), args.iter().map(|a| apply_assumptions(a, assume)).collect())
        }
        Ast::BinOp(op, l, r) => Ast::BinOp(
            op.clone(),
            Box::new(apply_assumptions(l, assume)),
            Box::new(apply_assumptions(r, assume)),
        ),
        Ast::UnaryNeg(u) => Ast::UnaryNeg(Box::new(apply_assumptions(u, assume))),
        Ast::Call(name, args) => {
            Ast::Call(name.clone(), args.iter().map(|a| apply_assumptions(a, assume)).collect())
        }
        other => other.clone(),
    }
}

/// Extract a single variable name from an `Ast::Var`.
fn var_name(ast: &Ast) -> Result<String, ExathError> {
    match ast {
        Ast::Var(v) => Ok(v.clone()),
        _ => Err(ExathError::parse("expected a variable name")),
    }
}

/// Extract variable names from a matrix/vector literal like `[x, y, z]`.
fn matrix_var_names(ast: &Ast) -> Result<Vec<String>, ExathError> {
    match ast {
        Ast::Matrix(rows) => {
            let mut names = Vec::new();
            for e in rows.iter().flatten() {
                match e {
                    Ast::Var(v) => names.push(v.clone()),
                    _ => {
                        return Err(ExathError::parse(
                            "expected a list of variables like [x, y]",
                        ))
                    }
                }
            }
            if names.is_empty() {
                return Err(ExathError::parse("variable list is empty"));
            }
            Ok(names)
        }
        _ => Err(ExathError::parse("expected a variable list like [x, y]")),
    }
}

/// Convert a [`CalcResult`] to a [`Cx`] for storage as a numeric variable.
fn cx_of(result: &CalcResult) -> Cx {
    match result {
        CalcResult::Real(v) => Cx::real(*v),
        CalcResult::Complex(re, im) => Cx { re: *re, im: *im },
    }
}

/// Detect `ident(params) = body` and split into (name, [param, ...], body_str).
fn split_fn_def(line: &str) -> Option<(&str, Vec<String>, &str)> {
    let lparen = line.find('(')?;
    let name = line[..lparen].trim();

    if name.is_empty()
        || !name.chars().next()?.is_ascii_alphabetic()
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }

    let rparen = line[lparen..].find(')')? + lparen;

    let after_paren = line[rparen + 1..].trim_start();
    if !after_paren.starts_with('=') {
        return None;
    }
    let after_eq = after_paren[1..].trim_start();
    if after_eq.starts_with('=') {
        return None;
    }

    let params_str = line[lparen + 1..rparen].trim();
    let params: Vec<String> = if params_str.is_empty() {
        vec![]
    } else {
        params_str
            .split(',')
            .map(|p| p.trim().to_string())
            .collect()
    };

    for param in &params {
        if param.is_empty()
            || !param.chars().next()?.is_ascii_alphabetic()
            || !param.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return None;
        }
    }

    Some((name, params, after_eq))
}

/// Detect `identifier = expression` and split into (lhs, rhs).
fn split_assignment(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'=' {
            let prev = if i > 0 { bytes[i - 1] } else { 0 };
            let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
            if prev != b'!' && prev != b'<' && prev != b'>' && next != b'=' {
                let lhs = line[..i].trim();
                let rhs = line[i + 1..].trim();
                if !lhs.is_empty()
                    && lhs.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && lhs.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
                {
                    return Some((lhs, rhs));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod eval_line_tests {
    use super::*;

    fn expr(s: &mut Session, line: &str) -> String {
        match s.eval_line(line) {
            Ok(LineResult::Expression(e)) => e,
            Ok(LineResult::Value(v)) => {
                assert!(false, "expected expression, got value {:?}", v);
                String::new()
            }
            Err(e) => {
                assert!(false, "eval_line('{}') failed: {}", line, e);
                String::new()
            }
        }
    }

    fn value(s: &mut Session, line: &str) -> f64 {
        match s.eval_line(line) {
            Ok(LineResult::Value(CalcResult::Real(v))) => v,
            other => {
                assert!(false, "expected real value, got {:?}", other);
                f64::NAN
            }
        }
    }

    #[test]
    fn diff_in_dsl() {
        let mut s = Session::new(AngleMode::Rad);
        assert_eq!(expr(&mut s, "diff(x^2, x)"), "2 * x");
        // canonical form: like terms collected, highest degree first
        assert_eq!(expr(&mut s, "simplify(2*3 + x*1)"), "x + 6");
        assert_eq!(expr(&mut s, "simplify(x + x)"), "2 * x");
        assert_eq!(expr(&mut s, "simplify(2*x + 3*x)"), "5 * x");
        assert_eq!(expr(&mut s, "simplify(x*x)"), "x^2");
        assert_eq!(expr(&mut s, "simplify((x + 1)^2)"), "x^2 + 2 * x + 1");
    }

    #[test]
    fn diff_of_user_function() {
        let mut s = Session::new(AngleMode::Rad);
        // f(x) = x^2 + 1  ->  d/dx = 2x
        assert_eq!(s.eval_line("f(x) = x^2 + 1").is_ok(), true);
        assert_eq!(expr(&mut s, "diff(f(x), x)"), "2 * x");
    }

    #[test]
    fn symbolic_variable_then_numeric() {
        let mut s = Session::new(AngleMode::Rad);
        // bind a symbolic variable, then evaluate it numerically
        assert_eq!(expr(&mut s, "g = diff(x^3, x)"), "3 * x^2");
        assert!(s.sym_var_names().contains(&"g".to_string()));
        let _ = s.eval_line("x = 2");
        assert!((value(&mut s, "g") - 12.0).abs() < 1e-9); // 3*2^2
        // and it can be simplified/used again
        assert_eq!(expr(&mut s, "simplify(g)"), "3 * x^2");
    }

    #[test]
    fn numeric_lines_unchanged() {
        let mut s = Session::new(AngleMode::Rad);
        assert!((value(&mut s, "2 + 3 * 4") - 14.0).abs() < 1e-9);
        let _ = s.eval_line("r = 5");
        assert!((value(&mut s, "r * 2") - 10.0).abs() < 1e-9);
    }

    #[test]
    fn errors_do_not_panic() {
        let mut s = Session::new(AngleMode::Rad);
        assert!(s.eval_line("diff(x^2)").is_err()); // wrong arg count
        assert!(s.eval_line("diff(x^2, 3)").is_err()); // var not an identifier
        assert!(s.eval_line("diff(x!, x)").is_err()); // unsupported construct
    }

    #[test]
    fn assumptions_in_simplify() {
        let mut s = Session::new(AngleMode::Rad);
        // without an assumption, sqrt(x^2) is not reduced to x
        let bare = expr(&mut s, "simplify(sqrt(x^2))");
        assert!(bare != "x", "should not reduce without assumption, got {}", bare);
        // assume x > 0 → sqrt(x^2) = x, abs(x) = x
        let _ = s.eval_line("assume(x > 0)");
        assert_eq!(expr(&mut s, "simplify(sqrt(x^2))"), "x");
        assert_eq!(expr(&mut s, "simplify(abs(x))"), "x");
        // assume x < 0 → abs(x) = -x
        let mut s2 = Session::new(AngleMode::Rad);
        let _ = s2.eval_line("assume(x < 0)");
        assert_eq!(expr(&mut s2, "simplify(abs(x))"), "-x");
    }

    #[test]
    fn piecewise_eval_and_diff() {
        // piecewise(x < 0, -x, x) = |x|
        let mut s = Session::new(AngleMode::Rad);
        s.set_var("x", -3.0, 0.0);
        assert!((value(&mut s, "piecewise(x < 0, -x, x)") - 3.0).abs() < 1e-9);
        s.set_var("x", 4.0, 0.0);
        assert!((value(&mut s, "piecewise(x < 0, -x, x)") - 4.0).abs() < 1e-9);
        // branch-wise derivative: d/dx if(x>0, x^2, x^3) = if(x>0, 2x, 3x^2)
        let mut s2 = Session::new(AngleMode::Rad);
        assert_eq!(
            expr(&mut s2, "diff(if(x > 0, x^2, x^3), x)"),
            "if(x > 0, 2 * x, 3 * x^2)"
        );
    }

    #[test]
    fn number_theory() {
        let mut s = Session::new(AngleMode::Rad);
        assert!((value(&mut s, "isprime(97)") - 1.0).abs() < 1e-9);
        assert!((value(&mut s, "isprime(91)") - 0.0).abs() < 1e-9); // 91 = 7*13
        assert!((value(&mut s, "nextprime(100)") - 101.0).abs() < 1e-9);
        assert!((value(&mut s, "totient(12)") - 4.0).abs() < 1e-9);
        assert!((value(&mut s, "powmod(2, 10, 1000)") - 24.0).abs() < 1e-9); // 1024 mod 1000
        assert_eq!(expr(&mut s, "factorint(360)"), "2^3 * 3^2 * 5");
    }

    #[test]
    fn matrix_literals_in_dsl() {
        let mut s = Session::new(AngleMode::Rad);
        assert_eq!(
            expr(&mut s, "[[1,2],[3,4]] * [[5,6],[7,8]]"),
            "[[19, 22], [43, 50]]"
        );
        assert_eq!(expr(&mut s, "[[1,2],[3,4]] + [[1,1],[1,1]]"), "[[2, 3], [4, 5]]");
        assert_eq!(expr(&mut s, "transpose([[1,2],[3,4]])"), "[[1, 3], [2, 4]]");
        assert!((value(&mut s, "det([[4,7],[2,6]])") - 10.0).abs() < 1e-9);
        // scalar arithmetic still works unchanged alongside matrices
        assert!((value(&mut s, "2 + 3 * 4") - 14.0).abs() < 1e-9);
    }

    #[test]
    fn ode_and_optimization() {
        let mut s = Session::new(AngleMode::Rad);
        let num = |s: &mut Session, line: &str| -> f64 { expr(s, line).parse::<f64>().unwrap_or(f64::NAN) };
        // y' = y, y(0)=1 → y(1) = e ≈ 2.71828
        assert!((num(&mut s, "odesolve(y, x, y, 0, 1, 1)") - std::f64::consts::E).abs() < 1e-5);
        // y' = x, y(0)=0 → y(2) = 2
        assert!((num(&mut s, "odesolve(x, x, y, 0, 0, 2)") - 2.0).abs() < 1e-6);
        // minimize (x-3)^2 on [0,10] → x = 3
        assert!((num(&mut s, "minimize((x - 3)^2, x, 0, 10)") - 3.0).abs() < 1e-6);
        // maximize 4 - (x-2)^2 on [0,5] → x = 2
        assert!((num(&mut s, "maximize(4 - (x - 2)^2, x, 0, 5)") - 2.0).abs() < 1e-6);
    }

    #[test]
    fn multivariate_calculus() {
        let mut s = Session::new(AngleMode::Rad);
        // grad(x^2 + y^2) = [2x, 2y] (as a column)
        assert_eq!(expr(&mut s, "grad(x^2 + y^2, [x, y])"), "[[2 * x], [2 * y]]");
        // hessian(x^2 + x*y) = [[2,1],[1,0]]? d2/dx2=2, dxdy=1, dydx=1, dy2=0
        assert_eq!(expr(&mut s, "hessian(x^2 + x*y, [x, y])"), "[[2, 1], [1, 0]]");
        // jacobian([x*y, x+y], [x,y]) = [[y, x], [1, 1]]
        assert_eq!(expr(&mut s, "jacobian([x*y, x + y], [x, y])"), "[[y, x], [1, 1]]");
    }

    #[test]
    fn eigenvalues_and_linsolve() {
        let mut s = Session::new(AngleMode::Rad);
        // diagonal matrix → eigenvalues are the diagonal entries
        assert_eq!(expr(&mut s, "eigenvalues([[2,0],[0,3]])"), "2, 3");
        // symmetric [[2,1],[1,2]] → eigenvalues 1 and 3
        assert_eq!(expr(&mut s, "eigenvalues([[2,1],[1,2]])"), "1, 3");
        // 2x+y=5, x+3y=10 → x=1, y=3
        assert_eq!(expr(&mut s, "linsolve([[2,1],[1,3]], [5,10])"), "[[1], [3]]");
        // rank
        assert!((value(&mut s, "rank([[1,2],[2,4]])") - 1.0).abs() < 1e-9);
        assert!((value(&mut s, "rank([[1,2],[3,4]])") - 2.0).abs() < 1e-9);
        // characteristic polynomial: det(xI - [[2,1],[1,2]]) = x^2 - 4x + 3
        assert_eq!(expr(&mut s, "charpoly([[2,1],[1,2]], x)"), "x^2 - 4 * x + 3");
        // symmetric → orthonormal eigenvectors (columns), λ=1 and λ=3
        assert_eq!(
            expr(&mut s, "eigenvectors([[2,1],[1,2]])"),
            "[[0.707106781, 0.707106781], [-0.707106781, 0.707106781]]"
        );
    }

    #[test]
    fn comma_is_a_separator_v2() {
        // Exath 2.0: comma is purely a separator; decimals use `.`.
        let mut s = Session::new(AngleMode::Rad);
        assert!((value(&mut s, "1.5 + 1") - 2.5).abs() < 1e-9);
        // `max(1,2)` now means two arguments → 2 (no more decimal-comma merge)
        assert!((value(&mut s, "max(1,2)") - 2.0).abs() < 1e-9);
        assert!((value(&mut s, "mean(1,2,3)") - 2.0).abs() < 1e-9);
        // matrix elements separated by comma
        assert_eq!(expr(&mut s, "[1,2,3]"), "[[1, 2, 3]]");
    }

    #[test]
    fn legacy_eval_still_works() {
        let mut s = Session::new(AngleMode::Rad);
        match s.eval("2 + 3") {
            Ok(CalcResult::Real(v)) => assert!((v - 5.0).abs() < 1e-9),
            other => assert!(false, "{:?}", other),
        }
    }
}
