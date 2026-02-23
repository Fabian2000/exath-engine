#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AngleMode {
    Deg,
    Rad,
    Grad,
}

impl AngleMode {
    pub fn cycle(&self) -> Self {
        match self {
            AngleMode::Deg => AngleMode::Rad,
            AngleMode::Rad => AngleMode::Grad,
            AngleMode::Grad => AngleMode::Deg,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            AngleMode::Deg => "Deg",
            AngleMode::Rad => "Rad",
            AngleMode::Grad => "Grad",
        }
    }

    pub fn to_radians(&self, value: f64) -> f64 {
        match self {
            AngleMode::Deg => value.to_radians(),
            AngleMode::Rad => value,
            AngleMode::Grad => value * std::f64::consts::PI / 200.0,
        }
    }

    pub fn from_radians(&self, value: f64) -> f64 {
        match self {
            AngleMode::Deg => value.to_degrees(),
            AngleMode::Rad => value,
            AngleMode::Grad => value * 200.0 / std::f64::consts::PI,
        }
    }
}
