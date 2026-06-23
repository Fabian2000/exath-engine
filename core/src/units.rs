//! Physical units, dimensional analysis and conversion.
//!
//! A self-contained, additive module (does not touch the scalar evaluator).
//! Quantities carry a value in SI base units plus a 7-dimensional exponent
//! vector, so addition checks compatibility and multiplication combines
//! dimensions. Conversion supports affine units (°C, °F). Panic-free.

use crate::error::ExathError;

/// SI base-dimension exponents: [length, mass, time, current, temperature,
/// amount, luminous intensity].
pub type Dim = [i32; 7];

const DIMLESS: Dim = [0, 0, 0, 0, 0, 0, 0];

/// A unit definition: `si_value = value * factor + offset`, with a dimension.
#[derive(Clone, Copy, Debug)]
pub struct Unit {
    pub factor: f64,
    pub offset: f64,
    pub dim: Dim,
}

/// A quantity stored in SI base units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    pub si_value: f64,
    pub dim: Dim,
}

fn unit(factor: f64, dim: Dim) -> Unit {
    Unit { factor, offset: 0.0, dim }
}

fn affine(factor: f64, offset: f64, dim: Dim) -> Unit {
    Unit { factor, offset, dim }
}

// Dimension constants.
const LEN: Dim = [1, 0, 0, 0, 0, 0, 0];
const MASS: Dim = [0, 1, 0, 0, 0, 0, 0];
const TIME: Dim = [0, 0, 1, 0, 0, 0, 0];
const TEMP: Dim = [0, 0, 0, 0, 1, 0, 0];
const AREA: Dim = [2, 0, 0, 0, 0, 0, 0];
const VOL: Dim = [3, 0, 0, 0, 0, 0, 0];

/// Look up a unit by name (case-sensitive symbol).
pub fn unit_of(name: &str) -> Option<Unit> {
    let u = match name {
        // Length (SI base: metre)
        "m" => unit(1.0, LEN),
        "km" => unit(1000.0, LEN),
        "cm" => unit(0.01, LEN),
        "mm" => unit(0.001, LEN),
        "um" => unit(1e-6, LEN),
        "nm" => unit(1e-9, LEN),
        "mi" => unit(1609.344, LEN),
        "yd" => unit(0.9144, LEN),
        "ft" => unit(0.3048, LEN),
        "in" => unit(0.0254, LEN),
        // Mass (SI base: kilogram)
        "kg" => unit(1.0, MASS),
        "g" => unit(0.001, MASS),
        "mg" => unit(1e-6, MASS),
        "t" => unit(1000.0, MASS),
        "lb" => unit(0.45359237, MASS),
        "oz" => unit(0.028349523125, MASS),
        // Time (SI base: second)
        "s" => unit(1.0, TIME),
        "ms" => unit(0.001, TIME),
        "min" => unit(60.0, TIME),
        "h" => unit(3600.0, TIME),
        "day" => unit(86400.0, TIME),
        // Area
        "m2" => unit(1.0, AREA),
        "km2" => unit(1e6, AREA),
        "ha" => unit(10000.0, AREA),
        // Volume
        "m3" => unit(1.0, VOL),
        "l" => unit(0.001, VOL),
        "ml" => unit(1e-6, VOL),
        // Temperature (affine)
        "K" => affine(1.0, 0.0, TEMP),
        "degC" => affine(1.0, 273.15, TEMP),
        "degF" => affine(5.0 / 9.0, 255.372222222222, TEMP),
        _ => return None,
    };
    Some(u)
}

impl Quantity {
    /// Build a quantity from a value expressed in `unit_name`.
    pub fn of(value: f64, unit_name: &str) -> Result<Quantity, ExathError> {
        let u = unit_of(unit_name)
            .ok_or_else(|| ExathError::domain(format!("unknown unit '{}'", unit_name)))?;
        Ok(Quantity { si_value: value * u.factor + u.offset, dim: u.dim })
    }

    /// Express this quantity in `unit_name` (dimensions must match).
    pub fn to(&self, unit_name: &str) -> Result<f64, ExathError> {
        let u = unit_of(unit_name)
            .ok_or_else(|| ExathError::domain(format!("unknown unit '{}'", unit_name)))?;
        if u.dim != self.dim {
            return Err(ExathError::domain(format!(
                "incompatible dimensions: cannot express this quantity in '{}'",
                unit_name
            )));
        }
        Ok((self.si_value - u.offset) / u.factor)
    }

    pub fn add(&self, other: &Quantity) -> Result<Quantity, ExathError> {
        if self.dim != other.dim {
            return Err(ExathError::domain("cannot add quantities of different dimensions"));
        }
        Ok(Quantity { si_value: self.si_value + other.si_value, dim: self.dim })
    }

    pub fn sub(&self, other: &Quantity) -> Result<Quantity, ExathError> {
        if self.dim != other.dim {
            return Err(ExathError::domain(
                "cannot subtract quantities of different dimensions",
            ));
        }
        Ok(Quantity { si_value: self.si_value - other.si_value, dim: self.dim })
    }

    pub fn mul(&self, other: &Quantity) -> Quantity {
        let mut dim = self.dim;
        for i in 0..7 {
            dim[i] += other.dim[i];
        }
        Quantity { si_value: self.si_value * other.si_value, dim }
    }

    pub fn div(&self, other: &Quantity) -> Result<Quantity, ExathError> {
        if other.si_value == 0.0 {
            return Err(ExathError::domain("division by zero quantity"));
        }
        let mut dim = self.dim;
        for i in 0..7 {
            dim[i] -= other.dim[i];
        }
        Ok(Quantity { si_value: self.si_value / other.si_value, dim })
    }

    pub fn is_dimensionless(&self) -> bool {
        self.dim == DIMLESS
    }
}

/// Convenience: convert `value` from one unit to another (same dimension).
pub fn convert(value: f64, from: &str, to: &str) -> Result<f64, ExathError> {
    Quantity::of(value, from)?.to(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn length_conversions() {
        assert!(approx(convert(1.0, "km", "m").unwrap(), 1000.0));
        assert!(approx(convert(100.0, "cm", "m").unwrap(), 1.0));
        assert!(approx(convert(1.0, "mi", "km").unwrap(), 1.609344));
        assert!(approx(convert(12.0, "in", "ft").unwrap(), 1.0));
    }

    #[test]
    fn time_and_mass() {
        assert!(approx(convert(1.0, "h", "s").unwrap(), 3600.0));
        assert!(approx(convert(1.0, "kg", "g").unwrap(), 1000.0));
    }

    #[test]
    fn temperature_affine() {
        assert!(approx(convert(0.0, "degC", "K").unwrap(), 273.15));
        assert!(approx(convert(100.0, "degC", "degF").unwrap(), 212.0));
        assert!(approx(convert(32.0, "degF", "degC").unwrap(), 0.0));
    }

    #[test]
    fn dimensional_arithmetic() {
        // 100 km / 2 h = 50 km/h = 13.888… m/s
        let dist = Quantity::of(100.0, "km").unwrap();
        let time = Quantity::of(2.0, "h").unwrap();
        let speed = dist.div(&time).unwrap();
        assert!(approx(speed.si_value, 100000.0 / 7200.0));
        assert_eq!(speed.dim, [1, 0, -1, 0, 0, 0, 0]); // length / time

        // area: 3 m * 4 m = 12 m²
        let a = Quantity::of(3.0, "m").unwrap();
        let b = Quantity::of(4.0, "m").unwrap();
        let area = a.mul(&b);
        assert!(approx(area.to("m2").unwrap(), 12.0));
    }

    #[test]
    fn incompatible_is_error() {
        assert!(convert(1.0, "m", "s").is_err());
        let m = Quantity::of(1.0, "m").unwrap();
        let s = Quantity::of(1.0, "s").unwrap();
        assert!(m.add(&s).is_err());
        assert!(convert(1.0, "m", "bogus").is_err());
    }
}
