use crate::error::ExathError;

pub fn factorial(n: f64) -> Result<f64, ExathError> {
    if n < 0.0 || n.fract() != 0.0 {
        return Err(ExathError::domain(
            "Factorial only defined for non-negative integers",
        ));
    }
    if n > 170.0 {
        return Ok(f64::INFINITY);
    }
    let mut result = 1.0f64;
    let mut i = 2.0f64;
    while i <= n {
        result *= i;
        i += 1.0;
    }
    Ok(result)
}
