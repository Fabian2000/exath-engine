use crate::error::ExathError;
use super::tokenizer::{Token, tokenize};
use super::types::{Ast, BinOp};

/// Parse an expression string into an AST.
pub fn parse_str(input: &str) -> Result<Ast, ExathError> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let node = parse_expr(&tokens, &mut pos)?;
    if pos < tokens.len() {
        return Err(ExathError::parse("Unexpected token after expression"));
    }
    Ok(node)
}

// Precedence (low → high):
//   logical or  (||)
//   logical and (&&)
//   comparison  (== != < <= > >=)
//   addition    (+ -)
//   term        (* / %)
//   power       (^)
//   unary       (- !)
//   primary     (number, ident, call, parens)

fn parse_expr(tokens: &[Token], pos: &mut usize) -> Result<Ast, ExathError> {
    parse_or(tokens, pos)
}

fn parse_or(tokens: &[Token], pos: &mut usize) -> Result<Ast, ExathError> {
    let mut left = parse_and(tokens, pos)?;
    while *pos < tokens.len() {
        if let Token::OrOr = &tokens[*pos] {
            *pos += 1;
            let right = parse_and(tokens, pos)?;
            left = Ast::BinOp(BinOp::Or, Box::new(left), Box::new(right));
        } else {
            break;
        }
    }
    Ok(left)
}

fn parse_and(tokens: &[Token], pos: &mut usize) -> Result<Ast, ExathError> {
    let mut left = parse_comparison(tokens, pos)?;
    while *pos < tokens.len() {
        if let Token::AndAnd = &tokens[*pos] {
            *pos += 1;
            let right = parse_comparison(tokens, pos)?;
            left = Ast::BinOp(BinOp::And, Box::new(left), Box::new(right));
        } else {
            break;
        }
    }
    Ok(left)
}

fn parse_comparison(tokens: &[Token], pos: &mut usize) -> Result<Ast, ExathError> {
    let mut left = parse_add(tokens, pos)?;
    while *pos < tokens.len() {
        let op = match &tokens[*pos] {
            Token::EqEq => BinOp::Eq,
            Token::Ne => BinOp::Ne,
            Token::Lt => BinOp::Lt,
            Token::Le => BinOp::Le,
            Token::Gt => BinOp::Gt,
            Token::Ge => BinOp::Ge,
            _ => break,
        };
        *pos += 1;
        let right = parse_add(tokens, pos)?;
        left = Ast::BinOp(op, Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn parse_add(tokens: &[Token], pos: &mut usize) -> Result<Ast, ExathError> {
    let mut left = parse_term(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Plus => {
                *pos += 1;
                let right = parse_term(tokens, pos)?;
                left = Ast::BinOp(BinOp::Add, Box::new(left), Box::new(right));
            }
            Token::Minus => {
                *pos += 1;
                let right = parse_term(tokens, pos)?;
                left = Ast::BinOp(BinOp::Sub, Box::new(left), Box::new(right));
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_term(tokens: &[Token], pos: &mut usize) -> Result<Ast, ExathError> {
    let mut left = parse_power(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Mul => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                left = Ast::BinOp(BinOp::Mul, Box::new(left), Box::new(right));
            }
            Token::Div => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                left = Ast::BinOp(BinOp::Div, Box::new(left), Box::new(right));
            }
            Token::Mod => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                left = Ast::BinOp(BinOp::Mod, Box::new(left), Box::new(right));
            }
            // Implicit multiplication: expression followed by ( or identifier
            Token::LParen | Token::Ident(_) => {
                let right = parse_power(tokens, pos)?;
                left = Ast::BinOp(BinOp::Mul, Box::new(left), Box::new(right));
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_power(tokens: &[Token], pos: &mut usize) -> Result<Ast, ExathError> {
    let base = parse_unary(tokens, pos)?;
    if *pos < tokens.len() {
        if let Token::Pow = &tokens[*pos] {
            *pos += 1;
            let exponent = parse_power(tokens, pos)?; // right-associative
            return Ok(Ast::BinOp(BinOp::Pow, Box::new(base), Box::new(exponent)));
        }
    }
    // Postfix factorial(s)
    let mut result = base;
    while *pos < tokens.len() {
        if let Token::Factorial = &tokens[*pos] {
            *pos += 1;
            result = Ast::Factorial(Box::new(result));
        } else {
            break;
        }
    }
    Ok(result)
}

fn parse_unary(tokens: &[Token], pos: &mut usize) -> Result<Ast, ExathError> {
    if *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Minus => {
                *pos += 1;
                let inner = parse_primary(tokens, pos)?;
                return Ok(Ast::UnaryNeg(Box::new(inner)));
            }
            Token::Plus => {
                *pos += 1;
                return parse_primary(tokens, pos);
            }
            Token::Factorial => {
                *pos += 1;
                let inner = parse_primary(tokens, pos)?;
                return Ok(Ast::UnaryNot(Box::new(inner)));
            }
            _ => {}
        }
    }
    parse_primary(tokens, pos)
}

fn parse_primary(tokens: &[Token], pos: &mut usize) -> Result<Ast, ExathError> {
    if *pos >= tokens.len() {
        return Err(ExathError::parse("Unexpected end of expression"));
    }
    match &tokens[*pos].clone() {
        Token::Number(value) => {
            *pos += 1;
            Ok(Ast::Number(*value))
        }
        Token::Ident(name) => {
            let name = name.clone();
            *pos += 1;
            if *pos < tokens.len() && matches!(&tokens[*pos], Token::LParen) {
                *pos += 1;
                let args = parse_arg_list(tokens, pos)?;
                if *pos < tokens.len() && matches!(&tokens[*pos], Token::RParen) {
                    *pos += 1;
                } else {
                    return Err(ExathError::parse("Missing ')'"));
                }
                Ok(Ast::Call(name, args))
            } else if is_function(&name) {
                let arg = parse_unary(tokens, pos)?;
                Ok(Ast::Call(name, vec![arg]))
            } else {
                resolve_const_or_var(name)
            }
        }
        Token::LParen => {
            *pos += 1;
            let inner = parse_expr(tokens, pos)?;
            if *pos < tokens.len() && matches!(&tokens[*pos], Token::RParen) {
                *pos += 1;
            } else {
                return Err(ExathError::parse("Missing ')'"));
            }
            Ok(inner)
        }
        _ => Err(ExathError::parse("Unexpected token")),
    }
}

fn parse_arg_list(tokens: &[Token], pos: &mut usize) -> Result<Vec<Ast>, ExathError> {
    let mut args = Vec::new();
    if *pos < tokens.len() && matches!(&tokens[*pos], Token::RParen) {
        return Ok(args);
    }
    args.push(parse_expr(tokens, pos)?);
    while *pos < tokens.len() && matches!(&tokens[*pos], Token::Comma) {
        *pos += 1;
        args.push(parse_expr(tokens, pos)?);
    }
    Ok(args)
}

/// Returns true if the identifier is a known function name.
fn is_function(name: &str) -> bool {
    matches!(
        name,
        "sin"  | "cos"  | "tan"  | "cot"  | "sec"  | "csc"  |
        "asin" | "acos" | "atan" | "acot" | "asec" | "acsc" |
        "sinh"  | "cosh"  | "tanh"  | "coth"  | "sech"  | "csch" |
        "asinh" | "acosh" | "atanh" | "acoth" | "asech" | "acsch" |
        "ln" | "lg" | "log" | "exp" |
        "sqrt" | "cbrt" | "abs" |
        "floor" | "ceil" | "round" | "trunc" | "frac" |
        "sign" | "sgn" | "arg" | "conj" | "real" | "imag" |
        "deg" | "rad" |
        "if" | "min" | "max" | "clamp" | "gcd" | "lcm"
    ) || name.starts_with("log:")
}

/// Resolve a bare identifier to a constant literal or a Var node.
fn resolve_const_or_var(name: String) -> Result<Ast, ExathError> {
    match name.as_str() {
        "e" => Ok(Ast::Number(std::f64::consts::E)),
        "pi" | "\u{03c0}" => Ok(Ast::Number(std::f64::consts::PI)),
        "phi" | "\u{03d5}" => Ok(Ast::Number(1.618_033_988_749_895)),
        "\u{03b5}" | "epsilon" => Ok(Ast::Number(std::f64::consts::E)),
        "mod" => Err(ExathError::parse("'mod' must be used as a binary operator")),
        _ => Ok(Ast::Var(name)),
    }
}
