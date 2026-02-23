mod cx;
mod calc_result;
mod factorial;
mod functions;
mod session;

pub use cx::Cx;
pub use calc_result::CalcResult;
pub use factorial::factorial;
pub use functions::apply_function;
pub use session::Session;

use crate::angle_mode::AngleMode;
use crate::ast::{eval_ast, UserFns};
use crate::error::ExathError;
use std::collections::HashMap;

/// Evaluate an expression, returning a real f64.
/// Returns Err if the result is complex or the expression is invalid.
pub fn evaluate(expr: &str, angle_mode: AngleMode) -> Result<f64, ExathError> {
    match evaluate_complex(expr, angle_mode)? {
        CalcResult::Real(value) => Ok(value),
        CalcResult::Complex(_, _) => Err(ExathError::complex_result("Result is complex")),
    }
}

/// Evaluate an expression, returning a CalcResult (Real or Complex).
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
