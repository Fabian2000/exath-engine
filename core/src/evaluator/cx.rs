use crate::error::ExathError;

/// Complex number type used throughout exath-engine.
/// All math is done over ℂ; real numbers are the special case im == 0.
#[derive(Debug, Clone, Copy)]
pub struct Cx {
    pub re: f64,
    pub im: f64,
}

impl Cx {
    pub fn real(re: f64) -> Self {
        Cx { re, im: 0.0 }
    }

    pub fn is_real(&self) -> bool {
        self.im.abs() < 1e-12
    }

    pub fn add(self, rhs: Cx) -> Cx {
        Cx {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }

    pub fn sub(self, rhs: Cx) -> Cx {
        Cx {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }

    pub fn mul(self, rhs: Cx) -> Cx {
        Cx {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }

    pub fn div(self, rhs: Cx) -> Result<Cx, ExathError> {
        let denominator = rhs.re * rhs.re + rhs.im * rhs.im;
        if denominator == 0.0 {
            return Err(ExathError::domain("Division by zero"));
        }
        Ok(Cx {
            re: (self.re * rhs.re + self.im * rhs.im) / denominator,
            im: (self.im * rhs.re - self.re * rhs.im) / denominator,
        })
    }

    pub fn neg(self) -> Cx {
        Cx {
            re: -self.re,
            im: -self.im,
        }
    }

    pub fn abs_val(self) -> f64 {
        (self.re * self.re + self.im * self.im).sqrt()
    }

    pub fn arg(self) -> f64 {
        // Normalize -0.0 to 0.0 to get consistent principal value (atan2(-0,-x) = -π, not +π)
        let im = if self.im == 0.0 { 0.0 } else { self.im };
        im.atan2(self.re)
    }

    pub fn ln(self) -> Result<Cx, ExathError> {
        let modulus = self.abs_val();
        if modulus == 0.0 {
            return Err(ExathError::domain("ln undefined for 0"));
        }
        Ok(Cx {
            re: modulus.ln(),
            im: self.arg(),
        })
    }

    pub fn exp(self) -> Cx {
        let exp_re = self.re.exp();
        Cx {
            re: exp_re * self.im.cos(),
            im: exp_re * self.im.sin(),
        }
    }

    pub fn pow(self, exponent: Cx) -> Result<Cx, ExathError> {
        if self.re == 0.0 && self.im == 0.0 {
            if exponent.re > 0.0 {
                return Ok(Cx::real(0.0));
            }
            return Err(ExathError::domain("0^x undefined for x≤0"));
        }
        Ok(self.ln()?.mul(exponent).exp())
    }

    pub fn sqrt(self) -> Cx {
        let modulus = self.abs_val().sqrt();
        let half_angle = self.arg() / 2.0;
        Cx {
            re: modulus * half_angle.cos(),
            im: modulus * half_angle.sin(),
        }
    }
}
