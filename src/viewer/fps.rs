/// Tracks frames per second.
///
/// It counts frames and recalculates the FPS at regular intervals
/// from the number of frames that passed in each interval.
pub struct FpsTracker {
    last_fps: f64,
    last_recalc_time: f64,
    frames_since_last_recalc: u32,
}

/// Recalculate the FPS every `RECALC_INTERVAL` seconds.
const RECALC_INTERVAL: f64 = 2.0;

impl FpsTracker {
    pub fn new() -> FpsTracker {
        FpsTracker {
            last_fps: 0.0,
            last_recalc_time: 0.0,
            frames_since_last_recalc: 0,
        }
    }

    pub fn get_fps(&self) -> f64 {
        self.last_fps
    }

    /// Notify the tracker that a frame was rendered.
    pub fn notify_frame(&mut self) {
        self.frames_since_last_recalc += 1;
    }

    pub fn update(&mut self, cur_time: f64) {
        let time_since_last_recalc = cur_time - self.last_recalc_time;
        if time_since_last_recalc >= RECALC_INTERVAL {
            // Recalc
            self.last_fps =
                self.frames_since_last_recalc as f64 / time_since_last_recalc;
            self.frames_since_last_recalc = 0;
            self.last_recalc_time = cur_time;
        }
    }
}
