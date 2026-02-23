use exath_engine::{
    AngleMode, CalcResult, Session,
    evaluate_complex, is_valid, supported_functions,
    deriv, integrate, sum, prod,
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

// ── Numerical methods ─────────────────────────────────────────────────────────

/// Numerically differentiate expr w.r.t. var at x.
/// Returns ExathResult with `.re` as the derivative (always real), or `.isError`.
#[wasm_bindgen]
pub fn deriv_at(expr: &str, var: &str, x: f64, angle_mode: &str) -> ExathResult {
    match deriv(expr, var, x, parse_angle_mode(angle_mode)) {
        Ok(value) => ExathResult {
            re: value,
            im: 0.0,
            is_complex: false,
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

/// Numerically integrate expr w.r.t. var from a to b.
/// Returns ExathResult with `.re` as the integral (always real), or `.isError`.
#[wasm_bindgen]
pub fn integrate_range(expr: &str, var: &str, a: f64, b: f64, angle_mode: &str) -> ExathResult {
    match integrate(expr, var, a, b, parse_angle_mode(angle_mode)) {
        Ok(value) => ExathResult {
            re: value,
            im: 0.0,
            is_complex: false,
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

/// Compute Σ expr for var = from to to (inclusive).
#[wasm_bindgen]
pub fn sum_range(expr: &str, var: &str, from: i32, to: i32, angle_mode: &str) -> ExathResult {
    match sum(expr, var, from as i64, to as i64, parse_angle_mode(angle_mode)) {
        Ok(value) => ExathResult {
            re: value,
            im: 0.0,
            is_complex: false,
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

/// Compute Π expr for var = from to to (inclusive).
#[wasm_bindgen]
pub fn prod_range(expr: &str, var: &str, from: i32, to: i32, angle_mode: &str) -> ExathResult {
    match prod(expr, var, from as i64, to as i64, parse_angle_mode(angle_mode)) {
        Ok(value) => ExathResult {
            re: value,
            im: 0.0,
            is_complex: false,
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
