use std::ops::{Add, AddAssign, Sub};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeStamp {
    hour: f32, // [0, 24)
    day: i32,
}

impl TimeStamp {
    pub fn new(hour: f32, day: i32) -> Result<Self, String> {
        if hour < 0.0 || hour >= 24.0 {
            return Err(format!("invalid time stamp hour: {}", hour));
        }
        Ok(Self { hour, day })
    }

    pub fn hour(&self) -> f32 {
        self.hour
    }
    pub fn day(&self) -> i32 {
        self.day
    }
}

// Implement += operator
impl AddAssign<f64> for TimeStamp {
    fn add_assign(&mut self, hours: f64) {
        let total_hours = self.hour as f64 + hours;
        let days_to_add = (total_hours / 24.0).floor() as i32;
        let remaining_hours = (total_hours % 24.0) as f32;

        self.day += days_to_add;
        self.hour = remaining_hours;
    }
}

// Implement + operator
impl Add<f64> for TimeStamp {
    type Output = Self;
    fn add(mut self, rhs: f64) -> Self::Output {
        self += rhs;
        self
    }
}

// Implement comparison (Ordering)
impl PartialOrd for TimeStamp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.day.partial_cmp(&other.day) {
            Some(std::cmp::Ordering::Equal) => self.hour.partial_cmp(&other.hour),
            ord => ord,
        }
    }
}

// Implement subtraction (difference in hours)
impl Sub for TimeStamp {
    type Output = f64;
    fn sub(self, rhs: Self) -> Self::Output {
        let day_diff = (self.day - rhs.day) as f64;
        let hour_diff = (self.hour - rhs.hour) as f64;
        (day_diff * 24.0) + hour_diff
    }
}

