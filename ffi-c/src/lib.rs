use exath_engine::{
    AngleMode, CalcResult, Session,
    evaluate_complex, is_valid, deriv, integrate, sum, prod,
};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// ── Angle mode ────────────────────────────────────────────────────────────────

/// Angle mode constants for use from C.
#[repr(C)]
pub enum ExathAngleMode {
    Deg  = 0,
    Rad  = 1,
    Grad = 2,
}

fn to_angle_mode(mode: &ExathAngleMode) -> AngleMode {
    match mode {
        ExathAngleMode::Deg  => AngleMode::Deg,
        ExathAngleMode::Rad  => AngleMode::Rad,
        ExathAngleMode::Grad => AngleMode::Grad,
    }
}

// ── Result type ───────────────────────────────────────────────────────────────

/// Result returned from evaluation functions.
/// If is_error == 0: re and im contain the result (im == 0 for real results).
/// If is_error == 1: error_msg contains a null-terminated error string.
///   Free it with exath_free_string() after use.
#[repr(C)]
pub struct ExathResult {
    pub re: f64,
    pub im: f64,
    pub is_error: i32,
    pub error_msg: *mut c_char,
}

fn ok_result(re: f64, im: f64) -> ExathResult {
    ExathResult {
        re,
        im,
        is_error: 0,
        error_msg: std::ptr::null_mut(),
    }
}

fn error_result(msg: &str) -> ExathResult {
    let sanitized = msg.replace('\0', "");
    let c_msg = match CString::new(sanitized) {
        Ok(cstring) => cstring,
        Err(_) => {
            CString::new("Unknown error (message contained invalid data)")
                .expect("static literal")
        }
    };
    ExathResult {
        re: 0.0,
        im: 0.0,
        is_error: 1,
        error_msg: c_msg.into_raw(),
    }
}

fn calc_to_result(result: Result<CalcResult, exath_engine::ExathError>) -> ExathResult {
    match result {
        Ok(CalcResult::Real(re)) => ok_result(re, 0.0),
        Ok(CalcResult::Complex(re, im)) => ok_result(re, im),
        Err(err) => error_result(&err.to_string()),
    }
}

// ── Free ─────────────────────────────────────────────────────────────────────

/// Free an error_msg string returned by any exath function.
#[no_mangle]
pub extern "C" fn exath_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

// ── Evaluate ─────────────────────────────────────────────────────────────────

/// Evaluate an expression string.
/// Returns ExathResult — free error_msg with exath_free_string() if is_error == 1.
#[no_mangle]
pub extern "C" fn exath_evaluate(
    expr: *const c_char,
    angle_mode: ExathAngleMode,
) -> ExathResult {
    let expr_str = match parse_cstr(expr) {
        Ok(str) => str,
        Err(err) => return error_result(&err),
    };
    calc_to_result(evaluate_complex(expr_str, to_angle_mode(&angle_mode)))
}

// ── is_valid ─────────────────────────────────────────────────────────────────

/// Returns 1 if the expression parses correctly, 0 otherwise.
#[no_mangle]
pub extern "C" fn exath_is_valid(expr: *const c_char) -> i32 {
    match parse_cstr(expr) {
        Ok(str) => {
            if is_valid(str) { 1 } else { 0 }
        }
        Err(_) => 0,
    }
}

// ── Supported functions ───────────────────────────────────────────────────────

/// Returns a null-terminated, comma-separated list of supported function names.
/// Free the result with exath_free_string().
#[no_mangle]
pub extern "C" fn exath_supported_functions() -> *mut c_char {
    let list = exath_engine::supported_functions().join(",");
    to_c_string(&list).into_raw()
}

// ── Numerical methods ─────────────────────────────────────────────────────────

/// Numerically differentiate expr w.r.t. var at x.
#[no_mangle]
pub extern "C" fn exath_deriv(
    expr: *const c_char,
    var: *const c_char,
    x: f64,
    angle_mode: ExathAngleMode,
) -> ExathResult {
    let (expr_str, var_str) = match (parse_cstr(expr), parse_cstr(var)) {
        (Ok(expr_str), Ok(var_str)) => (expr_str, var_str),
        _ => return error_result("Invalid UTF-8"),
    };
    match deriv(expr_str, var_str, x, to_angle_mode(&angle_mode)) {
        Ok(value) => ok_result(value, 0.0),
        Err(err) => error_result(&err.to_string()),
    }
}

/// Numerically integrate expr w.r.t. var from a to b.
#[no_mangle]
pub extern "C" fn exath_integrate(
    expr: *const c_char,
    var: *const c_char,
    a: f64,
    b: f64,
    angle_mode: ExathAngleMode,
) -> ExathResult {
    let (expr_str, var_str) = match (parse_cstr(expr), parse_cstr(var)) {
        (Ok(expr_str), Ok(var_str)) => (expr_str, var_str),
        _ => return error_result("Invalid UTF-8"),
    };
    match integrate(expr_str, var_str, a, b, to_angle_mode(&angle_mode)) {
        Ok(value) => ok_result(value, 0.0),
        Err(err) => error_result(&err.to_string()),
    }
}

/// Compute Σ expr for var = from to to (inclusive).
#[no_mangle]
pub extern "C" fn exath_sum(
    expr: *const c_char,
    var: *const c_char,
    from: i64,
    to: i64,
    angle_mode: ExathAngleMode,
) -> ExathResult {
    let (expr_str, var_str) = match (parse_cstr(expr), parse_cstr(var)) {
        (Ok(expr_str), Ok(var_str)) => (expr_str, var_str),
        _ => return error_result("Invalid UTF-8"),
    };
    match sum(expr_str, var_str, from, to, to_angle_mode(&angle_mode)) {
        Ok(value) => ok_result(value, 0.0),
        Err(err) => error_result(&err.to_string()),
    }
}

/// Compute Π expr for var = from to to (inclusive).
#[no_mangle]
pub extern "C" fn exath_prod(
    expr: *const c_char,
    var: *const c_char,
    from: i64,
    to: i64,
    angle_mode: ExathAngleMode,
) -> ExathResult {
    let (expr_str, var_str) = match (parse_cstr(expr), parse_cstr(var)) {
        (Ok(expr_str), Ok(var_str)) => (expr_str, var_str),
        _ => return error_result("Invalid UTF-8"),
    };
    match prod(expr_str, var_str, from, to, to_angle_mode(&angle_mode)) {
        Ok(value) => ok_result(value, 0.0),
        Err(err) => error_result(&err.to_string()),
    }
}

// ── Session ───────────────────────────────────────────────────────────────────

/// Opaque session handle.  Allocate with exath_session_new(), free with exath_session_free().
pub struct ExathSession(Session);

/// Create a new session.
#[no_mangle]
pub extern "C" fn exath_session_new(angle_mode: ExathAngleMode) -> *mut ExathSession {
    let session = ExathSession(Session::new(to_angle_mode(&angle_mode)));
    Box::into_raw(Box::new(session))
}

/// Free a session created with exath_session_new().
#[no_mangle]
pub extern "C" fn exath_session_free(session: *mut ExathSession) {
    if !session.is_null() {
        unsafe {
            drop(Box::from_raw(session));
        }
    }
}

/// Evaluate one line in a session (may be `var = expr` or a plain expression).
/// Returns ExathResult — free error_msg with exath_free_string() if is_error == 1.
#[no_mangle]
pub extern "C" fn exath_session_eval(
    session: *mut ExathSession,
    line: *const c_char,
) -> ExathResult {
    let line_str = match parse_cstr(line) {
        Ok(str) => str,
        Err(err) => return error_result(&err),
    };
    let inner = unsafe { &mut (*session).0 };
    calc_to_result(inner.eval(line_str))
}

/// Set a variable in the session.  im = 0.0 for real values.
#[no_mangle]
pub extern "C" fn exath_session_set_var(
    session: *mut ExathSession,
    name: *const c_char,
    re: f64,
    im: f64,
) {
    if let Ok(name_str) = parse_cstr(name) {
        unsafe {
            (*session).0.set_var(name_str, re, im);
        }
    }
}

/// Remove a variable from the session.
#[no_mangle]
pub extern "C" fn exath_session_remove_var(
    session: *mut ExathSession,
    name: *const c_char,
) {
    if let Ok(name_str) = parse_cstr(name) {
        unsafe {
            (*session).0.remove_var(name_str);
        }
    }
}

/// Clear all variables in the session.
#[no_mangle]
pub extern "C" fn exath_session_clear_vars(session: *mut ExathSession) {
    unsafe {
        (*session).0.clear_vars();
    }
}

/// Remove a user-defined function from the session.
#[no_mangle]
pub extern "C" fn exath_session_remove_fn(
    session: *mut ExathSession,
    name: *const c_char,
) {
    if let Ok(name_str) = parse_cstr(name) {
        unsafe {
            (*session).0.remove_fn(name_str);
        }
    }
}

/// Returns a null-terminated, comma-separated list of user-defined function names.
/// Free the result with exath_free_string().
#[no_mangle]
pub extern "C" fn exath_session_fn_names(session: *mut ExathSession) -> *mut c_char {
    let names = unsafe { (*session).0.fn_names() };
    to_c_string(&names.join(",")).into_raw()
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn parse_cstr<'a>(ptr: *const c_char) -> Result<&'a str, String> {
    unsafe {
        CStr::from_ptr(ptr)
            .to_str()
            .map_err(|_| "Invalid UTF-8".to_string())
    }
}

fn to_c_string(input: &str) -> CString {
    match CString::new(input) {
        Ok(cstring) => cstring,
        Err(_) => {
            let sanitized = input.replace('\0', "");
            CString::new(sanitized).expect("sanitized string still contains NUL")
        }
    }
}
