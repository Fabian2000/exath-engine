//! Interval arithmetic for rigorous error bounds.
//!
//! A self-contained, additive module (does not touch the scalar evaluator).
//! Each `Interval` represents the set [lo, hi]; operations produce an interval
//! guaranteed to contain every result of the corresponding real operation.
//! Panic-free.

use crate::error::ExathError;

/// A closed real interval `[lo, hi]` with `lo <= hi`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    pub fn new(lo: f64, hi: f64) -> Interval {
        if lo <= hi {
            Interval { lo, hi }
        } else {
            Interval { lo: hi, hi: lo }
        }
    }

    /// A degenerate interval [x, x].
    pub fn point(x: f64) -> Interval {
        Interval { lo: x, hi: x }
    }

    pub fn width(&self) -> f64 {
        self.hi - self.lo
    }

    pub fn midpoint(&self) -> f64 {
        0.5 * (self.lo + self.hi)
    }

    pub fn contains(&self, x: f64) -> bool {
        self.lo <= x && x <= self.hi
    }

    pub fn add(&self, o: &Interval) -> Interval {
        Interval::new(self.lo + o.lo, self.hi + o.hi)
    }

    pub fn sub(&self, o: &Interval) -> Interval {
        Interval::new(self.lo - o.hi, self.hi - o.lo)
    }

    pub fn mul(&self, o: &Interval) -> Interval {
        let p = [
            self.lo * o.lo,
            self.lo * o.hi,
            self.hi * o.lo,
            self.hi * o.hi,
        ];
        let lo = p.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = p.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        Interval { lo, hi }
    }

    /// Division; errors if the divisor interval contains zero.
    pub fn div(&self, o: &Interval) -> Result<Interval, ExathError> {
        if o.contains(0.0) {
            return Err(ExathError::domain("interval division by an interval containing 0"));
        }
        let recip = Interval::new(1.0 / o.hi, 1.0 / o.lo);
        Ok(self.mul(&recip))
    }

    pub fn neg(&self) -> Interval {
        Interval { lo: -self.hi, hi: -self.lo }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn basic_ops() {
        let a = Interval::new(1.0, 2.0);
        let b = Interval::new(3.0, 4.0);
        let s = a.add(&b);
        assert!(approx(s.lo, 4.0) && approx(s.hi, 6.0));
        let d = a.sub(&b);
        assert!(approx(d.lo, -3.0) && approx(d.hi, -1.0));
        let m = Interval::new(-1.0, 2.0).mul(&Interval::new(-3.0, 1.0));
        // products: 3, -1, -6, 2 → [-6, 3]
        assert!(approx(m.lo, -6.0) && approx(m.hi, 3.0));
        assert!(a.contains(1.5) && !a.contains(2.5));
    }

    #[test]
    fn division_guard() {
        let a = Interval::new(1.0, 2.0);
        assert!(a.div(&Interval::new(-1.0, 1.0)).is_err());
        let q = a.div(&Interval::new(2.0, 4.0)).unwrap();
        // [1,2]/[2,4] = [0.25, 1]
        assert!(approx(q.lo, 0.25) && approx(q.hi, 1.0));
    }
}
