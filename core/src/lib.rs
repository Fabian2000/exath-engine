pub mod angle_mode;
pub mod ast;
pub mod error;
pub mod evaluator;
pub mod analysis;
pub mod interval;
pub mod matrix;
pub mod numerics;
pub mod rational;
pub mod symbolic;
pub mod units;

// ── Convenience re-exports ────────────────────────────────────────────────────

pub use angle_mode::AngleMode;
pub use error::{ExathError, ErrorKind};
pub use evaluator::{
    CalcResult, Session, LineResult,
    evaluate, evaluate_complex, evaluate_with_vars, evaluate_with_vars_and_fns,
};
pub use analysis::{is_valid, supported_functions};
pub use ast::{Ast, BinOp, UserFns, parse_str};
pub use matrix::Matrix;
pub use interval::Interval;
pub use units::Quantity;
