/// Error type for all exath-engine operations.
///
/// Every public function returns `Result<_, ExathError>`.
/// The `Display` impl produces a human-readable message suitable for UIs and logs.
/// The `kind` field allows callers to branch on the error category without parsing strings.

use std::fmt;

/// Category of error, for programmatic handling.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    /// The expression string has invalid syntax or unexpected tokens.
    ParseError,
    /// A variable or function name was used before being defined.
    UndefinedName,
    /// A function received the wrong number of arguments.
    ArgumentCount,
    /// An argument had the wrong type (e.g. complex where real is required).
    ArgumentType,
    /// A mathematical domain was violated (ln(0), division by zero, etc.).
    DomainError,
    /// Overflow in integer arithmetic (gcd/lcm).
    Overflow,
    /// Numerical method produced a complex intermediate result.
    ComplexResult,
    /// Sum/product range exceeded the built-in limit.
    RangeTooLarge,
}

/// An error returned by any exath-engine function.
#[derive(Debug, Clone)]
pub struct ExathError {
    pub kind: ErrorKind,
    pub message: String,
}

impl ExathError {
    pub fn parse(msg: impl Into<String>) -> Self {
        ExathError {
            kind: ErrorKind::ParseError,
            message: msg.into(),
        }
    }

    pub fn undefined(msg: impl Into<String>) -> Self {
        ExathError {
            kind: ErrorKind::UndefinedName,
            message: msg.into(),
        }
    }

    pub fn arg_count(msg: impl Into<String>) -> Self {
        ExathError {
            kind: ErrorKind::ArgumentCount,
            message: msg.into(),
        }
    }

    pub fn arg_type(msg: impl Into<String>) -> Self {
        ExathError {
            kind: ErrorKind::ArgumentType,
            message: msg.into(),
        }
    }

    pub fn domain(msg: impl Into<String>) -> Self {
        ExathError {
            kind: ErrorKind::DomainError,
            message: msg.into(),
        }
    }

    pub fn overflow(msg: impl Into<String>) -> Self {
        ExathError {
            kind: ErrorKind::Overflow,
            message: msg.into(),
        }
    }

    pub fn complex_result(msg: impl Into<String>) -> Self {
        ExathError {
            kind: ErrorKind::ComplexResult,
            message: msg.into(),
        }
    }

    pub fn range_too_large(msg: impl Into<String>) -> Self {
        ExathError {
            kind: ErrorKind::RangeTooLarge,
            message: msg.into(),
        }
    }
}

impl fmt::Display for ExathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ExathError {}
