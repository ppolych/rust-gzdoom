#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fixed(i64);

impl Fixed {
    const SHIFT: i32 = 16;
    const SCALE: i64 = 1 << Self::SHIFT;

    pub fn from_f64(val: f64) -> Self {
        Self((val * Self::SCALE as f64) as i64)
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }

    pub fn mul_f64(self, val: f64) -> Self {
        Self((self.0 as f64 * val) as i64)
    }
}

impl std::ops::Add for Fixed {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Fixed {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul for Fixed {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self((self.0 * rhs.0) >> Self::SHIFT)
    }
}

impl std::ops::Div for Fixed {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self((self.0 << Self::SHIFT) / rhs.0)
    }
}
