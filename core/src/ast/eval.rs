use crate::angle_mode::AngleMode;
use crate::error::ExathError;
use crate::evaluator::{Cx, apply_function, factorial};
use super::types::{Ast, BinOp};
use std::collections::HashMap;

/// A map of user-defined functions: name → (parameter names, body AST).
pub type UserFns = HashMap<String, (Vec<String>, Ast)>;

/// Evaluate an AST with a variable map and user-defined functions.
pub fn eval_ast(
    ast: &Ast,
    vars: &HashMap<String, Cx>,
    fns: &UserFns,
    angle_mode: AngleMode,
) -> Result<Cx, ExathError> {
    match ast {
        Ast::Number(value) => Ok(Cx::real(*value)),

        Ast::Var(name) => vars
            .get(name)
            .copied()
            .ok_or_else(|| ExathError::undefined(format!("Undefined variable: {}", name))),

        Ast::BinOp(op, left_ast, right_ast) => {
            // Short-circuit for logical operators
            match op {
                BinOp::And => {
                    let left = eval_ast(left_ast, vars, fns, angle_mode)?;
                    if left.re == 0.0 && left.im == 0.0 {
                        return Ok(Cx::real(0.0));
                    }
                    let right = eval_ast(right_ast, vars, fns, angle_mode)?;
                    let truthy = right.re != 0.0 || right.im != 0.0;
                    return Ok(Cx::real(if truthy { 1.0 } else { 0.0 }));
                }
                BinOp::Or => {
                    let left = eval_ast(left_ast, vars, fns, angle_mode)?;
                    if left.re != 0.0 || left.im != 0.0 {
                        return Ok(Cx::real(1.0));
                    }
                    let right = eval_ast(right_ast, vars, fns, angle_mode)?;
                    let truthy = right.re != 0.0 || right.im != 0.0;
                    return Ok(Cx::real(if truthy { 1.0 } else { 0.0 }));
                }
                _ => {}
            }

            let left = eval_ast(left_ast, vars, fns, angle_mode)?;
            let right = eval_ast(right_ast, vars, fns, angle_mode)?;
            match op {
                BinOp::Add => Ok(left.add(right)),
                BinOp::Sub => Ok(left.sub(right)),
                BinOp::Mul => Ok(left.mul(right)),
                BinOp::Div => left.div(right),
                BinOp::Pow => left.pow(right),
                BinOp::Mod => {
                    if right.re == 0.0 && right.im == 0.0 {
                        return Err(ExathError::domain("Modulo by zero"));
                    }
                    if !right.is_real() {
                        return Err(ExathError::arg_type(
                            "Modulo only defined for real numbers",
                        ));
                    }
                    Ok(Cx::real(left.re % right.re))
                }
                BinOp::Eq => cmp_op(left, right, |a, b| (a - b).abs() < 1e-12),
                BinOp::Ne => cmp_op(left, right, |a, b| (a - b).abs() >= 1e-12),
                BinOp::Lt => cmp_op(left, right, |a, b| a < b),
                BinOp::Le => cmp_op(left, right, |a, b| a <= b),
                BinOp::Gt => cmp_op(left, right, |a, b| a > b),
                BinOp::Ge => cmp_op(left, right, |a, b| a >= b),
                BinOp::And | BinOp::Or => unreachable!(),
            }
        }

        Ast::UnaryNeg(inner) => {
            Ok(eval_ast(inner, vars, fns, angle_mode)?.neg())
        }

        Ast::UnaryNot(inner) => {
            let value = eval_ast(inner, vars, fns, angle_mode)?;
            let is_zero = value.re == 0.0 && value.im == 0.0;
            Ok(Cx::real(if is_zero { 1.0 } else { 0.0 }))
        }

        Ast::Factorial(inner) => {
            let value = eval_ast(inner, vars, fns, angle_mode)?;
            if !value.is_real() {
                return Err(ExathError::arg_type("Factorial only for real numbers"));
            }
            Ok(Cx::real(factorial(value.re)?))
        }

        Ast::Call(name, args) => {
            eval_call(name, args, vars, fns, angle_mode)
        }
    }
}

/// Evaluate a function call with its argument AST nodes (lazy — args not yet evaluated).
fn eval_call(
    name: &str,
    args: &[Ast],
    vars: &HashMap<String, Cx>,
    fns: &UserFns,
    angle_mode: AngleMode,
) -> Result<Cx, ExathError> {
    // User-defined functions
    if let Some((params, body)) = fns.get(name) {
        if args.len() != params.len() {
            return Err(ExathError::arg_count(format!(
                "{}() expects {} argument(s), got {}",
                name,
                params.len(),
                args.len()
            )));
        }
        let mut call_vars = vars.clone();
        for (param, arg_ast) in params.iter().zip(args.iter()) {
            let value = eval_ast(arg_ast, vars, fns, angle_mode)?;
            call_vars.insert(param.clone(), value);
        }
        return eval_ast(body, &call_vars, fns, angle_mode);
    }

    // Multi-argument / control-flow built-in functions
    match name {
        "if" => {
            if args.len() != 3 {
                return Err(ExathError::arg_count(
                    "if requires 3 arguments: if(condition, true_value, false_value)",
                ));
            }
            let condition = eval_ast(&args[0], vars, fns, angle_mode)?;
            if condition.re != 0.0 || condition.im != 0.0 {
                eval_ast(&args[1], vars, fns, angle_mode)
            } else {
                eval_ast(&args[2], vars, fns, angle_mode)
            }
        }

        "min" => {
            if args.is_empty() {
                return Err(ExathError::arg_count("min requires at least one argument"));
            }
            let mut best = eval_real_arg(&args[0], vars, fns, angle_mode, "min")?;
            for arg in &args[1..] {
                let value = eval_real_arg(arg, vars, fns, angle_mode, "min")?;
                if value < best {
                    best = value;
                }
            }
            Ok(Cx::real(best))
        }

        "max" => {
            if args.is_empty() {
                return Err(ExathError::arg_count("max requires at least one argument"));
            }
            let mut best = eval_real_arg(&args[0], vars, fns, angle_mode, "max")?;
            for arg in &args[1..] {
                let value = eval_real_arg(arg, vars, fns, angle_mode, "max")?;
                if value > best {
                    best = value;
                }
            }
            Ok(Cx::real(best))
        }

        "clamp" => {
            if args.len() != 3 {
                return Err(ExathError::arg_count(
                    "clamp requires 3 arguments: clamp(x, min, max)",
                ));
            }
            let value = eval_real_arg(&args[0], vars, fns, angle_mode, "clamp")?;
            let lower = eval_real_arg(&args[1], vars, fns, angle_mode, "clamp")?;
            let upper = eval_real_arg(&args[2], vars, fns, angle_mode, "clamp")?;
            Ok(Cx::real(value.max(lower).min(upper)))
        }

        "gcd" => {
            if args.len() != 2 {
                return Err(ExathError::arg_count("gcd requires 2 arguments"));
            }
            let a = to_integer(eval_real_arg(&args[0], vars, fns, angle_mode, "gcd")?, "gcd")?;
            let b = to_integer(eval_real_arg(&args[1], vars, fns, angle_mode, "gcd")?, "gcd")?;
            Ok(Cx::real(gcd(a.abs(), b.abs()) as f64))
        }

        "lcm" => {
            if args.len() != 2 {
                return Err(ExathError::arg_count("lcm requires 2 arguments"));
            }
            let a = to_integer(eval_real_arg(&args[0], vars, fns, angle_mode, "lcm")?, "lcm")?;
            let b = to_integer(eval_real_arg(&args[1], vars, fns, angle_mode, "lcm")?, "lcm")?;
            let divisor = gcd(a.abs(), b.abs());
            if divisor == 0 {
                return Ok(Cx::real(0.0));
            }
            let result = (a as i128 / divisor as i128 * b as i128).unsigned_abs();
            Ok(Cx::real(result as f64))
        }

        // All single-argument built-in functions
        _ => {
            if args.len() != 1 {
                return Err(ExathError::arg_count(format!(
                    "'{}' requires exactly 1 argument",
                    name
                )));
            }
            let value = eval_ast(&args[0], vars, fns, angle_mode)?;
            apply_function(name, value, angle_mode)
        }
    }
}

fn eval_real_arg(
    ast: &Ast,
    vars: &HashMap<String, Cx>,
    fns: &UserFns,
    angle_mode: AngleMode,
    fname: &str,
) -> Result<f64, ExathError> {
    let value = eval_ast(ast, vars, fns, angle_mode)?;
    if !value.is_real() {
        return Err(ExathError::arg_type(format!(
            "{} only defined for real arguments",
            fname
        )));
    }
    Ok(value.re)
}

fn cmp_op(left: Cx, right: Cx, compare: impl Fn(f64, f64) -> bool) -> Result<Cx, ExathError> {
    if !left.is_real() || !right.is_real() {
        return Err(ExathError::arg_type(
            "Comparison operators only defined for real numbers",
        ));
    }
    Ok(Cx::real(if compare(left.re, right.re) { 1.0 } else { 0.0 }))
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn to_integer(x: f64, fname: &str) -> Result<i64, ExathError> {
    if !x.is_finite() {
        return Err(ExathError::arg_type(format!(
            "{} requires finite integer arguments",
            fname
        )));
    }
    let rounded = x.round();
    if (x - rounded).abs() > 1e-9 {
        return Err(ExathError::arg_type(format!(
            "{} requires integer arguments, got {}",
            fname, x
        )));
    }
    if rounded.abs() > 9.007_199_254_740_992e15_f64 {
        return Err(ExathError::overflow(format!(
            "{} argument too large for integer arithmetic",
            fname
        )));
    }
    Ok(rounded as i64)
}
