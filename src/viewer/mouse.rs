use glium::backend::glutin_backend::WinRef;

pub struct MouseState {
    pub pos: (i32, i32),
    pub grabbed: GrabState,
}

pub enum GrabState {
    NotGrabbed,
    Grabbed { saved_pos: (i32, i32) },
}

impl MouseState {
    pub fn new() -> MouseState {
        MouseState { pos: (0, 0), grabbed: GrabState::NotGrabbed }
    }

    // Performs "best-effort" setting of the cursor position. If
    // setting the position fails, no error is signalled, but
    // self.pos is also not updated.
    pub fn set_position(&mut self, window: &WinRef, (x, y): (i32, i32)) {
        match window.set_cursor_position(x, y) {
            Ok(()) => self.pos = (x,y),
            Err(_) => (),
        }
    }
}
