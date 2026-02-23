/// Binary operators supported by the expression language.
#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Mod,
    // Comparison operators — result is 1.0 (true) or 0.0 (false)
    Eq,   // ==
    Ne,   // !=
    Lt,   // <
    Le,   // <=
    Gt,   // >
    Ge,   // >=
    // Logical
    And,  // &&
    Or,   // ||
}

/// Abstract Syntax Tree node for an exath-engine expression.
#[derive(Debug, Clone)]
pub enum Ast {
    /// A numeric literal (real-valued leaf)
    Number(f64),
    /// A variable reference by name
    Var(String),
    /// Binary operation
    BinOp(BinOp, Box<Ast>, Box<Ast>),
    /// Unary negation
    UnaryNeg(Box<Ast>),
    /// Logical NOT: !expr  →  1 if expr==0, else 0
    UnaryNot(Box<Ast>),
    /// Postfix factorial  n!
    Factorial(Box<Ast>),
    /// Function call with zero or more arguments: name(a, b, ...)
    Call(String, Vec<Ast>),
}
