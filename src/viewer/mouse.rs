use glium::glutin::Window;

pub struct MouseState {
    pub pos: (f64, f64),
    pub grabbed: GrabState,
}

pub enum GrabState {
    NotGrabbed,
    Grabbed { saved_pos: (f64, f64) },
}

impl MouseState {
    pub fn new() -> MouseState {
        MouseState { pos: (0.0, 0.0), grabbed: GrabState::NotGrabbed }
    }

    // Performs "best-effort" setting of the cursor position. If
    // setting the position fails, no error is signalled, but
    // self.pos is also not updated.
    pub fn set_position(&mut self, window: &Window, (x, y): (f64, f64)) {
        match window.set_cursor_position(x as i32, y as i32) {
            Ok(()) => self.pos = (x,y),
            Err(_) => (),
        }
    }
}
