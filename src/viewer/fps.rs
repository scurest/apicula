use super::FPS_INTERVAL;

/// Tracks frames per second.
pub struct FpsCounter {
    fps: f64,
    time_acc: f64,
    frames_acc: f64,
}

impl FpsCounter {
    pub fn new() -> FpsCounter {
        FpsCounter {
            fps: 0.0,
            time_acc: 0.0,
            frames_acc: 0.0,
        }
    }

    pub fn fps(&self) -> f64 {
        self.fps
    }

    pub fn update(&mut self, dt: f64) {
        self.time_acc += dt;
        self.frames_acc += 1.0;

        if self.time_acc > FPS_INTERVAL {
            self.fps = self.frames_acc / self.time_acc;
            self.time_acc = 0.0;
            self.frames_acc = 0.0;
        }
    }
}
