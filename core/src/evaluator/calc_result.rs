use super::cx::Cx;

#[derive(Debug, Clone)]
pub enum CalcResult {
    Real(f64),
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
