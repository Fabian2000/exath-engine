/// Abstract Syntax Tree for exath-engine expressions.
///
/// Separates parsing from evaluation so the tree can be reused
/// for derivatives, integration, and serialization.

mod types;
mod tokenizer;
mod parser;
mod eval;
mod visitor;

pub use types::{Ast, BinOp};
pub use parser::parse_str;
pub use eval::{eval_ast, UserFns};
pub use visitor::collect_vars;
