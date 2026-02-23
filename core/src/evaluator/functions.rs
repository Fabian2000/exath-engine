use crate::angle_mode::AngleMode;
use crate::error::ExathError;
use super::cx::Cx;

pub fn apply_function(name: &str, z: Cx, angle_mode: AngleMode) -> Result<Cx, ExathError> {
    match name {
        "sin" => {
            let angle = angle_mode.to_radians(z.re);
            Ok(Cx {
                re: angle.sin() * z.im.cosh(),
                im: angle.cos() * z.im.sinh(),
            })
        }
        "cos" => {
            let angle = angle_mode.to_radians(z.re);
            Ok(Cx {
                re: angle.cos() * z.im.cosh(),
                im: -angle.sin() * z.im.sinh(),
            })
        }
        "tan" => {
            let sin = apply_function("sin", z, angle_mode)?;
            let cos = apply_function("cos", z, angle_mode)?;
            sin.div(cos)
        }
        "cot" => {
            let sin = apply_function("sin", z, angle_mode)?;
            let cos = apply_function("cos", z, angle_mode)?;
            cos.div(sin)
        }
        "asin" => {
            // asin(z) = -i · ln(iz + sqrt(1-z²))
            let iz = Cx { re: -z.im, im: z.re };
            let one_minus_z2 = Cx::real(1.0).sub(z.mul(z)).sqrt();
            let result = iz.add(one_minus_z2).ln()?.mul(Cx { re: 0.0, im: -1.0 });
            Ok(Cx {
                re: angle_mode.from_radians(result.re),
                im: result.im,
            })
        }
        "acos" => {
            // acos(z) = -i · ln(z + i·sqrt(1-z²))
            let one_minus_z2 = Cx::real(1.0).sub(z.mul(z)).sqrt();
            let i_sqrt = one_minus_z2.mul(Cx { re: 0.0, im: 1.0 });
            let result = z.add(i_sqrt).ln()?.mul(Cx { re: 0.0, im: -1.0 });
            Ok(Cx {
                re: angle_mode.from_radians(result.re),
                im: result.im,
            })
        }
        "atan" => {
            // atan(z) = (i/2) · ln((i+z)/(i-z))
            let i = Cx { re: 0.0, im: 1.0 };
            let half_i = i.div(Cx::real(2.0))?;
            let quotient = i.add(z).div(i.sub(z))?;
            let result = half_i.mul(quotient.ln()?);
            Ok(Cx {
                re: angle_mode.from_radians(result.re),
                im: result.im,
            })
        }
        "acot" => {
            apply_function("atan", Cx::real(1.0).div(z)?, angle_mode)
        }

        "sinh" => {
            Ok(Cx {
                re: z.re.sinh() * z.im.cos(),
                im: z.re.cosh() * z.im.sin(),
            })
        }
        "cosh" => {
            Ok(Cx {
                re: z.re.cosh() * z.im.cos(),
                im: z.re.sinh() * z.im.sin(),
            })
        }
        "tanh" => {
            let sinh = apply_function("sinh", z, angle_mode)?;
            let cosh = apply_function("cosh", z, angle_mode)?;
            sinh.div(cosh)
        }
        "coth" => {
            let sinh = apply_function("sinh", z, angle_mode)?;
            let cosh = apply_function("cosh", z, angle_mode)?;
            cosh.div(sinh)
        }

        // asinh(z) = ln(z + sqrt(z²+1))
        "asinh" => {
            let z2_plus_1 = z.mul(z).add(Cx::real(1.0)).sqrt();
            z.add(z2_plus_1).ln()
        }
        // acosh(z) = ln(z + sqrt(z²-1))
        "acosh" => {
            let z2_minus_1 = z.mul(z).sub(Cx::real(1.0)).sqrt();
            z.add(z2_minus_1).ln()
        }
        // atanh(z) = (1/2) · ln((1+z)/(1-z))
        "atanh" => {
            let one = Cx::real(1.0);
            let half = Cx::real(0.5);
            let quotient = one.add(z).div(one.sub(z))?;
            Ok(quotient.ln()?.mul(half))
        }
        // acoth(z) = atanh(1/z)
        "acoth" => {
            apply_function("atanh", Cx::real(1.0).div(z)?, angle_mode)
        }

        "sec" => {
            Cx::real(1.0).div(apply_function("cos", z, angle_mode)?)
        }
        "csc" => {
            Cx::real(1.0).div(apply_function("sin", z, angle_mode)?)
        }
        "asec" => {
            apply_function("acos", Cx::real(1.0).div(z)?, angle_mode)
        }
        "acsc" => {
            apply_function("asin", Cx::real(1.0).div(z)?, angle_mode)
        }

        "sech" => {
            Cx::real(1.0).div(apply_function("cosh", z, angle_mode)?)
        }
        "csch" => {
            Cx::real(1.0).div(apply_function("sinh", z, angle_mode)?)
        }
        "asech" => {
            apply_function("acosh", Cx::real(1.0).div(z)?, angle_mode)
        }
        "acsch" => {
            apply_function("asinh", Cx::real(1.0).div(z)?, angle_mode)
        }

        "exp" => Ok(z.exp()),
        "ln" => z.ln(),
        "lg" | "log" => {
            let ln_10 = 10.0_f64.ln();
            Ok(z.ln()?.mul(Cx::real(1.0 / ln_10)))
        }
        "sqrt" => Ok(z.sqrt()),
        "cbrt" => z.pow(Cx::real(1.0 / 3.0)),
        "abs" => Ok(Cx::real(z.abs_val())),

        "floor" => {
            if !z.is_real() {
                return Err(ExathError::arg_type("floor only defined for real numbers"));
            }
            Ok(Cx::real(z.re.floor()))
        }
        "ceil" => {
            if !z.is_real() {
                return Err(ExathError::arg_type("ceil only defined for real numbers"));
            }
            Ok(Cx::real(z.re.ceil()))
        }
        "round" => {
            if !z.is_real() {
                return Err(ExathError::arg_type("round only defined for real numbers"));
            }
            Ok(Cx::real(z.re.round()))
        }
        "trunc" => {
            if !z.is_real() {
                return Err(ExathError::arg_type("trunc only defined for real numbers"));
            }
            Ok(Cx::real(z.re.trunc()))
        }
        "frac" => {
            if !z.is_real() {
                return Err(ExathError::arg_type("frac only defined for real numbers"));
            }
            Ok(Cx::real(z.re.fract()))
        }

        "sign" | "sgn" => {
            if !z.is_real() {
                return Err(ExathError::arg_type("sign only defined for real numbers"));
            }
            Ok(Cx::real(z.re.signum()))
        }

        "arg" => Ok(Cx::real(z.arg())),
        "conj" => Ok(Cx { re: z.re, im: -z.im }),
        "real" => Ok(Cx::real(z.re)),
        "imag" => Ok(Cx::real(z.im)),

        "deg" => Ok(Cx::real(z.re.to_degrees())),
        "rad" => Ok(Cx::real(z.re.to_radians())),

        _ if name.starts_with("log:") => {
            let base_str = &name[4..];
            let base_expr = base_str.replace(',', ".");
            let base: f64 = base_expr.parse().map_err(|_| {
                ExathError::parse(format!("Invalid log base: {}", base_str))
            })?;
            if base <= 0.0 || base == 1.0 {
                return Err(ExathError::domain("Log base must be positive and not 1"));
            }
            Ok(z.ln()?.mul(Cx::real(1.0 / base.ln())))
        }

        _ => Err(ExathError::undefined(format!("Unknown function: {}", name))),
    }
}
