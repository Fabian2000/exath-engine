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
        "gamma", "lgamma", "erf", "erfc", "digamma", "beta",
        "isprime", "nextprime", "totient", "powmod", "factorint",
        "mean", "median", "variance", "stddev", "npdf", "ncdf", "binom",
        // Rounding
        "floor", "ceil", "round", "trunc", "frac",
        // Sign
        "sign", "sgn",
        // Angle conversion
        "deg", "rad",
        // Control flow / multi-argument
        "if", "piecewise", "min", "max", "clamp", "gcd", "lcm", "assume", "abs",
        "sum", "product", "deriv", "convert",
        // Symbolic / calculus forms (usable via a session, e.g. eval_line)
        "diff", "simplify", "integral", "solve", "factor", "polygcd", "nsolve", "expand", "taylor", "limit",
        "grad", "jacobian", "hessian", "odesolve", "minimize", "maximize", "sumc", "laplace", "dsolve",
        // Matrix functions (used with [[..],[..]] literals via a session)
        "det", "inv", "transpose", "trace", "rank", "norm", "svdvals", "charpoly", "identity", "linsolve", "eigenvalues", "eigenvectors",
    ]
}

// ── parse ─────────────────────────────────────────────────────────────────────

/// Parse an expression string into an AST.
/// The returned AST can be inspected or passed to `eval_ast`.
pub use ast::parse_str as parse;
