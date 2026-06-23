use exath_engine::{
    AngleMode, CalcResult, Session, LineResult,
    evaluate_complex, is_valid,
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
/// Returns ExathResult, free error_msg with exath_free_string() if is_error == 1.
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
/// Returns ExathResult, free error_msg with exath_free_string() if is_error == 1.
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

/// Result of exath_session_eval_line.
/// If is_error == 1: error_msg holds the message (free with exath_free_string).
/// Else if is_expression == 1: expression holds a symbolic expression string
///   (free with exath_free_string); re/im are 0.
/// Else: re/im hold the numeric value and expression is NULL.
#[repr(C)]
pub struct ExathLineResult {
    pub is_expression: i32,
    pub expression: *mut c_char,
    pub re: f64,
    pub im: f64,
    pub is_error: i32,
    pub error_msg: *mut c_char,
}

fn line_value(re: f64, im: f64) -> ExathLineResult {
    ExathLineResult {
        is_expression: 0,
        expression: std::ptr::null_mut(),
        re,
        im,
        is_error: 0,
        error_msg: std::ptr::null_mut(),
    }
}

fn line_error(msg: &str) -> ExathLineResult {
    let c_msg = CString::new(msg.replace('\0', ""))
        .unwrap_or_else(|_| CString::new("Unknown error").expect("static literal"));
    ExathLineResult {
        is_expression: 0,
        expression: std::ptr::null_mut(),
        re: 0.0,
        im: 0.0,
        is_error: 1,
        error_msg: c_msg.into_raw(),
    }
}

/// Evaluate one line, understanding every DSL form, symbolic (diff, simplify,
/// factor, solve, integral, …), linear algebra (det, inv, eigenvalues, …) and
/// numeric forms (sum, product, deriv). See ExathLineResult for the result
/// convention. This is the single gateway for all operations.
#[no_mangle]
pub extern "C" fn exath_session_eval_line(
    session: *mut ExathSession,
    line: *const c_char,
) -> ExathLineResult {
    let line_str = match parse_cstr(line) {
        Ok(str) => str,
        Err(err) => return line_error(&err),
    };
    let inner = unsafe { &mut (*session).0 };
    match inner.eval_line(line_str) {
        Ok(LineResult::Value(CalcResult::Real(re))) => line_value(re, 0.0),
        Ok(LineResult::Value(CalcResult::Complex(re, im))) => line_value(re, im),
        Ok(LineResult::Expression(s)) => ExathLineResult {
            is_expression: 1,
            expression: to_c_string(&s).into_raw(),
            re: 0.0,
            im: 0.0,
            is_error: 0,
            error_msg: std::ptr::null_mut(),
        },
        Err(e) => line_error(&e.to_string()),
    }
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

/// Returns a null-terminated, comma-separated list of variable names.
/// Free the result with exath_free_string().
#[no_mangle]
pub extern "C" fn exath_session_var_names(session: *mut ExathSession) -> *mut c_char {
    let names = unsafe { (*session).0.var_names() };
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
