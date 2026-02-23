pub mod angle_mode;
pub mod ast;
pub mod error;
pub mod evaluator;
pub mod analysis;
pub mod numerics;

// ── Convenience re-exports ────────────────────────────────────────────────────

pub use angle_mode::AngleMode;
pub use error::{ExathError, ErrorKind};
pub use evaluator::{
    CalcResult, Session,
    evaluate, evaluate_complex, evaluate_with_vars, evaluate_with_vars_and_fns,
};
pub use analysis::{is_valid, supported_functions};
pub use numerics::{deriv, integrate, sum, prod};
pub use ast::{Ast, BinOp, UserFns, parse_str};
