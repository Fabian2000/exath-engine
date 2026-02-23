use crate::angle_mode::AngleMode;
use crate::ast::UserFns;
use crate::error::ExathError;
use super::calc_result::CalcResult;
use super::cx::Cx;
use std::collections::HashMap;

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
}

impl Session {
    pub fn new(angle_mode: AngleMode) -> Self {
        Session {
            angle_mode,
            vars: HashMap::new(),
            fns: UserFns::new(),
        }
    }

    /// Evaluate one line. Handles three forms:
    /// - `f(x, y) = expr` — defines a user function (stored, returns 0)
    /// - `ident = expr`   — assigns a variable, returns its value
    /// - `expr`           — evaluates the expression, returns its value
    pub fn eval(&mut self, line: &str) -> Result<CalcResult, ExathError> {
        let line = line.trim();

        if let Some((name, params, body_str)) = split_fn_def(line) {
            let body_ast = crate::ast::parse_str(body_str)?;
            self.fns.insert(name.to_lowercase(), (params, body_ast));
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

    /// Clear all variables.
    pub fn clear_vars(&mut self) {
        self.vars.clear();
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
        self.fns.remove(&name.to_lowercase());
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
