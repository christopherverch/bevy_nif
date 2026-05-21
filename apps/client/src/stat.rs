use std::ops::{Add, Div};

#[derive(Clone, Debug, Default)]
pub struct Stat<T> {
    pub base: T,
    pub modifier: T,
}

impl<T> Stat<T>
where
    T: Copy + Default + PartialOrd + Add<Output = T>,
{
    pub fn get_modifier(&self, capped: bool) -> T {
        let sum = self.base + self.modifier;
        let zero = T::default();

        if capped && sum < zero { zero } else { sum }
    }
}
#[derive(Clone, Debug)]
pub struct DynamicStat<T> {
    pub static_stat: Stat<T>,
    pub current: T,
}
impl Default for DynamicStat<f32> {
    fn default() -> Self {
        Self {
            static_stat: Stat::default(),
            current: 0.0,
        }
    }
}

impl<T> DynamicStat<T>
where
    T: PartialOrd + Copy + Default + Div<Output = T> + From<u8>,
{
    pub fn set_current(
        &mut self,
        value: T,
        allow_decrease_below_zero: bool,
        allow_increase_above_modifier: bool,
    ) {
        let modifier = self.static_stat.modifier;
        let zero = T::default();

        if value > self.current {
            // Increase
            if value <= modifier || allow_increase_above_modifier {
                self.current = value;
            } else if self.current > modifier {
                return;
            } else {
                self.current = modifier;
            }
        } else if value > zero || allow_decrease_below_zero {
            // Allowed decrease
            self.current = value;
        } else if self.current > zero {
            // Capped decrease
            self.current = zero;
        }
    }

    pub fn get_ratio(&self, nan_is_zero: bool) -> T {
        let modifier = self.static_stat.modifier;
        let zero = T::default();

        if modifier == zero {
            if nan_is_zero {
                return zero;
            }
            // Returns 1
            return T::from(1);
        }

        self.current / modifier
    }
}
// Partial Equality: Compares the "current" values
impl PartialEq<f32> for DynamicStat<f32> {
    fn eq(&self, other: &f32) -> bool {
        self.current == *other
    }
}

// Required for the derive/generic parity
impl PartialEq for DynamicStat<f32> {
    fn eq(&self, other: &Self) -> bool {
        self.current == other.current
    }
}

// Partial Comparison: Allows using <, >, <=, >= against f32
impl PartialOrd<f32> for DynamicStat<f32> {
    fn partial_cmp(&self, other: &f32) -> Option<std::cmp::Ordering> {
        self.current.partial_cmp(other)
    }
}

// Allows comparing DynamicStat to DynamicStat
impl PartialOrd for DynamicStat<f32> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.current.partial_cmp(&other.current)
    }
}
