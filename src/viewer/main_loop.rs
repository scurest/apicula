use glium::glutin::{self, EventsLoop, dpi::{LogicalSize, LogicalPosition}};
use super::viewer::Viewer;
use db::Database;
use connection::Connection;

pub fn main_loop(db: Database, conn: Connection) {
    let window_builder = glutin::WindowBuilder::new()
        .with_dimensions(LogicalSize {
            width: super::WINDOW_WIDTH as f64,
            height: super::WINDOW_HEIGHT as f64
        });
    let context_builder = glutin::ContextBuilder::new()
        .with_depth_buffer(24);
    let mut events_loop = EventsLoop::new();
    let display = glium::Display::new(window_builder, context_builder, &events_loop)
        .expect("failed to get rendering context");

    let window = display.gl_window();

    let mut viewer = Viewer::new(&display, db, conn);

    // Track the last known mouse position.
    let mut last_mouse_xy = LogicalPosition { x: 0.0, y: 0.0 };
    // Mouse grabbing/capturing, ie. moving the mouse stops moving the cursor
    // and our window gets the raw deltas. We have to fake it.
    let mut mouse_grabbed = false;
    // The position of the mouse when a capture started. Used so we can restore
    // it when the capture ends.
    let mut saved_mouse_pos = LogicalPosition { x: 0.0, y: 0.0 };

    let grab_cursor = |grab: bool| {
        let _ = window.grab_cursor(grab);
        let _ = window.hide_cursor(grab);
    };

    // Buffer for the window's title
    let mut win_title = String::new();

    let mut cur_time = time::precise_time_ns();
    let mut last_time;
    loop {
        last_time = cur_time;
        cur_time = time::precise_time_ns();
        let dt_in_ns = cur_time.wrapping_sub(last_time);
        let dt = dt_in_ns as f64 / 1_000_000_000.0;

        viewer.update(&display, dt);

        if let Some(LogicalSize { width, height }) = window.get_inner_size() {
            viewer.set_aspect_ratio(width / height);
        }

        let mut frame = display.draw();
        viewer.draw(&mut frame);
        frame.finish().expect("rendering error");

        win_title.clear();
        viewer.title(&mut win_title);
        window.set_title(&win_title);

        let mut should_close = false;
        events_loop.poll_events(|ev| {
            use self::glutin::Event as Ev;
            use self::glutin::WindowEvent as WEv;
            use self::glutin::DeviceEvent as DEv;

            match ev {
                Ev::WindowEvent { event, .. } => match event {
                    WEv::CloseRequested => {
                        should_close = true;
                    }
                    WEv::KeyboardInput { input, .. } => {
                        if input.virtual_keycode.is_none() { return; }
                        let keycode = input.virtual_keycode.unwrap();
                        viewer.key(&display, (input.state, keycode));
                    }
                    WEv::MouseInput { state, button, .. } => {
                        use self::glutin::ElementState as Es;
                        use self::glutin::MouseButton as MB;

                        match (state, button) {
                            (Es::Pressed, MB::Left) => {
                                mouse_grabbed = true;
                                saved_mouse_pos = last_mouse_xy;
                                grab_cursor(true);
                            }
                            (Es::Released, MB::Left) => {
                                mouse_grabbed = false;
                                let _ = window.set_cursor_position(saved_mouse_pos);
                                grab_cursor(false);
                            }
                            _ => (),
                        }
                    }
                    WEv::CursorMoved { position, .. } => {
                        last_mouse_xy = position;

                        if mouse_grabbed {
                            // Warp the mouse to the center of the window to
                            // keep it inside our window to fake mouse capture.
                            let LogicalSize { width, height } =
                                window
                                .get_outer_size()
                                .unwrap_or(LogicalSize { width: 0.0, height: 0.0 });
                            let center = LogicalPosition {
                                x: width as f64 / 2.0,
                                y: height as f64 / 2.0,
                            };
                            let _ = window.set_cursor_position(center);
                        }
                    }
                    WEv::Focused(false) => {
                        viewer.blur();

                        // Release the mouse
                        mouse_grabbed = false;
                        grab_cursor(false);
                    }
                    _ => ()
                },
                Ev::DeviceEvent { event, .. } => match event {
                    DEv::MouseMotion { delta } => {
                        // delta is in an "unspecified coordinate system" but
                        // appears to be pixels on my machine
                        if mouse_grabbed {
                            viewer.mouse_drag(delta);
                        }
                    }
                    _ => (),
                },
                _ => (),
            }
        });
        if should_close { break; }
    }
}
