use crate::error::ExathError;

#[derive(Debug, Clone)]
pub(crate) enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Mul,
    Div,
    Pow,
    Mod,
    Factorial,
    LParen,
    RParen,
    Comma,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
}

pub(crate) fn tokenize(input: &str) -> Result<Vec<Token>, ExathError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        match chars[pos] {
            // Whitespace and calculator marker characters
            ' ' | '\t' | '\u{2041}' | '\u{203E}' | '\u{208D}' | '\u{208E}' => {
                pos += 1;
            }

            '+' => {
                tokens.push(Token::Plus);
                pos += 1;
            }
            '-' | '\u{2212}' => {
                tokens.push(Token::Minus);
                pos += 1;
            }

            '*' | '\u{00d7}' => {
                pos += 1;
                if pos < chars.len() && chars[pos] == '*' {
                    tokens.push(Token::Pow);
                    pos += 1;
                } else {
                    tokens.push(Token::Mul);
                }
            }

            '/' | '\u{00f7}' => {
                tokens.push(Token::Div);
                pos += 1;
            }
            '^' => {
                tokens.push(Token::Pow);
                pos += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                pos += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                pos += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                pos += 1;
            }
            '%' => {
                tokens.push(Token::Mod);
                pos += 1;
            }

            '!' => {
                pos += 1;
                if pos < chars.len() && chars[pos] == '=' {
                    tokens.push(Token::Ne);
                    pos += 1;
                } else {
                    tokens.push(Token::Factorial);
                }
            }

            '=' => {
                pos += 1;
                if pos < chars.len() && chars[pos] == '=' {
                    tokens.push(Token::EqEq);
                    pos += 1;
                } else {
                    return Err(ExathError::parse(
                        "Unexpected '=' in expression (use '==' for equality)",
                    ));
                }
            }

            '<' => {
                pos += 1;
                if pos < chars.len() && chars[pos] == '=' {
                    tokens.push(Token::Le);
                    pos += 1;
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '>' => {
                pos += 1;
                if pos < chars.len() && chars[pos] == '=' {
                    tokens.push(Token::Ge);
                    pos += 1;
                } else {
                    tokens.push(Token::Gt);
                }
            }

            '&' => {
                pos += 1;
                if pos < chars.len() && chars[pos] == '&' {
                    tokens.push(Token::AndAnd);
                    pos += 1;
                } else {
                    return Err(ExathError::parse("Expected '&&'"));
                }
            }

            '|' if pos + 1 < chars.len() && chars[pos + 1] == '|' => {
                tokens.push(Token::OrOr);
                pos += 2;
            }

            // |expr| → abs(expr)
            '|' => {
                tokens.push(Token::Ident("abs".to_string()));
                tokens.push(Token::LParen);
                pos += 1;
                let mut depth = 1;
                while pos < chars.len() && depth > 0 {
                    if chars[pos] == '|' {
                        depth -= 1;
                    }
                    if depth > 0 {
                        tokens.push(match chars[pos] {
                            '+' => Token::Plus,
                            '-' | '\u{2212}' => Token::Minus,
                            '*' | '\u{00d7}' => Token::Mul,
                            '/' | '\u{00f7}' => Token::Div,
                            '^' => Token::Pow,
                            '(' => Token::LParen,
                            ')' => Token::RParen,
                            ch if ch.is_ascii_digit() => match ch.to_digit(10) {
                                Some(digit) => Token::Number(digit as f64),
                                None => {
                                    return Err(ExathError::parse(format!(
                                        "Invalid digit in absolute value: '{}'",
                                        ch
                                    )));
                                }
                            },
                            _ => {
                                pos += 1;
                                continue;
                            }
                        });
                    }
                    pos += 1;
                }
                tokens.push(Token::RParen);
            }

            // Decimal point starting a fractional number (e.g. ".5")
            '.' => {
                let start = pos;
                let mut num_str = String::from("0.");
                pos += 1;
                while pos < chars.len() && chars[pos].is_ascii_digit() {
                    num_str.push(chars[pos]);
                    pos += 1;
                }
                if num_str == "0." {
                    return Err(ExathError::parse(format!(
                        "Unexpected token at position {}",
                        start
                    )));
                }
                let value: f64 = num_str
                    .parse()
                    .map_err(|_| ExathError::parse("Invalid number"))?;
                tokens.push(Token::Number(value));
            }

            // Digits
            ch if ch.is_ascii_digit() => {
                let mut num_str = String::new();
                while pos < chars.len() && (chars[pos].is_ascii_digit() || chars[pos] == '.') {
                    num_str.push(chars[pos]);
                    pos += 1;
                }
                // Accept comma as decimal separator ONLY when immediately followed by digits
                if pos < chars.len()
                    && chars[pos] == ','
                    && pos + 1 < chars.len()
                    && chars[pos + 1].is_ascii_digit()
                {
                    num_str.push('.');
                    pos += 1;
                    while pos < chars.len() && chars[pos].is_ascii_digit() {
                        num_str.push(chars[pos]);
                        pos += 1;
                    }
                }
                let value: f64 = num_str
                    .parse()
                    .map_err(|_| ExathError::parse("Invalid number"))?;
                tokens.push(Token::Number(value));
            }

            // Greek letters for constants + ASCII identifiers
            ch if ch.is_ascii_alphabetic()
                || ch == '\u{03c0}'
                || ch == '\u{03d5}'
                || ch == '\u{03b5}' =>
            {
                let mut name = String::new();
                if ch == '\u{03c0}' || ch == '\u{03d5}' || ch == '\u{03b5}' {
                    name.push(ch);
                    pos += 1;
                } else {
                    while pos < chars.len()
                        && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '_')
                    {
                        name.push(chars[pos]);
                        pos += 1;
                    }
                }
                let lower = name.to_lowercase();

                // Handle log with subscript base: log₍base₎
                if lower == "log" && pos < chars.len() && chars[pos] == '\u{208D}' {
                    pos += 1;
                    let mut base_str = String::new();
                    while pos < chars.len() && chars[pos] != '\u{208E}' {
                        base_str.push(chars[pos]);
                        pos += 1;
                    }
                    if pos < chars.len() && chars[pos] == '\u{208E}' {
                        pos += 1;
                    }
                    tokens.push(Token::Ident(format!("log:{}", base_str)));
                } else {
                    tokens.push(Token::Ident(lower));
                }
            }

            // √ symbol → sqrt function
            '\u{221a}' => {
                tokens.push(Token::Ident("sqrt".to_string()));
                pos += 1;
            }

            ch => {
                return Err(ExathError::parse(format!(
                    "Unexpected character: '{}'",
                    ch
                )));
            }
        }
    }
    Ok(tokens)
}
