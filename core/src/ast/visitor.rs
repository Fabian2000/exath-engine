use super::types::Ast;

/// Collect all variable names referenced in an AST.
pub fn collect_vars(ast: &Ast) -> Vec<String> {
    let mut vars = Vec::new();
    collect_vars_rec(ast, &mut vars);
    vars.sort();
    vars.dedup();
    vars
}

fn collect_vars_rec(ast: &Ast, out: &mut Vec<String>) {
    match ast {
        Ast::Var(name) => {
            out.push(name.clone());
        }
        Ast::BinOp(_, left, right) => {
            collect_vars_rec(left, out);
            collect_vars_rec(right, out);
        }
        Ast::UnaryNeg(inner) | Ast::UnaryNot(inner) | Ast::Factorial(inner) => {
            collect_vars_rec(inner, out);
        }
        Ast::Call(_, args) => {
            for arg in args {
                collect_vars_rec(arg, out);
            }
        }
        Ast::Number(_) => {}
    }
}
