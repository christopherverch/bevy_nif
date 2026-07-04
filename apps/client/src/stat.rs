use std::ops::{Add, Div};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, Default)]
pub struct Stat<T> {
    pub base: T,
    pub modifier: T,
}

impl<T> Stat<T>
where
    T: Copy + Default + PartialOrd + Add<Output = T>,
{
    pub fn get_modified(&self, capped: bool) -> T {
        let sum = self.base + self.modifier;
        let zero = T::default();

        // Can't use max without further restricting T, so just use < instead
        if capped && sum < zero { zero } else { sum }
    }
}
#[derive(Clone, Debug)]
pub struct DynamicStat<T> {
    pub static_stat: Stat<T>,
    pub current: T,
}

// So we can do stat.base
impl<T> Deref for DynamicStat<T> {
    type Target = Stat<T>;

    fn deref(&self) -> &Self::Target {
        &self.static_stat
    }
}

impl<T> DerefMut for DynamicStat<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.static_stat
    }
}
impl Default for DynamicStat<f32> {
    fn default() -> Self {
        Self {
            static_stat: Stat {
                base: 100.0,
                modifier: 100.0,
            },
            current: 100.0,
        }
    }
}

impl<T> DynamicStat<T>
where
    T: PartialOrd + Add<Output = T> + Copy + Default + Div<Output = T> + From<u8>,
{
    pub fn set_current(
        &mut self,
        value: T,
        allow_decrease_below_zero: bool,
        allow_increase_above_modifier: bool,
    ) {
        let modified = self.static_stat.get_modified(true);
        let zero = T::default();
        if value > self.current {
            // Increase
            if value <= modified || allow_increase_above_modifier {
                self.current = value;
            } else if self.current <= modified {
                self.current = modified;
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttributeValue {
    pub base: f32,
    pub modifier: f32,
    pub damage: f32,
}

impl AttributeValue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_modified(&self) -> f32 {
        (self.base - self.damage + self.modifier).max(0.0)
    }

    pub fn get_base(&self) -> f32 {
        self.base
    }

    pub fn get_modifier(&self) -> f32 {
        self.modifier
    }

    pub fn set_base(&mut self, base: f32, clear_modifier: bool) {
        self.base = base;
        if clear_modifier {
            self.modifier = 0.0;
        }
    }

    pub fn set_modifier(&mut self, mod_val: f32) {
        if mod_val < 0.0 {
            self.modifier = 0.0;
            self.damage -= mod_val; // subtracting negative adds to damage
        } else {
            self.modifier = mod_val;
        }
    }

    pub fn damage(&mut self, damage: f32) {
        self.damage += damage;
    }

    pub fn restore(&mut self, amount: f32) {
        self.damage = (self.damage - amount).max(0.0);
    }

    pub fn get_damage(&self) -> f32 {
        self.damage
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillValue {
    pub attribute: AttributeValue,
    pub progress: f32,
}

impl SkillValue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_progress(&self) -> f32 {
        self.progress
    }

    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress;
    }
}
