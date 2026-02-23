/// Static analysis utilities: validation, function list, AST access.

use crate::ast;

// ── is_valid ──────────────────────────────────────────────────────────────────

/// Returns true if the expression parses without error.
/// Does NOT evaluate it (variables don't need to be defined).
pub fn is_valid(expr: &str) -> bool {
    ast::parse_str(expr).is_ok()
}

// ── supported_functions ───────────────────────────────────────────────────────

/// Returns a list of all built-in function names supported by the engine.
pub fn supported_functions() -> &'static [&'static str] {
    &[
        // Trigonometric
        "sin", "cos", "tan", "cot", "sec", "csc",
        // Inverse trigonometric
        "asin", "acos", "atan", "acot", "asec", "acsc",
        // Hyperbolic
        "sinh", "cosh", "tanh", "coth", "sech", "csch",
        // Inverse hyperbolic
        "asinh", "acosh", "atanh", "acoth", "asech", "acsch",
        // Exponential / logarithmic
        "exp", "ln", "lg", "log",
        // Roots
        "sqrt", "cbrt",
        // Magnitude / complex parts
        "abs", "arg", "conj", "real", "imag",
        // Rounding
        "floor", "ceil", "round", "trunc", "frac",
        // Sign
        "sign", "sgn",
        // Angle conversion
        "deg", "rad",
        // Control flow / multi-argument
        "if", "min", "max", "clamp", "gcd", "lcm",
    ]
}

// ── parse ─────────────────────────────────────────────────────────────────────

/// Parse an expression string into an AST.
/// The returned AST can be inspected or passed to `eval_ast`.
pub use ast::parse_str as parse;
