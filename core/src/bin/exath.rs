use exath_engine::{AngleMode, CalcResult, Session};
use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut session = Session::new(AngleMode::Rad);

    if args.len() > 1 {
        // File mode: run a script
        let path = &args[1];
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading {}: {}", path, e);
                std::process::exit(1);
            }
        };
        run_lines(&mut session, content.lines(), true);
    } else {
        // REPL mode
        println!("exath 0.1 — interactive DSL session (type 'exit' to quit)");
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut line_num = 0u32;

        loop {
            print!(">> ");
            stdout.flush().ok();

            let mut line = String::new();
            match stdin.lock().read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Read error: {}", e);
                    break;
                }
            }

            let trimmed = line.trim();
            if trimmed == "exit" || trimmed == "quit" {
                break;
            }
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            line_num += 1;
            eval_and_print(&mut session, trimmed, line_num, true);
        }
    }
}

fn run_lines<'a>(session: &mut Session, lines: impl Iterator<Item = &'a str>, verbose: bool) {
    for (i, line) in lines.enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        eval_and_print(session, trimmed, (i + 1) as u32, verbose);
    }
}

fn eval_and_print(session: &mut Session, line: &str, line_num: u32, show_input: bool) {
    // Detect if this is a function definition (contains `(` before `=`)
    let is_fn_def = is_function_def(line);
    let is_assignment = !is_fn_def && is_var_assignment(line);

    match session.eval(line) {
        Ok(result) => {
            if is_fn_def {
                // Function definitions: print confirmation
                if show_input {
                    println!("  defined: {}", line);
                }
            } else {
                let formatted = format_result(&result);
                if is_assignment {
                    // Show the assignment with result
                    println!("  {} = {}", line.split('=').next().unwrap_or(line).trim(), formatted);
                } else {
                    println!("  {}", formatted);
                }
            }
        }
        Err(e) => {
            eprintln!("  [line {}] Error: {}", line_num, e);
        }
    }
}

fn format_result(result: &CalcResult) -> String {
    match result {
        CalcResult::Real(f) => format_f64(*f),
        CalcResult::Complex(re, im) => {
            let re_str = format_f64(*re);
            if *im >= 0.0 {
                format!("{} + {}i", re_str, format_f64(*im))
            } else {
                format!("{} - {}i", re_str, format_f64(-*im))
            }
        }
    }
}

fn format_f64(f: f64) -> String {
    let rounded = f.round();
    let tol = f.abs().max(1.0) * 1e-12;
    if (f - rounded).abs() < tol && f.abs() < 1e15 {
        format!("{:.0}", rounded)
    } else {
        format!("{}", f)
    }
}

/// Quick check if line looks like `name(params) = body`.
fn is_function_def(line: &str) -> bool {
    if let Some(lp) = line.find('(') {
        if let Some(rp) = line[lp..].find(')') {
            let after = line[lp + rp + 1..].trim_start();
            if after.starts_with('=') && !after.starts_with("==") {
                return true;
            }
        }
    }
    false
}

/// Quick check if line looks like `ident = expr` (not ==, <=, >=, !=).
fn is_var_assignment(line: &str) -> bool {
    for (i, b) in line.bytes().enumerate() {
        if b == b'=' {
            let prev = if i > 0 { line.as_bytes()[i - 1] } else { 0 };
            let next = if i + 1 < line.len() { line.as_bytes()[i + 1] } else { 0 };
            if prev != b'!' && prev != b'<' && prev != b'>' && next != b'=' {
                let lhs = line[..i].trim();
                if let Some(first) = lhs.chars().next() {
                    return first.is_ascii_alphabetic()
                        && lhs.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                }
                return false;
            }
        }
    }
    false
}
