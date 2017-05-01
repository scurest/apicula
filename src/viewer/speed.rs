use std::default::Default;

const SPEEDS: [f32; 9] = [0.25, 0.5, 1.0, 3.0, 7.5, 15.0, 32.0, 64.0, 96.0];
const DEFAULT_LEVEL: u8 = 2;

/// Measurement for speed that advances in discrete steps.
///
/// Abstracted out for convenience.
pub struct SpeedLevel {
    level: u8,
}

impl SpeedLevel {
    pub fn speed_up(&mut self) {
        if self.level + 1 != SPEEDS.len() as u8 {
            self.level += 1
        }
    }

    pub fn speed_down(&mut self) {
        if self.level != 0 {
            self.level -= 1
        }
    }

    pub fn speed(&self) -> f32 {
        SPEEDS[self.level as usize]
    }
}

impl Default for SpeedLevel {
    fn default() -> SpeedLevel {
        SpeedLevel { level: DEFAULT_LEVEL }
    }
}
