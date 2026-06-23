//! Exact rational numbers with an f64 fallback.
//!
//! Used for coefficients in the symbolic normal form so that exact fractions
//! like `1/3` stay exact (and render as `x / 3` instead of `0.333…`). Values
//! that cannot be represented exactly (irrationals such as `sqrt(2)`, or results
//! that overflow `i128`) gracefully fall back to a floating-point value.
//!
//! Panic-free: no `unwrap`/`expect`/`panic!`; overflow is detected with checked
//! arithmetic and degrades to `Real`.

/// An exact rational `p/q` (q > 0, gcd(|p|,q)=1) or a floating-point real.
#[derive(Clone, Copy, Debug)]
pub enum Num {
    Rat(i128, i128),
    Real(f64),
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

impl Num {
    pub fn int(n: i128) -> Num {
        Num::Rat(n, 1)
    }

    pub fn zero() -> Num {
        Num::Rat(0, 1)
    }

    pub fn one() -> Num {
        Num::Rat(1, 1)
    }

    /// Normalise `a/b` to lowest terms with positive denominator.
    pub fn rat(a: i128, b: i128) -> Num {
        if b == 0 {
            return Num::Real(if a == 0 { f64::NAN } else { f64::INFINITY });
        }
        let g = gcd(a, b).max(1);
        let (mut n, mut d) = (a / g, b / g);
        if d < 0 {
            n = -n;
            d = -d;
        }
        Num::Rat(n, d)
    }

    /// Convert from f64, preferring an exact integer representation.
    pub fn from_f64(x: f64) -> Num {
        if x.is_finite() && x == x.trunc() && x.abs() < 9.0e15 {
            Num::Rat(x as i128, 1)
        } else {
            Num::Real(x)
        }
    }

    pub fn to_f64(&self) -> f64 {
        match self {
            Num::Rat(a, b) => *a as f64 / *b as f64,
            Num::Real(x) => *x,
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Num::Rat(a, _) => *a == 0,
            Num::Real(x) => x.abs() < 1e-12,
        }
    }

    pub fn is_one(&self) -> bool {
        match self {
            Num::Rat(a, b) => *a == *b,
            Num::Real(x) => (*x - 1.0).abs() < 1e-12,
        }
    }

    pub fn is_negative(&self) -> bool {
        self.to_f64() < 0.0
    }

    /// Integer numerator/denominator if this is an exact rational.
    pub fn as_ratio(&self) -> Option<(i128, i128)> {
        match self {
            Num::Rat(a, b) => Some((*a, *b)),
            Num::Real(_) => None,
        }
    }

    pub fn neg(&self) -> Num {
        match self {
            Num::Rat(a, b) => Num::Rat(-a, *b),
            Num::Real(x) => Num::Real(-x),
        }
    }

    pub fn abs(&self) -> Num {
        if self.is_negative() {
            self.neg()
        } else {
            *self
        }
    }

    pub fn add(&self, other: &Num) -> Num {
        if let (Num::Rat(a, b), Num::Rat(c, d)) = (self, other) {
            if let (Some(ad), Some(cb), Some(bd)) =
                (a.checked_mul(*d), c.checked_mul(*b), b.checked_mul(*d))
            {
                if let Some(n) = ad.checked_add(cb) {
                    return Num::rat(n, bd);
                }
            }
        }
        Num::Real(self.to_f64() + other.to_f64())
    }

    pub fn sub(&self, other: &Num) -> Num {
        self.add(&other.neg())
    }

    pub fn mul(&self, other: &Num) -> Num {
        if let (Num::Rat(a, b), Num::Rat(c, d)) = (self, other) {
            if let (Some(n), Some(den)) = (a.checked_mul(*c), b.checked_mul(*d)) {
                return Num::rat(n, den);
            }
        }
        Num::Real(self.to_f64() * other.to_f64())
    }

    pub fn recip(&self) -> Num {
        match self {
            Num::Rat(a, b) if *a != 0 => Num::rat(*b, *a),
            _ => Num::Real(1.0 / self.to_f64()),
        }
    }

    pub fn div(&self, other: &Num) -> Num {
        self.mul(&other.recip())
    }

    /// Raise to a real power; stays exact for integer exponents on rationals.
    pub fn powf(&self, e: f64) -> Num {
        if let Num::Rat(a, b) = self {
            if e == e.trunc() && e.abs() <= 64.0 {
                let n = e as i32;
                let (mut num, mut den) = (1i128, 1i128);
                let (base_n, base_d) = if n >= 0 { (*a, *b) } else { (*b, *a) };
                let mut ok = base_d != 0;
                for _ in 0..n.unsigned_abs() {
                    match (num.checked_mul(base_n), den.checked_mul(base_d)) {
                        (Some(x), Some(y)) => {
                            num = x;
                            den = y;
                        }
                        _ => {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    return Num::rat(num, den);
                }
            }
        }
        Num::Real(self.to_f64().powf(e))
    }

    /// Canonical string used as a map key (stable, distinguishes values).
    pub fn key(&self) -> String {
        match self {
            Num::Rat(a, b) if *b == 1 => format!("{}", a),
            Num::Rat(a, b) => format!("{}/{}", a, b),
            Num::Real(x) => format!("r{}", x),
        }
    }
}
