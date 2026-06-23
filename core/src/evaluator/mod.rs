mod cx;
mod calc_result;
mod factorial;
mod functions;
mod session;

pub use cx::Cx;
pub use calc_result::CalcResult;
pub use factorial::factorial;
pub use functions::apply_function;
pub use session::{Session, LineResult};

use crate::angle_mode::AngleMode;
use crate::ast::{eval_ast, UserFns};
use crate::error::ExathError;
use std::collections::HashMap;

/// Evaluate an expression to a real `f64` (stateless, numeric only).
///
/// Identical to [`evaluate_complex`] except it errors when the result is
/// complex, use this when you specifically want a real number. Does not
/// understand symbolic forms like `diff(x^2, x)` or `factor(...)`; for those
/// use [`Session::eval_line`].
pub fn evaluate(expr: &str, angle_mode: AngleMode) -> Result<f64, ExathError> {
    match evaluate_complex(expr, angle_mode)? {
        CalcResult::Real(value) => Ok(value),
        CalcResult::Complex(_, _) => Err(ExathError::complex_result("Result is complex")),
    }
}

/// Evaluate an expression to a [`CalcResult`], Real or Complex (stateless,
/// numeric only).
///
/// Same evaluation as [`evaluate`], but keeps complex results instead of
/// erroring on them. For stateful evaluation (variables, user functions) use
/// [`Session::eval`]; for symbolic forms use [`Session::eval_line`].
pub fn evaluate_complex(expr: &str, angle_mode: AngleMode) -> Result<CalcResult, ExathError> {
    evaluate_with_vars(expr, angle_mode, &HashMap::new())
}

/// Evaluate an expression with a variable map.
pub fn evaluate_with_vars(
    expr: &str,
    angle_mode: AngleMode,
    vars: &HashMap<String, Cx>,
) -> Result<CalcResult, ExathError> {
    evaluate_with_vars_and_fns(expr, angle_mode, vars, &UserFns::new())
}

/// Evaluate an expression with a variable map and user-defined functions.
pub fn evaluate_with_vars_and_fns(
    expr: &str,
    angle_mode: AngleMode,
    vars: &HashMap<String, Cx>,
    fns: &UserFns,
) -> Result<CalcResult, ExathError> {
    let ast = crate::ast::parse_str(expr)?;
    let result = eval_ast(&ast, vars, fns, angle_mode)?;
    Ok(result.to_calc_result())
}
