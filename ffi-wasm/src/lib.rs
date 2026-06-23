//! WebAssembly bindings for the exath-engine expression evaluator.
//!
//! Everything is reached through one gateway: `evaluate(expr)` (one-shot) or an
//! `ExathSession` via `.eval` / `.evalLine`. `evalLine` understands every form,
//! numeric, symbolic and matrix, returning an `ExathLine` (value or expression).
//! This surface mirrors the Rust crate and the C-FFI.

use exath_engine::{
    AngleMode, CalcResult, Session, LineResult,
    evaluate_complex, is_valid, supported_functions,
};
use wasm_bindgen::prelude::*;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_angle_mode(input: &str) -> AngleMode {
    match input.to_lowercase().as_str() {
        "deg"  => AngleMode::Deg,
        "grad" => AngleMode::Grad,
        _      => AngleMode::Rad,
    }
}

// ── ExathResult ───────────────────────────────────────────────────────────────

/// Result object returned to JavaScript.
#[wasm_bindgen]
pub struct ExathResult {
    re: f64,
    im: f64,
    is_complex: bool,
    error: Option<String>,
}

#[wasm_bindgen]
impl ExathResult {
    #[wasm_bindgen(getter)]
    pub fn re(&self) -> f64 {
        self.re
    }

    #[wasm_bindgen(getter)]
    pub fn im(&self) -> f64 {
        self.im
    }

    #[wasm_bindgen(getter, js_name = isComplex)]
    pub fn is_complex(&self) -> bool {
        self.is_complex
    }

    #[wasm_bindgen(getter, js_name = isError)]
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    #[wasm_bindgen(getter, js_name = errorMessage)]
    pub fn error_message(&self) -> Option<String> {
        self.error.clone()
    }
}

fn calc_to_result(result: Result<CalcResult, exath_engine::ExathError>) -> ExathResult {
    match result {
        Ok(CalcResult::Real(re)) => ExathResult {
            re,
            im: 0.0,
            is_complex: false,
            error: None,
        },
        Ok(CalcResult::Complex(re, im)) => ExathResult {
            re,
            im,
            is_complex: true,
            error: None,
        },
        Err(err) => ExathResult {
            re: 0.0,
            im: 0.0,
            is_complex: false,
            error: Some(err.to_string()),
        },
    }
}

// ── Stateless evaluate ────────────────────────────────────────────────────────

/// Evaluate an expression string.
///
/// - expr: expression string, e.g. `"sqrt(-4) + 2*pi"`
/// - angle_mode: `"deg"`, `"rad"`, or `"grad"` (case-insensitive, defaults to `"rad"`)
///
/// Returns an ExathResult with `.re`, `.im`, `.isComplex`, `.isError`, `.errorMessage`.
#[wasm_bindgen]
pub fn evaluate(expr: &str, angle_mode: &str) -> ExathResult {
    calc_to_result(evaluate_complex(expr, parse_angle_mode(angle_mode)))
}

// ── Validation ────────────────────────────────────────────────────────────────

/// Returns true if the expression parses without error.
#[wasm_bindgen(js_name = isValid)]
pub fn js_is_valid(expr: &str) -> bool {
    is_valid(expr)
}

// ── Supported functions ───────────────────────────────────────────────────────

/// Returns an array of supported function names.
#[wasm_bindgen(js_name = supportedFunctions)]
pub fn js_supported_functions() -> Vec<JsValue> {
    supported_functions()
        .iter()
        .map(|name| JsValue::from_str(name))
        .collect()
}

// ── Session ───────────────────────────────────────────────────────────────────

/// A stateful session that persists variables between eval calls.
///
/// ```js
/// const s = new ExathSession("rad");
/// s.eval("a = 5");
/// s.eval("b = sqrt(a)");
/// const r = s.eval("a + b");   // r.re === 7.2360...
/// ```
#[wasm_bindgen]
pub struct ExathSession {
    inner: Session,
}

#[wasm_bindgen]
impl ExathSession {
    #[wasm_bindgen(constructor)]
    pub fn new(angle_mode: &str) -> ExathSession {
        ExathSession {
            inner: Session::new(parse_angle_mode(angle_mode)),
        }
    }

    /// Evaluate one line (may be `var = expr` or a plain expression).
    pub fn eval(&mut self, line: &str) -> ExathResult {
        calc_to_result(self.inner.eval(line))
    }

    /// Evaluate one line, also understanding every DSL form, symbolic (diff,
    /// simplify, factor, solve, integral, …), linear algebra (det, inv,
    /// eigenvalues, …) and numeric forms (sum, product, deriv). Returns an
    /// `ExathLine`: `.isExpression` is true for symbolic results (read
    /// `.expression`), otherwise read `.re`/`.im`.
    #[wasm_bindgen(js_name = evalLine)]
    pub fn eval_line(&mut self, line: &str) -> ExathLine {
        match self.inner.eval_line(line) {
            Ok(LineResult::Value(CalcResult::Real(re))) => ExathLine::from_value(re, 0.0),
            Ok(LineResult::Value(CalcResult::Complex(re, im))) => ExathLine::from_value(re, im),
            Ok(LineResult::Expression(s)) => ExathLine::from_expression(s),
            Err(e) => ExathLine::from_error(e.to_string()),
        }
    }

    /// Set a variable (im = 0.0 for real values).
    #[wasm_bindgen(js_name = setVar)]
    pub fn set_var(&mut self, name: &str, re: f64, im: f64) {
        self.inner.set_var(name, re, im);
    }

    /// Remove a variable.
    #[wasm_bindgen(js_name = removeVar)]
    pub fn remove_var(&mut self, name: &str) {
        self.inner.remove_var(name);
    }

    /// Clear all variables.
    #[wasm_bindgen(js_name = clearVars)]
    pub fn clear_vars(&mut self) {
        self.inner.clear_vars();
    }

    /// List all variable names as a JS Array of strings.
    #[wasm_bindgen(js_name = varNames)]
    pub fn var_names(&self) -> Vec<JsValue> {
        self.inner
            .var_names()
            .into_iter()
            .map(|name| JsValue::from_str(&name))
            .collect()
    }

    /// List all user-defined function names as a JS Array of strings.
    #[wasm_bindgen(js_name = fnNames)]
    pub fn fn_names(&self) -> Vec<JsValue> {
        self.inner
            .fn_names()
            .into_iter()
            .map(|name| JsValue::from_str(&name))
            .collect()
    }

    /// Remove a user-defined function.
    #[wasm_bindgen(js_name = removeFn)]
    pub fn remove_fn(&mut self, name: &str) {
        self.inner.remove_fn(name);
    }
}

// ── ExathLine (result of ExathSession.evalLine) ───────────────────────────────

/// Result of `ExathSession.evalLine`: either a numeric value or a symbolic
/// expression string (for symbolic forms like `diff` / `factor` / `solve`).
#[wasm_bindgen]
pub struct ExathLine {
    is_expression: bool,
    expression: String,
    re: f64,
    im: f64,
    is_error: bool,
    error_message: Option<String>,
}

impl ExathLine {
    fn from_value(re: f64, im: f64) -> ExathLine {
        ExathLine { is_expression: false, expression: String::new(), re, im, is_error: false, error_message: None }
    }
    fn from_expression(s: String) -> ExathLine {
        ExathLine { is_expression: true, expression: s, re: 0.0, im: 0.0, is_error: false, error_message: None }
    }
    fn from_error(msg: String) -> ExathLine {
        ExathLine { is_expression: false, expression: String::new(), re: 0.0, im: 0.0, is_error: true, error_message: Some(msg) }
    }
}

#[wasm_bindgen]
impl ExathLine {
    /// True if the result is a symbolic expression (read `.expression`).
    #[wasm_bindgen(getter, js_name = isExpression)]
    pub fn is_expression(&self) -> bool {
        self.is_expression
    }

    /// The symbolic expression string (empty for numeric results).
    #[wasm_bindgen(getter)]
    pub fn expression(&self) -> String {
        self.expression.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn re(&self) -> f64 {
        self.re
    }

    #[wasm_bindgen(getter)]
    pub fn im(&self) -> f64 {
        self.im
    }

    #[wasm_bindgen(getter, js_name = isComplex)]
    pub fn is_complex(&self) -> bool {
        self.im != 0.0
    }

    #[wasm_bindgen(getter, js_name = isError)]
    pub fn is_error(&self) -> bool {
        self.is_error
    }

    #[wasm_bindgen(getter, js_name = errorMessage)]
    pub fn error_message(&self) -> Option<String> {
        self.error_message.clone()
    }
}

