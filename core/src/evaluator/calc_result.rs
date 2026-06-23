use super::cx::Cx;

/// The result of a numeric evaluation.
///
/// A real result carries one `f64`; a complex result carries `(real, imaginary)`.
/// Match on it to read the value:
///
/// ```
/// use exath_engine::{evaluate_complex, AngleMode, CalcResult};
///
/// match evaluate_complex("sqrt(-4)", AngleMode::Rad)? {
///     CalcResult::Real(x) => println!("real: {x}"),
///     CalcResult::Complex(re, im) => println!("{re} + {im}i"),
/// }
/// # Ok::<(), exath_engine::ExathError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum CalcResult {
    /// A real value.
    Real(f64),
    /// A complex value, as `(real, imaginary)`.
    Complex(f64, f64),
}

impl CalcResult {
    pub fn to_f64_lossy(&self) -> f64 {
        match self {
            CalcResult::Real(value) => *value,
            CalcResult::Complex(_, _) => f64::NAN,
        }
    }
}

impl Cx {
    pub fn to_calc_result(self) -> CalcResult {
        if self.is_real() {
            CalcResult::Real(self.re)
        } else {
            CalcResult::Complex(self.re, self.im)
        }
    }
}
